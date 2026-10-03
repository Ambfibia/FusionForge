use super::*;

pub(super) fn reject_stale_transactions(asset_root: &Path) -> Result<()> {
    let entries = fs::read_dir(asset_root)
        .map_err(|error| io_at(asset_root, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(asset_root, error))?;
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".semantic-audio-stage-")
            || name.starts_with(".semantic-audio-backup-")
            || matches!(
                name.as_str(),
                ".asset-manifest.semantic-audio.next" | ".asset-manifest.semantic-audio.backup"
            )
        {
            return invalid(format!(
                "stale semantic-audio transaction artifact exists at {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_source_build(source_build: &str) -> Result<()> {
    if source_build.trim().is_empty()
        || source_build.len() > 256
        || source_build.chars().any(char::is_control)
    {
        return invalid("source build is empty, too long, or contains a control character");
    }
    Ok(())
}

pub(super) fn validate_mapping(mapping: &CookMapping) -> Result<()> {
    if mapping.name.trim().is_empty()
        || mapping.name.len() > 240
        || mapping.name.chars().any(char::is_control)
    {
        return invalid(format!(
            "audio mapping {:?} has an invalid true m_Name",
            mapping.native_path
        ));
    }
    validate_hash(&mapping.native_key, "audio mapping nativeKey")?;
    if !is_hashed_audio_path(&mapping.native_path) {
        return invalid(format!(
            "audio mapping nativePath is not an original hash-addressed OGG: {:?}",
            mapping.native_path
        ));
    }
    if mapping.source.is_empty() {
        return invalid(format!(
            "audio mapping {:?} has no source provenance",
            mapping.native_path
        ));
    }
    Ok(())
}

pub(super) fn validate_hash(value: &str, context: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return invalid(format!(
            "{context} must be 64 lowercase hexadecimal characters"
        ));
    }
    Ok(())
}

pub(super) fn validate_hash_suffix(entry: &ProjectAssetFile) -> Result<()> {
    let file = entry
        .path
        .strip_prefix("audio/")
        .and_then(|path| path.strip_suffix(".ogg"))
        .ok_or_else(|| invalid_error("original audio path shape changed after selection"))?;
    let suffix = file
        .rsplit_once("--")
        .map(|(_, suffix)| suffix)
        .ok_or_else(|| invalid_error("original audio path has no hash suffix"))?;
    if !entry.blake3.starts_with(suffix) {
        return invalid(format!(
            "manifest path hash suffix does not match blake3 for {:?}",
            entry.path
        ));
    }
    Ok(())
}

pub(super) fn validate_relative_semantic_path(path: &str) -> Result<()> {
    if path.contains('\\') || !path.starts_with("audio/") || !path.ends_with(".ogg") {
        return invalid(format!("invalid semantic audio destination {path:?}"));
    }
    let parsed = Path::new(path);
    if parsed.is_absolute()
        || parsed
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!(
            "semantic audio destination is not a strict relative path: {path:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_manifest_destination_closure(
    manifest: &ProjectAssetManifest,
    prepared: &[PreparedAudio],
) -> Result<()> {
    let mut occupied = manifest
        .files
        .iter()
        .filter(|entry| !is_owned_entry(entry))
        .map(|entry| (canonical_path_identity(&entry.path), entry.path.clone()))
        .collect::<BTreeMap<_, _>>();
    for audio in prepared {
        let identity = canonical_path_identity(&audio.destination);
        if let Some(existing) = occupied.insert(identity, audio.destination.clone()) {
            return invalid(format!(
                "semantic audio destination {:?} collides with existing manifest path {existing:?}",
                audio.destination
            ));
        }
    }
    let catalog_identity = canonical_path_identity(SEMANTIC_AUDIO_CATALOG);
    if let Some(existing) = occupied.insert(catalog_identity, SEMANTIC_AUDIO_CATALOG.to_owned()) {
        return invalid(format!(
            "semantic audio catalog collides with existing manifest path {existing:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_staged_files(stage: &Path, entries: &[ProjectAssetFile]) -> Result<()> {
    let mut expected = BTreeMap::new();
    for entry in entries {
        let relative = if entry.path == SEMANTIC_AUDIO_CATALOG {
            "catalog.json"
        } else {
            entry
                .path
                .strip_prefix("audio/")
                .ok_or_else(|| invalid_error("staged manifest path is outside audio/"))?
        };
        expected.insert(
            canonical_path_identity(relative),
            (relative.to_owned(), entry.bytes, entry.blake3.clone()),
        );
    }

    let files = collect_files(stage)?;
    if files.len() != expected.len() {
        return invalid(format!(
            "staged semantic audio file count mismatch: expected {}, found {}",
            expected.len(),
            files.len()
        ));
    }
    for relative in files {
        let identity = canonical_path_identity(&relative);
        let Some((expected_path, expected_bytes, expected_hash)) = expected.remove(&identity)
        else {
            return invalid(format!(
                "unexpected staged semantic audio file {relative:?}"
            ));
        };
        if expected_path != relative {
            return invalid(format!(
                "case/normalization alias in staged path: expected {expected_path:?}, got {relative:?}"
            ));
        }
        let path = join_relative(stage, &relative)?;
        let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
        let actual_hash = blake3::hash(&bytes).to_hex().to_string();
        if bytes.len() as u64 != expected_bytes || actual_hash != expected_hash {
            return invalid(format!("staged byte identity mismatch for {relative:?}"));
        }
    }
    if !expected.is_empty() {
        return invalid("one or more expected semantic audio files were not staged");
    }
    Ok(())
}

pub(super) fn validate_manifest_paths(manifest: &ProjectAssetManifest) -> Result<()> {
    let mut seen = BTreeMap::<String, String>::new();
    for entry in &manifest.files {
        let identity = canonical_path_identity(&entry.path);
        if let Some(first) = seen.insert(identity, entry.path.clone()) {
            return invalid(format!(
                "project manifest path collision after semantic audio install: {first:?} and {:?}",
                entry.path
            ));
        }
    }
    Ok(())
}

pub(super) fn invalid_error(reason: impl Into<String>) -> PipelineError {
    PipelineError::UnsupportedAsset {
        path: "audio".to_owned(),
        reason: reason.into(),
    }
}
