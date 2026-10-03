use super::*;

pub(super) fn required_original_hash<'a>(
    original_hashes: &'a BTreeMap<String, String>,
    path: &str,
) -> Result<&'a str> {
    original_hashes
        .get(path)
        .map(String::as_str)
        .ok_or_else(|| invalid_error(format!("completed archive omits original payload {path:?}")))
}

pub(super) fn push_expected_hash_drift(
    drifts: &mut Vec<LegacyWorldCatalogHashDrift>,
    entry: &LegacyWorldCatalogEntry,
    payload: &str,
    path: &str,
    catalog_blake3: &str,
    verified_source_blake3: &str,
) {
    if catalog_blake3 != verified_source_blake3 {
        drifts.push(LegacyWorldCatalogHashDrift {
            instance_id: entry.instance_id.clone(),
            payload: payload.to_owned(),
            path: path.to_owned(),
            catalog_blake3: catalog_blake3.to_owned(),
            verified_source_blake3: verified_source_blake3.to_owned(),
        });
    }
}

pub(super) fn sort_hash_drifts(drifts: &mut [LegacyWorldCatalogHashDrift]) {
    drifts.sort_by(|left, right| {
        (
            left.instance_id.as_str(),
            left.payload.as_str(),
            left.path.as_str(),
        )
            .cmp(&(
                right.instance_id.as_str(),
                right.payload.as_str(),
                right.path.as_str(),
            ))
    });
}

pub(super) fn build_plan(asset_root: PathBuf, archive_root: PathBuf) -> Result<RuntimeWorldMigrationPlan> {
    if archive_root.exists() {
        return invalid(format!(
            "world conversion archive already exists: {}",
            archive_root.display()
        ));
    }
    reject_stale_transactions(&asset_root, archive_root.parent())?;

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid(format!(
            "unsupported project-asset manifest schema {:?}",
            manifest.schema
        ));
    }
    let manifest_by_path = manifest_index(&manifest)?;
    if manifest_by_path.contains_key(RUNTIME_WORLD_REGISTRY_PATH) {
        return invalid(format!(
            "{RUNTIME_WORLD_REGISTRY_PATH} is already present; refusing to replace a runtime registry"
        ));
    }

    let catalog_bytes = verified_manifest_bytes(
        &asset_root,
        &manifest_by_path,
        LEGACY_WORLD_CATALOG_PATH,
        None,
    )?;
    let catalog: LegacyWorldCatalog =
        serde_json::from_slice(&catalog_bytes).map_err(|source| PipelineError::Json {
            path: asset_root
                .join(LEGACY_WORLD_CATALOG_PATH)
                .display()
                .to_string(),
            source,
        })?;
    if catalog.schema != LEGACY_WORLD_CATALOG_SCHEMA {
        return invalid(format!(
            "legacy world catalog schema is {:?}, expected {LEGACY_WORLD_CATALOG_SCHEMA:?}",
            catalog.schema
        ));
    }

    let mut environment_rewrites = BTreeMap::<String, PreparedRuntimeRewrite>::new();
    let mut terrain_rewrites = BTreeMap::<String, PreparedRuntimeRewrite>::new();
    let mut scene_rewrites = BTreeMap::<String, PreparedRuntimeRewrite>::new();
    let mut registry_entries = Vec::new();
    let mut excluded_blocked_entries = Vec::new();
    let mut legacy_catalog_hash_drifts = Vec::new();
    let mut identities = BTreeSet::new();
    for entry in catalog.entries {
        validate_catalog_identity(&entry, &mut identities)?;

        let environment_original = verified_manifest_bytes(
            &asset_root,
            &manifest_by_path,
            &entry.environment.path,
            None,
        )?;
        let environment_source_hash = blake3::hash(&environment_original).to_hex().to_string();
        if environment_source_hash != entry.environment.blake3 {
            legacy_catalog_hash_drifts.push(LegacyWorldCatalogHashDrift {
                instance_id: entry.instance_id.clone(),
                payload: "environment".to_owned(),
                path: entry.environment.path.clone(),
                catalog_blake3: entry.environment.blake3.clone(),
                verified_source_blake3: environment_source_hash,
            });
        }
        let (environment_bytes, environment_removed) =
            sanitize_environment(&entry.environment.path, &environment_original)?;
        let environment_runtime_hash = blake3::hash(&environment_bytes).to_hex().to_string();
        insert_rewrite(
            &mut environment_rewrites,
            prepared_rewrite(
                &entry.environment.path,
                &environment_original,
                environment_bytes,
                environment_removed,
            ),
        )?;

        let terrain_original = verified_manifest_bytes(
            &asset_root,
            &manifest_by_path,
            &entry.terrain_descriptor,
            None,
        )?;
        let terrain_source_hash = blake3::hash(&terrain_original).to_hex().to_string();
        if terrain_source_hash != entry.terrain_descriptor_blake3 {
            legacy_catalog_hash_drifts.push(LegacyWorldCatalogHashDrift {
                instance_id: entry.instance_id.clone(),
                payload: "terrain".to_owned(),
                path: entry.terrain_descriptor.clone(),
                catalog_blake3: entry.terrain_descriptor_blake3.clone(),
                verified_source_blake3: terrain_source_hash,
            });
        }
        let (terrain_bytes, terrain_removed) = sanitize_terrain(
            &entry.terrain_descriptor,
            &terrain_original,
            &entry.environment.path,
            &environment_runtime_hash,
        )?;
        let terrain_runtime_hash = blake3::hash(&terrain_bytes).to_hex().to_string();
        insert_rewrite(
            &mut terrain_rewrites,
            prepared_rewrite(
                &entry.terrain_descriptor,
                &terrain_original,
                terrain_bytes,
                terrain_removed,
            ),
        )?;

        match entry.placement_status.as_str() {
            "linked" => {
                let scene_path = entry.scene.as_deref().ok_or_else(|| {
                    invalid_error(format!(
                        "linked world entry {:?} has no scene path",
                        entry.instance_id
                    ))
                })?;
                let scene_expected = entry.scene_blake3.as_deref().ok_or_else(|| {
                    invalid_error(format!(
                        "linked world entry {:?} has no scene hash",
                        entry.instance_id
                    ))
                })?;
                let scene_original =
                    verified_manifest_bytes(&asset_root, &manifest_by_path, scene_path, None)?;
                let scene_source_hash = blake3::hash(&scene_original).to_hex().to_string();
                if scene_source_hash != scene_expected {
                    legacy_catalog_hash_drifts.push(LegacyWorldCatalogHashDrift {
                        instance_id: entry.instance_id.clone(),
                        payload: "scene".to_owned(),
                        path: scene_path.to_owned(),
                        catalog_blake3: scene_expected.to_owned(),
                        verified_source_blake3: scene_source_hash,
                    });
                }
                let (scene_bytes, scene_removed) = sanitize_scene(
                    scene_path,
                    &scene_original,
                    &entry.terrain_descriptor,
                    &terrain_runtime_hash,
                    &entry.environment.path,
                    &environment_runtime_hash,
                )?;
                let scene_runtime_hash = blake3::hash(&scene_bytes).to_hex().to_string();
                insert_rewrite(
                    &mut scene_rewrites,
                    prepared_rewrite(scene_path, &scene_original, scene_bytes, scene_removed),
                )?;
                registry_entries.push(RuntimeWorldRegistryEntry {
                    id: entry.instance_id,
                    scope: entry.scope,
                    tile: entry.tile,
                    scene: RuntimeWorldContentReference {
                        path: scene_path.to_owned(),
                        blake3: scene_runtime_hash,
                    },
                    terrain: RuntimeWorldContentReference {
                        path: entry.terrain_descriptor,
                        blake3: terrain_runtime_hash,
                    },
                    environment: Some(RuntimeWorldContentReference {
                        path: entry.environment.path,
                        blake3: environment_runtime_hash,
                    }),
                });
            }
            "blocked" => {
                if entry.scene.is_some() || entry.scene_blake3.is_some() {
                    return invalid(format!(
                        "blocked world entry {:?} unexpectedly has a runtime scene",
                        entry.instance_id
                    ));
                }
                excluded_blocked_entries.push(entry.instance_id);
            }
            status => {
                return invalid(format!(
                    "world entry {:?} has unsupported placementStatus {status:?}",
                    entry.instance_id
                ));
            }
        }

        let provenance = native_path(&entry.provenance)?;
        if !asset_root.join(&provenance).is_file() {
            return invalid(format!(
                "world entry provenance is missing: {:?}",
                entry.provenance
            ));
        }
    }
    registry_entries.sort_by(|left, right| {
        (left.scope.as_str(), left.tile, left.id.as_str()).cmp(&(
            right.scope.as_str(),
            right.tile,
            right.id.as_str(),
        ))
    });
    excluded_blocked_entries.sort();
    sort_hash_drifts(&mut legacy_catalog_hash_drifts);

    let registry_document = RuntimeWorldRegistry {
        schema: RUNTIME_WORLD_REGISTRY_SCHEMA.to_owned(),
        entries: registry_entries,
    };
    let registry_bytes =
        pretty_json_bytes(&registry_document, Path::new(RUNTIME_WORLD_REGISTRY_PATH))?;
    audit_sanitized_json(RUNTIME_WORLD_REGISTRY_PATH, &registry_bytes)?;
    let registry = RuntimeWorldRegistryArtifact {
        path: RUNTIME_WORLD_REGISTRY_PATH.to_owned(),
        bytes: registry_bytes.len() as u64,
        blake3: blake3::hash(&registry_bytes).to_hex().to_string(),
    };

    let mut rewrites = environment_rewrites
        .into_values()
        .chain(terrain_rewrites.into_values())
        .chain(scene_rewrites.into_values())
        .collect::<Vec<_>>();
    rewrites.sort_by(|left, right| left.report.path.cmp(&right.report.path));

    let world_json_paths = collect_world_json_paths(&asset_root)?;
    let mut archived = Vec::new();
    for relative in world_json_paths {
        let Some(reason) = technical_metadata_reason(&relative) else {
            continue;
        };
        let bytes = verified_manifest_bytes(&asset_root, &manifest_by_path, &relative, None)?;
        archived.push(ArchivedWorldTechnicalMetadata {
            archive_path: format!("{ARCHIVE_FILES_DIRECTORY}/{relative}"),
            source_path: relative,
            reason: reason.to_owned(),
            bytes: bytes.len() as u64,
            blake3: blake3::hash(&bytes).to_hex().to_string(),
        });
    }
    archived.sort_by(|left, right| left.source_path.cmp(&right.source_path));
    validate_archive_coverage(&archived, &rewrites, &registry_document)?;

    let archived_paths = archived
        .iter()
        .map(|entry| entry.source_path.as_str())
        .collect::<BTreeSet<_>>();
    let rewrite_by_path = rewrites
        .iter()
        .map(|entry| (entry.report.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut next_manifest = manifest.clone();
    next_manifest
        .files
        .retain(|entry| !archived_paths.contains(entry.path.as_str()));
    for entry in &mut next_manifest.files {
        if let Some(rewrite) = rewrite_by_path.get(entry.path.as_str()) {
            entry.bytes = rewrite.report.runtime_bytes;
            entry.blake3 = rewrite.report.runtime_blake3.clone();
        }
    }
    next_manifest.files.push(ProjectAssetFile {
        source_path: "runtime-world/generated/_runtime/world.json".to_owned(),
        path: RUNTIME_WORLD_REGISTRY_PATH.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: registry.bytes,
        blake3: registry.blake3.clone(),
    });
    next_manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));

    Ok(RuntimeWorldMigrationPlan {
        asset_root,
        archive_root,
        manifest_path,
        manifest,
        next_manifest,
        registry_bytes,
        registry,
        archived,
        rewrites,
        excluded_blocked_entries,
        legacy_catalog_hash_drifts,
        blockers: Vec::new(),
    })
}

pub(super) fn prepared_rewrite(
    path: &str,
    original: &[u8],
    runtime: Vec<u8>,
    removed_evidence_fields: u64,
) -> PreparedRuntimeRewrite {
    PreparedRuntimeRewrite {
        report: RewrittenRuntimeWorldMetadata {
            path: path.to_owned(),
            original_archive_path: format!("{ARCHIVE_ORIGINALS_DIRECTORY}/{path}"),
            original_bytes: original.len() as u64,
            original_blake3: blake3::hash(original).to_hex().to_string(),
            runtime_bytes: runtime.len() as u64,
            runtime_blake3: blake3::hash(&runtime).to_hex().to_string(),
            removed_evidence_fields,
        },
        bytes: runtime,
    }
}

pub(super) fn insert_rewrite(
    rewrites: &mut BTreeMap<String, PreparedRuntimeRewrite>,
    rewrite: PreparedRuntimeRewrite,
) -> Result<()> {
    let path = rewrite.report.path.clone();
    if rewrites.insert(path.clone(), rewrite).is_some() {
        return invalid(format!("runtime world rewrite repeats {path:?}"));
    }
    Ok(())
}

pub(super) fn sanitize_environment(path: &str, original: &[u8]) -> Result<(Vec<u8>, u64)> {
    let mut value: Value =
        serde_json::from_slice(original).map_err(|source| PipelineError::Json {
            path: path.to_owned(),
            source,
        })?;
    let root = object_mut(&mut value, path)?;
    let mut removed = 0_u64;
    removed += remove_required(root, "mapScene", path)?;
    removed += remove_required(root, "placementAudit", path)?;
    removed += remove_required(root, "sourceCodeEvidence", path)?;
    let ambience = child_object_mut(root, "ambience", path)?;
    removed += remove_required(ambience, "sourceObject", path)?;
    let terrain_detail = child_object_mut(root, "terrainDetail", path)?;
    removed += remove_required(terrain_detail, "sourceObject", path)?;
    removed += remove_required(terrain_detail, "terrainRendererSourceObject", path)?;
    replace_provenance_words(&mut value);
    let bytes = pretty_value_bytes(&value, Path::new(path))?;
    audit_sanitized_json(path, &bytes)?;
    Ok((bytes, removed))
}

pub(crate) fn sanitize_scene(
    path: &str,
    original: &[u8],
    terrain_path: &str,
    terrain_blake3: &str,
    environment_path: &str,
    environment_blake3: &str,
) -> Result<(Vec<u8>, u64)> {
    let mut value: Value =
        serde_json::from_slice(original).map_err(|source| PipelineError::Json {
            path: path.to_owned(),
            source,
        })?;
    let root = object_mut(&mut value, path)?;
    let mut removed = remove_required(root, "provenance", path)?;
    let terrain = child_object_mut(root, "nativeTerrain", path)?;
    removed += remove_required(terrain, "sceneInstancePath", path)?;
    removed += remove_required(terrain, "sceneInstanceBlake3", path)?;
    let actual_terrain_path = terrain
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_error(format!("{path:?} nativeTerrain has no path")))?;
    if actual_terrain_path != terrain_path {
        return invalid(format!(
            "{path:?} nativeTerrain path is {actual_terrain_path:?}, expected {terrain_path:?}"
        ));
    }
    terrain.insert(
        "blake3".to_owned(),
        Value::String(terrain_blake3.to_owned()),
    );
    let environment = child_object_mut(terrain, "environment", path)?;
    let actual_environment_path = environment
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_error(format!("{path:?} nativeTerrain environment has no path")))?;
    if actual_environment_path != environment_path {
        return invalid(format!(
            "{path:?} nativeTerrain environment path is {actual_environment_path:?}, expected {environment_path:?}"
        ));
    }
    environment.insert(
        "blake3".to_owned(),
        Value::String(environment_blake3.to_owned()),
    );
    replace_provenance_words(&mut value);
    let bytes = pretty_value_bytes(&value, Path::new(path))?;
    audit_sanitized_json(path, &bytes)?;
    Ok((bytes, removed))
}

pub(super) fn replace_provenance_words(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                replace_provenance_words(value);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_provenance_words(value);
            }
        }
        Value::String(text) => {
            if text.contains("provenance") {
                *text = text.replace("provenance", "runtime source");
            }
            if text.contains("Provenance") {
                *text = text.replace("Provenance", "Runtime source");
            }
        }
        _ => {}
    }
}

pub(super) fn technical_metadata_reason(relative: &str) -> Option<&'static str> {
    if relative == LEGACY_WORLD_CATALOG_PATH {
        return Some("legacy world catalog replaced by minimal runtime registry");
    }
    let file_name = relative.rsplit('/').next().unwrap_or(relative);
    if file_name == "provenance.json" {
        return Some("per-tile import provenance");
    }
    if file_name == "manifest.json" {
        return Some("per-tile conversion manifest");
    }
    if file_name == "scene-instance.json" {
        return Some("scene-instance conversion linkage");
    }
    if relative.contains("/components/") && file_name.ends_with(".parsed.json") {
        return Some("parsed terrain component evidence");
    }
    if relative.contains("/environment/source/") && file_name.ends_with(".parsed.json") {
        return Some("parsed environment source evidence");
    }
    if relative.ends_with("/details/detail-database.raw.json") {
        return Some("raw terrain detail database document");
    }
    if relative.ends_with("/details/trees.raw.json") {
        return Some("raw terrain tree document");
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rollback(
    plan: &RuntimeWorldMigrationPlan,
    backup: &Path,
    manifest_next: &Path,
    manifest_backup: &Path,
    backed_up: &[PathBuf],
    installed: &[PathBuf],
    manifest_was_backed_up: bool,
    manifest_was_replaced: bool,
) {
    if manifest_was_replaced {
        let _ = fs::remove_file(&plan.manifest_path);
    }
    if manifest_was_backed_up {
        let _ = fs::rename(manifest_backup, &plan.manifest_path);
    }
    for relative in installed.iter().rev() {
        let _ = fs::remove_file(plan.asset_root.join(relative));
    }
    for relative in backed_up.iter().rev() {
        let source = backup.join(relative);
        let target = plan.asset_root.join(relative);
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::rename(source, target);
    }
    let _ = fs::remove_file(manifest_next);
    let _ = fs::remove_dir_all(backup);
    let _ = fs::remove_dir_all(&plan.archive_root);
}

pub(super) fn normalized_relative(root: &Path, path: &Path) -> Result<String> {
    let relative = if root.as_os_str().is_empty() {
        path
    } else {
        path.strip_prefix(root)
            .map_err(|_| invalid_error("world path escaped its root"))?
    };
    relative
        .components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid_error("world path is not UTF-8")),
            _ => Err(invalid_error("world path is not a normal relative path")),
        })
        .collect::<Result<Vec<_>>>()
        .map(|parts| parts.join("/"))
}

pub(super) fn remove_required(object: &mut Map<String, Value>, key: &str, path: &str) -> Result<u64> {
    if object.remove(key).is_none() {
        return invalid(format!(
            "{path:?} is missing required import-evidence field {key:?}"
        ));
    }
    Ok(1)
}

pub(super) fn remove_optional(object: &mut Map<String, Value>, key: &str) -> u64 {
    u64::from(object.remove(key).is_some())
}

pub(super) fn pretty_json_bytes(value: &impl Serialize, path: &Path) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn pretty_value_bytes(value: &Value, path: &Path) -> Result<Vec<u8>> {
    pretty_json_bytes(value, path)
}

pub(super) fn verify_file_identity(path: &Path, expected_bytes: u64, expected_blake3: &str) -> Result<()> {
    let metadata = fs::metadata(path).map_err(|error| io_at(path, error))?;
    let actual = hash_file(path)?;
    if !metadata.is_file() || metadata.len() != expected_bytes || actual != expected_blake3 {
        return invalid(format!(
            "written world migration file identity mismatch: {}",
            path.display()
        ));
    }
    Ok(())
}

pub(super) fn hash_file(path: &Path) -> Result<String> {
    let file = fs::File::open(path).map_err(|error| io_at(path, error))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| io_at(path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
