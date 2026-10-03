use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct LauncherManifest {
    pub(super) uuid: String,
    pub(super) main_file_info: LauncherFileInfo,
    #[serde(default)]
    pub(super) bundles: BTreeMap<String, LauncherBundleInfo>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct NativeAssetEntry {
    pub(super) key: String,
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FontIndexEntry {
    pub(super) key: String,
    pub(super) family: String,
    pub(super) asset_key: String,
    pub(super) include_russian: bool,
    pub(super) replace_ascii: bool,
    pub(super) vertical_offset: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) target_key: Option<String>,
}

pub(super) struct NativeAssetIdentity {
    pub(super) key: String,
    pub(super) display_name: String,
    pub(super) safe_name: String,
    pub(super) content_hash: String,
}

pub(super) struct EmittedAsset {
    pub(super) key: String,
    pub(super) name: String,
    pub(super) path: String,
}

pub(super) fn resolve_launcher_manifest(input: &Path) -> Result<PathBuf, String> {
    if input.is_file() {
        if !input
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("json"))
        {
            return Err(format!(
                "launcher manifest must be a .json file: {}",
                input.display()
            ));
        }
        let metadata =
            fs::symlink_metadata(input).map_err(|err| format!("{}: {err}", input.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "launcher manifest cannot be a symlink: {}",
                input.display()
            ));
        }
        return fs::canonicalize(input).map_err(|err| format!("{}: {err}", input.display()));
    }
    if !input.is_dir() {
        return Err(format!(
            "manifest path is neither file nor directory: {}",
            input.display()
        ));
    }
    let mut candidates = fs::read_dir(input)
        .map_err(|err| format!("{}: {err}", input.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("json"))
                && path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .is_some_and(is_uuid)
        })
        .collect::<Vec<_>>();
    candidates.sort();
    if candidates.len() != 1 {
        return Err(format!(
            "{} must contain exactly one UUID-named .json launcher manifest; found {}",
            input.display(),
            candidates.len()
        ));
    }
    fs::canonicalize(&candidates[0]).map_err(|err| format!("{}: {err}", candidates[0].display()))
}

pub(super) fn declared_overlay_manifest(
    path: &Path,
    declared: bool,
    label: &str,
) -> Result<Option<PathBuf>, String> {
    if path.is_file() {
        return ensure_contained_regular_file(
            path.parent().unwrap_or_else(|| Path::new(".")),
            path,
        )
        .map(Some);
    }
    if declared {
        return Err(format!(
            "declared {label} overlay is missing: {}",
            path.display()
        ));
    }
    Ok(None)
}

pub(super) fn native_asset_identity(
    kind: ContentKind,
    semantic_name: &str,
    bytes: &[u8],
) -> NativeAssetIdentity {
    let kind_name = content_kind_name(kind);
    let display_name = neutral_semantic_label(semantic_name, kind_name);
    let safe_name = semantic_safe(&display_name);
    let content_hash = blake3_hex(bytes);
    let key = stable_key(
        "ffone.asset.v1",
        &[
            kind_name.as_bytes(),
            display_name.as_bytes(),
            content_hash.as_bytes(),
        ],
    );
    NativeAssetIdentity {
        key,
        display_name,
        safe_name,
        content_hash,
    }
}

pub(super) fn cook_report_path(output: &Path) -> Result<PathBuf, String> {
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("invalid output path: {}", output.display()))?;
    Ok(output.with_file_name(format!("{name}.cook-report.json")))
}
