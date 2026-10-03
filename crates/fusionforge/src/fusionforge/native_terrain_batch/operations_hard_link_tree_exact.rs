use super::*;

pub(super) fn publication_payloads(source_root: &Path) -> Result<Vec<JsonValue>, String> {
    let mut pending = vec![source_root.to_path_buf()];
    let mut files = Vec::<PathBuf>::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|err| format!("could not enumerate {}: {err}", directory.display()))?;
        for entry in entries {
            let path = entry
                .map_err(|err| format!("could not enumerate {}: {err}", directory.display()))?
                .path();
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(source_root)
                .map_err(|_| format!("{} escaped {}", path.display(), source_root.display()))?;
            let relative = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let bytes = fs::read(&path)
                .map_err(|err| format!("could not read {}: {err}", path.display()))?;
            let kind = match path.extension().and_then(|value| value.to_str()) {
                Some("png") => "texture",
                _ => "data",
            };
            let role = publication_payload_role(&relative);
            Ok(json!({
                "path": relative,
                "blake3": hash_bytes(&bytes),
                "kind": kind,
                "role": role,
            }))
        })
        .collect()
}

pub(super) fn hash_output_document(root: &Path, relative: &str) -> Result<String, String> {
    let path = root.join(path_from_forward_slashes(relative));
    let bytes =
        fs::read(&path).map_err(|err| format!("could not hash {}: {err}", path.display()))?;
    Ok(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
}

pub(super) fn hash_bytes(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}

pub(super) fn valid_tile_component(value: &str) -> bool {
    value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_digit())
}

pub(super) fn required_json_str<'a>(
    value: &'a JsonValue,
    field: &str,
    label: &str,
) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("{label} has no string {field}"))
}

pub(super) fn hard_link_tree_exact(
    source_root: &Path,
    destination_root: &Path,
) -> Result<(usize, u64), String> {
    if !source_root.is_dir() || !destination_root.is_dir() {
        return Err(
            "hard-link clone requires existing source and destination directories".to_string(),
        );
    }
    let source_root = fs::canonicalize(source_root)
        .map_err(|err| format!("could not canonicalize {}: {err}", source_root.display()))?;
    let destination_root = fs::canonicalize(destination_root).map_err(|err| {
        format!(
            "could not canonicalize {}: {err}",
            destination_root.display()
        )
    })?;
    if destination_root.starts_with(&source_root) || source_root.starts_with(&destination_root) {
        return Err(
            "source and destination hard-link trees must not contain one another".to_string(),
        );
    }
    let mut pending = vec![(source_root.clone(), destination_root.clone())];
    let mut file_count = 0usize;
    let mut byte_count = 0u64;
    while let Some((source_directory, destination_directory)) = pending.pop() {
        let mut entries = fs::read_dir(&source_directory)
            .map_err(|err| format!("could not enumerate {}: {err}", source_directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("could not enumerate {}: {err}", source_directory.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let source = entry.path();
            let metadata = fs::symlink_metadata(&source)
                .map_err(|err| format!("could not inspect {}: {err}", source.display()))?;
            if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
                return Err(format!(
                    "refusing symlink/reparse point in native terrain batch: {}",
                    source.display()
                ));
            }
            let destination = destination_directory.join(entry.file_name());
            if metadata.is_dir() {
                fs::create_dir(&destination)
                    .map_err(|err| format!("could not create {}: {err}", destination.display()))?;
                pending.push((source, destination));
            } else if metadata.is_file() {
                fs::hard_link(&source, &destination).map_err(|err| {
                    format!(
                        "could not hard-link immutable payload {} -> {}: {err}",
                        source.display(),
                        destination.display()
                    )
                })?;
                file_count += 1;
                byte_count = byte_count
                    .checked_add(metadata.len())
                    .ok_or_else(|| "hard-linked byte count overflowed u64".to_string())?;
            } else {
                return Err(format!(
                    "refusing non-file/non-directory batch entry: {}",
                    source.display()
                ));
            }
        }
    }
    Ok((file_count, byte_count))
}

#[cfg(windows)]
pub(super) fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub(super) fn metadata_is_reparse_point(_: &fs::Metadata) -> bool {
    false
}

pub(super) fn replace_json_hardlink_safe(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("{} has no UTF-8 filename", path.display()))?;
    let nonce = unique_nonce()?;
    let replacement = parent.join(format!(
        ".{file_name}.replace-{}-{nonce:x}",
        std::process::id()
    ));
    write_json_new(&replacement, value)?;
    fs::remove_file(path)
        .map_err(|err| format!("could not unlink hard-linked {}: {err}", path.display()))?;
    fs::rename(&replacement, path).map_err(|err| {
        format!(
            "could not install replacement {} -> {}: {err}",
            replacement.display(),
            path.display()
        )
    })
}

pub(super) fn blocker_sort_key(value: &JsonValue) -> (String, String, String, String, String) {
    (
        value
            .get("scope")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string(),
        value
            .get("tileId")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string(),
        value
            .get("stage")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string(),
        value
            .get("code")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string(),
        value
            .get("message")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string(),
    )
}

pub(super) fn blocked_tile(
    source: &TileSource,
    stage: &'static str,
    code: &'static str,
    message: String,
) -> BatchBlocked {
    BatchBlocked {
        scope: Some(source.scope.label()),
        tile_id: Some(source.tile_id.clone()),
        source_path: Some(source.resource.path.clone()),
        stage,
        code,
        message,
    }
}

pub(super) fn canonical_string(path: &Path) -> Result<String, String> {
    fs::canonicalize(path)
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|err| format!("could not canonicalize {}: {err}", path.display()))
}

pub(super) fn unique_nonce() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .map_err(|err| err.to_string())
}
