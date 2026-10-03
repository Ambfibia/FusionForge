use super::*;

pub const TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH: &str =
    "world/tutorial/static/install-manifest.json";

pub const WORLD_MAP_STATIC_WORLD_OWNERSHIP_PATH: &str = "world/maps/static/install-manifest.json";

pub(super) const EXPORT_MANIFEST_SCHEMA: &str = "ffone.native-static-world-manifest.v1";

pub(super) const EXPORT_MANIFEST_FILE: &str = "export-manifest.json";

pub(super) const STATIC_CATALOG_SCHEMA: &str = "ffone.native-static-world-catalog.v1";

pub(super) const WORLD_CATALOG_PATH: &str = "world/catalog.json";

pub(super) const WORLD_CATALOG_SCHEMA: &str = "ffone.semantic-world-catalog.v2";

pub(super) const RUNTIME_WORLD_REGISTRY_PATH: &str = "_runtime/world.json";

pub(super) const RUNTIME_WORLD_REGISTRY_SCHEMA: &str = "ffone.runtime-world.v1";

pub(super) const CLEANUP_REVISION_INDEX_SCHEMA: &str = "ffone.conversion-metadata-revision-index.v1";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ExportManifest {
    pub(super) counts: ExportManifestCounts,
    pub(super) files: Vec<ExportManifestFile>,
    pub(super) schema: String,
    pub(super) self_excluded: String,
    pub(super) source_build: String,
    pub(super) tile_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ExportManifestCounts {
    pub(super) bytes: u64,
    pub(super) files: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ExportManifestFile {
    pub(super) blake3: String,
    pub(super) byte_length: u64,
    pub(super) kind: String,
    pub(super) path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(dead_code, reason = "fields are kept for strict schema validation (deny_unknown_fields)")]
pub(super) struct WorldCatalogBlocker {
    pub(super) scope: String,
    pub(super) instance_id: String,
    pub(super) stage: String,
    pub(super) code: String,
    pub(super) message: String,
    pub(super) placement_status: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(dead_code, reason = "fields are kept for strict schema validation (deny_unknown_fields)")]
pub(super) struct WorldCatalogEnvironment {
    pub(super) schema: String,
    pub(super) status: String,
    pub(super) path: String,
    pub(super) blake3: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(dead_code, reason = "fields are kept for strict schema validation (deny_unknown_fields)")]
pub(super) struct WorldCatalogEntry {
    pub(super) scope: WorldReferenceScope,
    pub(super) instance_id: String,
    pub(super) tile: [i32; 2],
    pub(super) placement_status: String,
    pub(super) scene: Option<String>,
    #[serde(default)]
    pub(super) scene_blake3: Option<String>,
    pub(super) terrain_descriptor: String,
    pub(super) terrain_descriptor_blake3: String,
    pub(super) provenance: String,
    #[serde(default)]
    pub(super) environment: Option<WorldCatalogEnvironment>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(dead_code, reason = "fields are kept for strict schema validation (deny_unknown_fields)")]
pub(super) struct WorldCatalog {
    pub(super) schema: String,
    pub(super) source_build: String,
    pub(super) status: String,
    pub(super) terrain_glb_allowed: bool,
    pub(super) entries: Vec<WorldCatalogEntry>,
    pub(super) blocked: Vec<WorldCatalogBlocker>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeWorldAssetReference {
    pub(super) path: String,
    pub(super) blake3: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeWorldRegistryEntry {
    pub(super) id: String,
    pub(super) scope: WorldReferenceScope,
    pub(super) tile: [i32; 2],
    pub(super) scene: RuntimeWorldAssetReference,
    pub(super) terrain: RuntimeWorldAssetReference,
    #[serde(default)]
    pub(super) environment: Option<RuntimeWorldAssetReference>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeWorldRegistry {
    pub(super) schema: String,
    pub(super) entries: Vec<RuntimeWorldRegistryEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CleanupArchiveIndex {
    pub(super) schema: String,
    pub(super) source_build: String,
    #[serde(default)]
    pub(super) plan_blake3: Option<String>,
    pub(super) files: Vec<ArchivedRuntimeMetadata>,
}

pub(super) fn rewrite_world_catalog(
    bytes: &[u8],
    path: &Path,
    expected: &BTreeMap<String, ExpectedSceneReference>,
    current_scene_blake3: &BTreeMap<String, String>,
    allow_legacy_stale_hashes: bool,
) -> Result<(Vec<u8>, u64)> {
    let catalog: WorldCatalog = parse_json(bytes, path)?;
    if catalog.schema != WORLD_CATALOG_SCHEMA
        || catalog.source_build != TUTORIAL_STATIC_WORLD_SOURCE_BUILD
        || catalog.status.trim().is_empty()
        || catalog.terrain_glb_allowed
    {
        return invalid("native world catalog header is outside the tutorial installer contract");
    }
    let mut value: JsonValue = parse_json(bytes, path)?;
    let entries = value
        .get_mut("entries")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("native world catalog entries is not an array"))?;
    if entries.len() != catalog.entries.len() {
        return invalid("native world catalog typed/raw entry count differs");
    }

    let mut seen = BTreeSet::new();
    let mut changed = false;
    for (index, entry) in catalog.entries.iter().enumerate() {
        let Some(scene_path) = entry.scene.as_deref() else {
            continue;
        };
        let Some(reference) = expected.get(scene_path) else {
            continue;
        };
        if entry.scope != StaticWorldScope::of_tile(&reference.tile_id)?.world_reference_scope()
            || entry.instance_id != reference.tile_id
            || entry.tile != reference.tile
            || entry.placement_status != "linked"
        {
            return invalid(format!(
                "native world catalog identity differs for {scene_path:?}"
            ));
        }
        if !seen.insert(scene_path.to_owned()) {
            return invalid(format!(
                "native world catalog contains duplicate tutorial scene {scene_path:?}"
            ));
        }
        let observed = entry.scene_blake3.as_deref().ok_or_else(|| {
            invalid_error(format!(
                "native world catalog lacks sceneBlake3 for {scene_path:?}"
            ))
        })?;
        validate_observed_scene_hash(
            WORLD_CATALOG_PATH,
            scene_path,
            observed,
            current_scene_blake3,
            allow_legacy_stale_hashes,
        )?;
        changed |= observed != reference.blake3;
        entries[index]
            .as_object_mut()
            .ok_or_else(|| invalid_error("native world catalog entry is not an object"))?
            .insert(
                "sceneBlake3".to_owned(),
                JsonValue::String(reference.blake3.clone()),
            );
    }
    require_complete_reference_coverage(WORLD_CATALOG_PATH, expected, &seen)?;
    let next_bytes = if changed {
        pretty_json(&value)?
    } else {
        bytes.to_vec()
    };
    Ok((next_bytes, seen.len() as u64))
}

pub(super) fn rewrite_runtime_world_registry(
    bytes: &[u8],
    path: &Path,
    expected: &BTreeMap<String, ExpectedSceneReference>,
    current_scene_blake3: &BTreeMap<String, String>,
    allow_legacy_stale_hashes: bool,
) -> Result<(Vec<u8>, u64)> {
    let registry: RuntimeWorldRegistry = parse_json(bytes, path)?;
    if registry.schema != RUNTIME_WORLD_REGISTRY_SCHEMA || registry.entries.is_empty() {
        return invalid("runtime world registry header is outside the tutorial installer contract");
    }
    let mut value: JsonValue = parse_json(bytes, path)?;
    let entries = value
        .get_mut("entries")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("runtime world registry entries is not an array"))?;
    if entries.len() != registry.entries.len() {
        return invalid("runtime world registry typed/raw entry count differs");
    }

    let mut seen = BTreeSet::new();
    let mut changed = false;
    for (index, entry) in registry.entries.iter().enumerate() {
        let scene_path = entry.scene.path.as_str();
        let Some(reference) = expected.get(scene_path) else {
            continue;
        };
        if entry.scope != StaticWorldScope::of_tile(&reference.tile_id)?.world_reference_scope()
            || entry.id != reference.tile_id
            || entry.tile != reference.tile
        {
            return invalid(format!(
                "runtime world registry identity differs for {scene_path:?}"
            ));
        }
        if !seen.insert(scene_path.to_owned()) {
            return invalid(format!(
                "runtime world registry contains duplicate tutorial scene {scene_path:?}"
            ));
        }
        validate_observed_scene_hash(
            RUNTIME_WORLD_REGISTRY_PATH,
            scene_path,
            &entry.scene.blake3,
            current_scene_blake3,
            allow_legacy_stale_hashes,
        )?;
        changed |= entry.scene.blake3 != reference.blake3;
        entries[index]
            .get_mut("scene")
            .and_then(JsonValue::as_object_mut)
            .ok_or_else(|| invalid_error("runtime world registry scene is not an object"))?
            .insert(
                "blake3".to_owned(),
                JsonValue::String(reference.blake3.clone()),
            );
    }
    require_complete_reference_coverage(RUNTIME_WORLD_REGISTRY_PATH, expected, &seen)?;
    let next_bytes = if changed {
        pretty_json(&value)?
    } else {
        bytes.to_vec()
    };
    Ok((next_bytes, seen.len() as u64))
}

pub(super) fn move_path(from_root: &Path, backup: &Path, relative: &str) -> Result<()> {
    let source = safe_join(from_root, relative)?;
    if !source.exists() {
        return invalid(format!(
            "transaction source disappeared: {}",
            source.display()
        ));
    }
    reject_symlink(&source, "transaction source")?;
    let target = safe_join(backup, relative)?;
    if target.exists() {
        return invalid(format!(
            "transaction backup collision: {}",
            target.display()
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    fs::rename(&source, &target).map_err(|error| io_at(&source, error))
}

pub(super) fn publish_path(stage: &Path, asset_root: &Path, relative: &str) -> Result<()> {
    let source = safe_join(stage, relative)?;
    let target = safe_join(asset_root, relative)?;
    if target.exists() {
        return invalid(format!(
            "tutorial static-world destination appeared during publication: {}",
            target.display()
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    fs::rename(&source, &target).map_err(|error| io_at(&source, error))
}

pub(super) fn verify_manifest_file(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    relative: &str,
    label: &str,
) -> Result<Vec<u8>> {
    let entry = project_entry(manifest, relative)?;
    let path = safe_join(asset_root, relative)?;
    let bytes = read_regular_file(&path, label)?;
    if bytes.len() as u64 != entry.bytes || hash_bytes(&bytes) != entry.blake3 {
        return invalid(format!(
            "{label} differs from project manifest at {relative:?}"
        ));
    }
    Ok(bytes)
}

pub(super) fn tile_scene_path(tile_id: &str) -> String {
    match StaticWorldScope::of_tile(tile_id) {
        Ok(StaticWorldScope::WorldMap) => format!("world/maps/{tile_id}/scene.json"),
        _ => format!("world/tutorial/terrain/tiles/{tile_id}/scene.json"),
    }
}

pub(super) fn static_metadata_path(tile_id: &str, file: &str) -> String {
    let root = StaticWorldScope::of_tile(tile_id)
        .unwrap_or(StaticWorldScope::Tutorial)
        .static_root();
    format!("{root}/tiles/{tile_id}/{file}")
}

pub(super) fn relative_path_string(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(component) = component else {
            return invalid(format!(
                "path is not relative and normalized: {}",
                path.display()
            ));
        };
        parts.push(
            component
                .to_str()
                .ok_or_else(|| invalid_error("path is not valid UTF-8"))?,
        );
    }
    Ok(parts.join("/"))
}
