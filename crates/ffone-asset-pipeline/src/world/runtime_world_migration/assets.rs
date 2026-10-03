use super::*;

pub const RUNTIME_WORLD_REGISTRY_SCHEMA: &str = "ffone.runtime-world.v1";

pub const RUNTIME_WORLD_REGISTRY_PATH: &str = "_runtime/world.json";

pub const LEGACY_WORLD_CATALOG_PATH: &str = "world/catalog.json";

pub const RUNTIME_WORLD_ARCHIVE_INDEX_SCHEMA: &str = "ffone.runtime-world-conversion-archive.v1";

pub(super) const LEGACY_WORLD_CATALOG_SCHEMA: &str = "ffone.semantic-world-catalog.v2";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWorldRegistryEntry {
    pub id: String,
    pub scope: String,
    pub tile: [i32; 2],
    pub scene: RuntimeWorldContentReference,
    pub terrain: RuntimeWorldContentReference,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<RuntimeWorldContentReference>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWorldRegistry {
    pub schema: String,
    pub entries: Vec<RuntimeWorldRegistryEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyWorldCatalogHashDrift {
    pub instance_id: String,
    pub payload: String,
    pub path: String,
    pub catalog_blake3: String,
    pub verified_source_blake3: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWorldRegistryArtifact {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeWorldArchiveIndex<'a> {
    pub(super) schema: &'static str,
    pub(super) source_build: &'a str,
    pub(super) technical_files: &'a [ArchivedWorldTechnicalMetadata],
    pub(super) rewritten_originals: &'a [RewrittenRuntimeWorldMetadata],
    pub(super) legacy_catalog_hash_drifts: &'a [LegacyWorldCatalogHashDrift],
    pub(super) runtime_registry: &'a RuntimeWorldRegistryArtifact,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CompletedRuntimeWorldArchiveIndex {
    pub(super) schema: String,
    pub(super) source_build: String,
    pub(super) technical_files: Vec<ArchivedWorldTechnicalMetadata>,
    pub(super) rewritten_originals: Vec<RewrittenRuntimeWorldMetadata>,
    pub(super) legacy_catalog_hash_drifts: Vec<LegacyWorldCatalogHashDrift>,
    pub(super) runtime_registry: RuntimeWorldRegistryArtifact,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LegacyWorldCatalog {
    pub(super) schema: String,
    pub(super) entries: Vec<LegacyWorldCatalogEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LegacyWorldCatalogEntry {
    pub(super) instance_id: String,
    pub(super) placement_status: String,
    pub(super) provenance: String,
    pub(super) scene: Option<String>,
    pub(super) scene_blake3: Option<String>,
    pub(super) scope: String,
    pub(super) terrain_descriptor: String,
    pub(super) terrain_descriptor_blake3: String,
    pub(super) tile: [i32; 2],
    pub(super) environment: LegacyWorldEnvironmentReference,
}

pub(super) fn manifest_index<'a>(
    manifest: &'a ProjectAssetManifest,
) -> Result<BTreeMap<&'a str, Vec<&'a ProjectAssetFile>>> {
    let mut index = BTreeMap::<&str, Vec<&ProjectAssetFile>>::new();
    for entry in &manifest.files {
        index.entry(entry.path.as_str()).or_default().push(entry);
    }
    if let Some((path, _)) = index.iter().find(|(_, entries)| entries.len() != 1) {
        return invalid(format!("manifest repeats path {path:?}"));
    }
    Ok(index)
}

pub(super) fn verified_manifest_bytes(
    asset_root: &Path,
    manifest: &BTreeMap<&str, Vec<&ProjectAssetFile>>,
    relative: &str,
    expected_blake3: Option<&str>,
) -> Result<Vec<u8>> {
    let entries = manifest
        .get(relative)
        .ok_or_else(|| invalid_error(format!("manifest does not list {relative:?}")))?;
    let [entry] = entries.as_slice() else {
        return invalid(format!("manifest does not list {relative:?} exactly once"));
    };
    if entry.kind != ProjectAssetKind::Data {
        return invalid(format!(
            "world JSON {relative:?} is {:?}, expected data",
            entry.kind
        ));
    }
    let path = asset_root.join(native_path(relative)?);
    let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if entry.bytes != bytes.len() as u64 || entry.blake3 != actual {
        return invalid(format!(
            "world JSON manifest identity mismatch for {relative:?}"
        ));
    }
    if expected_blake3.is_some_and(|expected| expected != actual) {
        return invalid(format!("world catalog hash mismatch for {relative:?}"));
    }
    Ok(bytes)
}

pub(super) fn native_path(relative: &str) -> Result<PathBuf> {
    if relative.is_empty() || relative.contains('\\') || relative.starts_with('/') {
        return invalid(format!("unsafe relative asset path {relative:?}"));
    }
    let mut native = PathBuf::new();
    for segment in relative.split('/') {
        if segment.is_empty() || matches!(segment, "." | "..") {
            return invalid(format!("unsafe relative asset path {relative:?}"));
        }
        native.push(segment);
    }
    Ok(native)
}
