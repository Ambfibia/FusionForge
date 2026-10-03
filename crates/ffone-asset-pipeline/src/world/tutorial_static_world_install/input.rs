use super::*;

pub(super) fn load_world_map_contract(path: &Path, source_build: &str) -> Result<Vec<TileContract>> {
    let declaration: WorldMapStaticWorldContract =
        read_json(path, "world-map static world contract")?;
    if declaration.schema != WORLD_MAP_STATIC_WORLD_CONTRACT_SCHEMA
        || declaration.source_build != source_build
        || declaration.tiles.is_empty()
    {
        return invalid(format!(
            "{} is not a {WORLD_MAP_STATIC_WORLD_CONTRACT_SCHEMA} document for {source_build:?}",
            path.display()
        ));
    }
    let mut seen = BTreeSet::new();
    let mut contracts = Vec::with_capacity(declaration.tiles.len());
    for tile in declaration.tiles {
        validate_blake3(&tile.source_archive_blake3, "contract source archive")?;
        tile_coordinates(&tile.id)?;
        if !seen.insert(tile.id.clone()) {
            return invalid(format!(
                "{} declares tile {:?} more than once",
                path.display(),
                tile.id
            ));
        }
        contracts.push(TileContract {
            id: Cow::Owned(tile.id),
            source_archive_blake3: Cow::Owned(tile.source_archive_blake3),
            scene_nodes: tile.scene_nodes,
            exported_visuals: tile.exported_visuals,
            runtime_visuals: tile.runtime_visuals,
            exported_colliders: tile.exported_colliders,
            runtime_colliders: tile.runtime_colliders,
            exported_models: tile.exported_models,
            vertices: tile.vertices,
            indices: tile.indices,
        });
    }
    contracts.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(contracts)
}

pub(super) fn load_runtime_migration_proof(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    source_build: &str,
) -> Result<RuntimeMigrationProof> {
    if asset_root.file_name().and_then(|name| name.to_str()) != Some("game") {
        return invalid(
            "runtime-migrated tutorial recovery requires an asset root named assets/game",
        );
    }
    let assets_root = asset_root
        .parent()
        .ok_or_else(|| invalid_error("assets/game has no assets parent"))?;
    if assets_root.file_name().and_then(|name| name.to_str()) != Some("assets") {
        return invalid(
            "runtime-migrated tutorial recovery requires an asset root named assets/game",
        );
    }
    let project_root = assets_root
        .parent()
        .ok_or_else(|| invalid_error("assets/game has no project parent"))?;
    let archive_root = project_root
        .join("content")
        .join("imported")
        .join(source_build)
        .join("world-conversion-metadata");
    let report = validate_completed_migration(asset_root, &archive_root, source_build)?;
    if report.registry.path != RUNTIME_WORLD_REGISTRY_PATH {
        return invalid("completed runtime migration publishes an unexpected registry path");
    }

    let registry_bytes = verify_manifest_file(
        asset_root,
        manifest,
        RUNTIME_WORLD_REGISTRY_PATH,
        "completed runtime world registry",
    )?;
    if registry_bytes.len() as u64 != report.registry.bytes
        || hash_bytes(&registry_bytes) != report.registry.blake3
    {
        return invalid("completed runtime migration registry proof differs from the project");
    }
    let registry_path = safe_join(asset_root, RUNTIME_WORLD_REGISTRY_PATH)?;
    let registry: RuntimeWorldRegistry = parse_json(&registry_bytes, &registry_path)?;
    if registry.schema != RUNTIME_WORLD_REGISTRY_SCHEMA
        || registry.entries.len() as u64 != report.counts.registry_entries
    {
        return invalid("completed runtime migration registry has an invalid identity or count");
    }
    let mut registry_by_scene = BTreeMap::new();
    for entry in registry.entries {
        validate_relative(&entry.scene.path)?;
        if registry_by_scene
            .insert(entry.scene.path.clone(), entry)
            .is_some()
        {
            return invalid("completed runtime migration registry repeats a scene path");
        }
    }
    Ok(RuntimeMigrationProof {
        archive_root,
        report,
        registry_by_scene,
    })
}

pub(super) fn load_cleanup_archived_ownership(asset_root: &Path) -> Result<Option<CleanupArchivedOwnership>> {
    if asset_root.file_name().and_then(|name| name.to_str()) != Some("game") {
        return Ok(None);
    }
    let Some(assets_root) = asset_root
        .parent()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("assets"))
    else {
        return Ok(None);
    };
    let Some(project_root) = assets_root.parent() else {
        return Ok(None);
    };
    let archive_root = project_root
        .join("content")
        .join("imported")
        .join(TUTORIAL_STATIC_WORLD_SOURCE_BUILD)
        .join("conversion-metadata");
    if !archive_root.exists() {
        return Ok(None);
    }
    let _ = validate_completed_cleanup_archive(&archive_root, TUTORIAL_STATIC_WORLD_SOURCE_BUILD)?;

    let mut roots = vec![archive_root.clone()];
    let revisions_root = archive_root.join("revisions");
    if revisions_root.exists() {
        let mut revisions = fs::read_dir(&revisions_root)
            .map_err(|error| io_at(&revisions_root, error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| io_at(&revisions_root, error))?;
        revisions.sort_by_key(|entry| entry.file_name());
        roots.extend(revisions.into_iter().map(|entry| entry.path()));
    }

    let mut candidates = Vec::new();
    for root in roots {
        let ownership_archive_path = safe_join(
            &root,
            &format!("files/{TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH}"),
        )?;
        if !ownership_archive_path.exists() {
            continue;
        }
        let index_path = root.join(CLEAN_RUNTIME_METADATA_INDEX_FILE);
        let index_bytes = read_regular_file(&index_path, "cleanup archive index")?;
        let index: CleanupArchiveIndex = parse_json(&index_bytes, &index_path)?;
        let primary = root == archive_root;
        let revision_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let valid_identity = if primary {
            index.schema == CLEAN_RUNTIME_METADATA_INDEX_SCHEMA && index.plan_blake3.is_none()
        } else {
            index.schema == CLEANUP_REVISION_INDEX_SCHEMA
                && index.plan_blake3.as_deref() == Some(revision_name)
        };
        if !valid_identity || index.source_build != TUTORIAL_STATIC_WORLD_SOURCE_BUILD {
            return invalid("cleanup archive ownership index has the wrong identity");
        }
        let mut files_by_source = BTreeMap::new();
        for entry in index.files {
            if files_by_source
                .insert(entry.source_path.clone(), entry)
                .is_some()
            {
                return invalid("cleanup archive ownership index repeats a source path");
            }
        }
        let ownership_entry = files_by_source
            .get(TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH)
            .ok_or_else(|| invalid_error("cleanup ownership payload is absent from its index"))?;
        if ownership_entry.archive_path != format!("files/{TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH}")
            || ownership_entry.reason != CLEANUP_STATIC_REASON
            || !ownership_entry.manifested
            || ownership_entry.manifest_source_path.as_deref() != Some(INSTALLER_ID)
            || ownership_entry.manifest_kind != Some(ProjectAssetKind::Data)
        {
            return invalid("cleanup-archived tutorial ownership has an invalid index proof");
        }
        let ownership_bytes = read_regular_file(
            &ownership_archive_path,
            "cleanup-archived tutorial ownership",
        )?;
        if ownership_bytes.len() as u64 != ownership_entry.bytes
            || hash_bytes(&ownership_bytes) != ownership_entry.blake3
        {
            return invalid("cleanup-archived tutorial ownership payload proof differs");
        }
        let ownership: TutorialStaticWorldOwnership =
            parse_json(&ownership_bytes, &ownership_archive_path)?;
        let mut expected_static = ownership
            .owned_files
            .iter()
            .filter(|owned| owned.kind == ProjectAssetKind::Data)
            .map(|owned| owned.path.clone())
            .collect::<BTreeSet<_>>();
        expected_static.insert(TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH.to_owned());
        let actual_static = files_by_source
            .keys()
            .filter(|path| path.starts_with("world/tutorial/static/"))
            .cloned()
            .collect::<BTreeSet<_>>();
        if actual_static != expected_static
            || actual_static.iter().any(|path| {
                files_by_source
                    .get(path)
                    .is_none_or(|entry| entry.reason != CLEANUP_STATIC_REASON)
            })
        {
            return invalid(
                "cleanup archive does not contain the exact tutorial static metadata closure",
            );
        }
        candidates.push(CleanupArchivedOwnership {
            ownership,
            files_by_source,
        });
    }
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.pop()),
        _ => invalid("cleanup archive contains ambiguous tutorial ownership revisions"),
    }
}

pub(super) fn collect_files(root: &Path, relative_root: &Path, output: &mut BTreeSet<String>) -> Result<()> {
    reject_symlink(root, "audited directory")?;
    if !root.is_dir() {
        return invalid(format!("audited directory is missing: {}", root.display()));
    }
    for entry in fs::read_dir(root).map_err(|error| io_at(root, error))? {
        let entry = entry.map_err(|error| io_at(root, error))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| io_at(&path, error))?;
        if metadata.file_type().is_symlink() {
            return invalid(format!(
                "symlink is forbidden in audited tree: {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_files(&path, relative_root, output)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(relative_root)
                .map_err(|_| invalid_error("audited path escaped its root"))?;
            let relative = relative_path_string(relative)?;
            validate_relative(&relative)?;
            if !output.insert(relative.clone()) {
                return invalid(format!("duplicate audited path {relative:?}"));
            }
        } else {
            return invalid(format!(
                "special filesystem entry is forbidden in audited tree: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

pub(super) fn read_regular_file(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return invalid(format!(
            "{label} must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    fs::read(path).map_err(|error| io_at(path, error))
}

pub(super) fn read_json<T: DeserializeOwned>(path: &Path, label: &str) -> Result<T> {
    let bytes = read_regular_file(path, label)?;
    parse_json(&bytes, path)
}

pub(super) fn parse_json<T: DeserializeOwned>(bytes: &[u8], path: &Path) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })
}
