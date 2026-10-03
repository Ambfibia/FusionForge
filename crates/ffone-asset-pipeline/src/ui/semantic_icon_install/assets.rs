use super::*;

pub const SEMANTIC_ICON_CATALOG_SCHEMA: &str = "ffone.semantic-icons.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticIconSourceAsset {
    pub manifest_path: String,
    pub manifest_source_path: String,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticIconAsset {
    pub key: String,
    pub legacy_true_name: String,
    pub legacy_icon_number: u32,
    pub path: String,
    pub source: SemanticIconSourceAsset,
    pub classification: SemanticIconClassificationProof,
    pub table_references: Vec<SemanticIconTableReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticIconCatalogProofs {
    pub source_build: String,
    pub source_manifest_path: String,
    pub source_manifest_schema: String,
    pub source_manifest_without_owned_icons_blake3: String,
    pub table_set_path: String,
    pub table_set_manifest_source_path: String,
    pub table_set_schema: String,
    pub table_set_bytes: u64,
    pub table_set_blake3: String,
    pub classification_policy: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticIconCatalogCounts {
    pub raw_table_references: u64,
    pub unique_table_icons: u64,
    pub published: u64,
    pub unmatched: u64,
    pub missing: u64,
    pub ambiguous: u64,
    pub unsupported_types: u64,
    pub unreferenced_legacy_textures: u64,
    pub name_only_unclassified: u64,
    pub by_category: BTreeMap<SemanticIconCategory, u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticIconCatalog {
    pub schema: &'static str,
    pub proofs: SemanticIconCatalogProofs,
    pub counts: SemanticIconCatalogCounts,
    pub assets: Vec<SemanticIconAsset>,
}

pub(super) fn canonical_source_manifest_hash(
    manifest: &ProjectAssetManifest,
    manifest_path: &Path,
) -> Result<String> {
    let mut source = manifest.clone();
    source
        .files
        .retain(|entry| !entry.path.starts_with("icons/"));
    source
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    let bytes = serde_json::to_vec(&source).map_err(|source| PipelineError::Json {
        path: manifest_path.display().to_string(),
        source,
    })?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

pub(super) fn manifest_relative_path(asset_root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(asset_root).map_err(|_| {
        invalid_error(format!(
            "{} must be inside ASSET_ROOT {}",
            path.display(),
            asset_root.display()
        ))
    })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_string_lossy().into_owned()),
            _ => return invalid("manifest paths may contain only normal path components"),
        }
    }
    if parts.is_empty() {
        return invalid("manifest path must not be empty");
    }
    Ok(parts.join("/"))
}

pub(super) fn join_manifest_path(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative.contains('\\') {
        return invalid(format!(
            "manifest path must use forward slashes: {relative:?}"
        ));
    }
    let mut output = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(value) => output.push(value),
            _ => return invalid(format!("unsafe manifest path {relative:?}")),
        }
    }
    Ok(output)
}

pub(super) fn replace_manifest(path: &Path, manifest: &ProjectAssetManifest) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("asset manifest has no parent"))?;
    let next = parent.join(".asset-manifest.semantic-icons.next");
    let backup = parent.join(".asset-manifest.semantic-icons.backup");
    if next.exists() || backup.exists() {
        return invalid(format!(
            "stale semantic-icon manifest transaction file at {} or {}",
            next.display(),
            backup.display()
        ));
    }
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })?;
    bytes.push(b'\n');
    fs::write(&next, bytes).map_err(|error| io_at(&next, error))?;
    fs::rename(path, &backup).map_err(|error| io_at(path, error))?;
    if let Err(error) = fs::rename(&next, path) {
        let _ = fs::rename(&backup, path);
        let _ = fs::remove_file(&next);
        return Err(io_at(path, error));
    }
    fs::remove_file(&backup).map_err(|error| io_at(&backup, error))
}
