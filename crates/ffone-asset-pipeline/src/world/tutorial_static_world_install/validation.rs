use super::*;

pub(super) fn audit_tile_export(
    export_root: &Path,
    contract: &TileContract,
    source_build: &str,
) -> Result<AuditedTile> {
    let scope = StaticWorldScope::of_tile(&contract.id)?;
    let tile_root = export_root.join(contract.id.as_ref());
    reject_symlink(&tile_root, "static world tile export")?;
    let manifest_path = tile_root.join(EXPORT_MANIFEST_FILE);
    let manifest_bytes = read_regular_file(&manifest_path, "static world export manifest")?;
    let manifest: ExportManifest = parse_json(&manifest_bytes, &manifest_path)?;
    if manifest.schema != EXPORT_MANIFEST_SCHEMA
        || manifest.self_excluded != EXPORT_MANIFEST_FILE
        || manifest.source_build != source_build
        || manifest.tile_id != *contract.id
    {
        return invalid(format!(
            "{} is not the exact {:?} export manifest for {}",
            manifest_path.display(),
            source_build,
            contract.id
        ));
    }
    if manifest.counts.files != manifest.files.len() as u64 {
        return invalid(format!(
            "{} file count is {}, expected {} entries",
            manifest_path.display(),
            manifest.counts.files,
            manifest.files.len()
        ));
    }
    let declared_bytes = manifest
        .files
        .iter()
        .map(|file| file.byte_length)
        .sum::<u64>();
    if manifest.counts.bytes != declared_bytes {
        return invalid(format!(
            "{} byte count is {}, but entries total {}",
            manifest_path.display(),
            manifest.counts.bytes,
            declared_bytes
        ));
    }

    let mut files = BTreeMap::<String, &ExportManifestFile>::new();
    let mut casefold = BTreeSet::<String>::new();
    for file in &manifest.files {
        validate_relative(&file.path)?;
        validate_blake3(&file.blake3, "export file")?;
        if !casefold.insert(file.path.to_ascii_lowercase()) {
            return invalid(format!(
                "case-folded export path collision in {} at {:?}",
                contract.id, file.path
            ));
        }
        if files.insert(file.path.clone(), file).is_some() {
            return invalid(format!(
                "duplicate export path in {} at {:?}",
                contract.id, file.path
            ));
        }
        let path = safe_join(&tile_root, &file.path)?;
        let (bytes, blake3) = hash_regular_file(&path, "static world export file")?;
        if bytes != file.byte_length || blake3 != file.blake3 {
            return invalid(format!(
                "static world export proof mismatch at {}: bytes={bytes}/{}, blake3={blake3}/{}",
                path.display(),
                file.byte_length,
                file.blake3
            ));
        }
    }
    validate_complete_export_tree(&tile_root, &files)?;

    let report_entry = required_export_file(&files, EXPORT_REPORT_FILE)?;
    let report_path = tile_root.join(EXPORT_REPORT_FILE);
    let report: ExportReport = read_json(&report_path, "static world export report")?;
    let scene_path = tile_scene_path(&contract.id);
    let hierarchy_path = static_metadata_path(&contract.id, "hierarchy.json");
    let material_path = static_metadata_path(&contract.id, "materials.json");
    let catalog_path = static_metadata_path(&contract.id, "catalog.json");
    if report.schema != EXPORT_REPORT_SCHEMA
        || report.status != EXPORT_STATUS
        || report.source_build != source_build
        || report.tile_id != *contract.id
        || report.source_archive_blake3 != contract.source_archive_blake3
        || report.scene_path != scene_path
        || report.hierarchy_path != hierarchy_path
        || report.material_path != material_path
        || report.catalog_path != catalog_path
        || report.manifest_path != EXPORT_MANIFEST_FILE
    {
        return invalid(format!(
            "{} does not match the exact static-world source contract for {}",
            report_path.display(),
            contract.id
        ));
    }
    validate_blake3(&report.source_archive_blake3, "source archive")?;
    validate_report_counts(
        &report.counts,
        contract,
        &manifest.counts,
        manifest_bytes.len() as u64,
    )?;

    let scene_entry = required_export_file(&files, &scene_path)?;
    let hierarchy_entry = required_export_file(&files, &hierarchy_path)?;
    let material_entry = required_export_file(&files, &material_path)?;
    let catalog_entry = required_export_file(&files, &catalog_path)?;
    for (label, declared, entry) in [
        ("scene", &report.scene_blake3, scene_entry),
        ("hierarchy", &report.hierarchy_blake3, hierarchy_entry),
        ("materials", &report.material_blake3, material_entry),
        ("catalog", &report.catalog_blake3, catalog_entry),
    ] {
        if declared != &entry.blake3 {
            return invalid(format!(
                "{} {label} hash differs from export manifest for {}",
                report_path.display(),
                contract.id
            ));
        }
    }
    let _ = report_entry;

    validate_metadata_identity(
        &safe_join(&tile_root, &catalog_path)?,
        STATIC_CATALOG_SCHEMA,
        source_build,
        &contract.id,
    )?;
    validate_metadata_identity(
        &safe_join(&tile_root, &hierarchy_path)?,
        STATIC_HIERARCHY_SCHEMA,
        source_build,
        &contract.id,
    )?;
    validate_metadata_identity(
        &safe_join(&tile_root, &material_path)?,
        STATIC_MATERIALS_SCHEMA,
        source_build,
        &contract.id,
    )?;

    let scene_file = safe_join(&tile_root, &scene_path)?;
    let scene_bytes = read_regular_file(&scene_file, "static world merged scene")?;
    let scene: JsonValue = parse_json(&scene_bytes, &scene_file)?;
    validate_source_scene(&scene, contract, &files)?;

    let model_prefix = format!("{}/", model_tile_root(&contract.id));
    let mut selected = Vec::<SourcePublication>::new();
    for file in &manifest.files {
        let is_model_asset = file.path.starts_with(&model_prefix);
        let is_metadata = matches!(
            file.path.as_str(),
            path if path == hierarchy_path || path == material_path || path == catalog_path
        );
        if !is_model_asset && !is_metadata {
            continue;
        }
        let kind = export_kind(&file.kind)?;
        if is_model_asset && !matches!(kind, ProjectAssetKind::Model | ProjectAssetKind::Texture) {
            return invalid(format!(
                "unexpected {:?} file inside runtime model tree: {:?}",
                kind, file.path
            ));
        }
        if is_metadata && kind != ProjectAssetKind::Data {
            return invalid(format!(
                "static metadata must be data, got {:?} at {:?}",
                kind, file.path
            ));
        }
        selected.push(SourcePublication {
            source: safe_join(&tile_root, &file.path)?,
            source_path: format!(
                "{}/{}/{}",
                scope.source_path_prefix(),
                contract.id,
                file.path
            ),
            path: file.path.clone(),
            kind,
            bytes: file.byte_length,
            blake3: file.blake3.clone(),
        });
    }
    selected.sort_by(|left, right| left.path.cmp(&right.path));
    validate_glb_dependencies(&selected)?;

    Ok(AuditedTile {
        proof: TutorialStaticWorldTileProof {
            tile_id: contract.id.clone().into_owned(),
            source_archive_blake3: report.source_archive_blake3,
            export_manifest_bytes: manifest_bytes.len() as u64,
            export_manifest_blake3: hash_bytes(&manifest_bytes),
            export_files: manifest.counts.files,
            export_bytes: manifest.counts.bytes,
            source_scene_blake3: scene_entry.blake3.clone(),
        },
        selected,
        scene,
        scene_bytes,
        scene_blake3: scene_entry.blake3.clone(),
        model_count: contract.exported_models,
        visual_count: contract.runtime_visuals,
        collider_count: contract.runtime_colliders,
    })
}

pub(super) fn validate_report_counts(
    counts: &ExportReportCounts,
    contract: &TileContract,
    manifest: &ExportManifestCounts,
    manifest_byte_length: u64,
) -> Result<()> {
    let exact = counts.scene_nodes == contract.scene_nodes
        && counts.exported_visual_payloads == contract.exported_visuals
        && counts.runtime_visuals == contract.runtime_visuals
        && counts.exported_collider_payloads == contract.exported_colliders
        && counts.runtime_colliders == contract.runtime_colliders
        && counts.exported_models == contract.exported_models
        && counts.vertices == contract.vertices
        && counts.indices == contract.indices;
    if !exact {
        return invalid(format!(
            "static export report counts differ from exact contract for {}",
            contract.id
        ));
    }
    // Exports made before the exporter report-finalization fix contain the
    // explicit pair 0/0 here. The manifest is self-excluded, while current
    // reports deliberately count the complete committed tree including that
    // manifest. Retain the intermediate self-excluded pair for already-made
    // exports, but reject every other non-zero combination.
    let legacy_zero_pair = counts.output_files == 0 && counts.output_bytes == 0;
    let legacy_self_excluded_pair =
        counts.output_files == manifest.files && counts.output_bytes == manifest.bytes;
    let finalized_complete_tree_pair = counts.output_files == manifest.files.saturating_add(1)
        && counts.output_bytes == manifest.bytes.saturating_add(manifest_byte_length);
    if !legacy_zero_pair && !legacy_self_excluded_pair && !finalized_complete_tree_pair {
        return invalid(format!(
            "static export output counts are neither an audited legacy pair nor the exact complete-tree counts for {}",
            contract.id
        ));
    }
    Ok(())
}

pub(super) fn validate_source_scene(
    scene: &JsonValue,
    contract: &TileContract,
    files: &BTreeMap<String, &ExportManifestFile>,
) -> Result<()> {
    let object = scene
        .as_object()
        .ok_or_else(|| invalid_error("static world scene is not a JSON object"))?;
    let expected_runtime_keys = [
        "colliders",
        "coordinateContract",
        "coverage",
        "models",
        "name",
        "nativeTerrain",
        "root",
        "schema",
        "scope",
        "tile",
        "visuals",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let mut expected_conversion_keys = expected_runtime_keys.clone();
    expected_conversion_keys.insert("provenance");
    let actual_keys = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if (actual_keys != expected_runtime_keys && actual_keys != expected_conversion_keys)
        || object.get("schema").and_then(JsonValue::as_str) != Some(WORLD_SCENE_SCHEMA)
        || object.get("scope").and_then(JsonValue::as_str)
            != Some(StaticWorldScope::of_tile(&contract.id)?.scene_scope())
        || object.get("name").and_then(JsonValue::as_str) != Some(contract.id.as_ref())
        || object.get("coverage").and_then(JsonValue::as_str) != Some(STATIC_COVERAGE)
    {
        return invalid(format!(
            "static world scene for {} has the wrong identity or shape",
            contract.id
        ));
    }
    let models = required_array(scene, "models", &contract.id)?;
    let visuals = required_array(scene, "visuals", &contract.id)?;
    let colliders = required_array(scene, "colliders", &contract.id)?;
    if models.len() as u64 != contract.exported_models
        || visuals.len() as u64 != contract.runtime_visuals
        || colliders.len() as u64 != contract.runtime_colliders
    {
        return invalid(format!(
            "static world scene array counts differ from exact contract for {}",
            contract.id
        ));
    }

    let model_prefix = format!("{}/", model_tile_root(&contract.id));
    let exported_glbs = files
        .iter()
        .filter_map(|(path, entry)| {
            (path.starts_with(&model_prefix) && path.ends_with(".glb"))
                .then_some((path.as_str(), entry.blake3.as_str()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut scene_glbs = BTreeMap::<&str, &str>::new();
    let mut ids = BTreeSet::<&str>::new();
    for model in models {
        let path = required_string(model, "path", "scene model")?;
        let id = required_string(model, "id", "scene model")?;
        let blake3 = required_string(model, "blake3", "scene model")?;
        validate_relative(path)?;
        if !ids.insert(id) || scene_glbs.insert(path, blake3).is_some() {
            return invalid(format!(
                "duplicate model id or path in static scene for {}",
                contract.id
            ));
        }
    }
    if scene_glbs != exported_glbs {
        return invalid(format!(
            "static scene model proofs do not exactly match exported GLBs for {}",
            contract.id
        ));
    }
    for (label, instances) in [("visual", visuals), ("collider", colliders)] {
        for instance in instances {
            let model = required_string(instance, "model", label)?;
            if !ids.contains(model) {
                return invalid(format!(
                    "{label} references unknown model {model:?} in {}",
                    contract.id
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_observed_scene_hash(
    document: &str,
    scene_path: &str,
    observed: &str,
    current_scene_blake3: &BTreeMap<String, String>,
    allow_legacy_stale_hashes: bool,
) -> Result<()> {
    validate_blake3(observed, "world scene reference")?;
    let current = current_scene_blake3.get(scene_path).ok_or_else(|| {
        invalid_error(format!(
            "current tutorial scene proof is missing for {scene_path:?}"
        ))
    })?;
    if !allow_legacy_stale_hashes && observed != current {
        return invalid(format!(
            "{document} scene hash differs from the current project scene at {scene_path:?}"
        ));
    }
    Ok(())
}

pub(super) fn require_complete_reference_coverage(
    document: &str,
    expected: &BTreeMap<String, ExpectedSceneReference>,
    seen: &BTreeSet<String>,
) -> Result<()> {
    if seen.len() == expected.len() && expected.keys().all(|path| seen.contains(path)) {
        return Ok(());
    }
    let missing = expected
        .keys()
        .filter(|path| !seen.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    invalid(format!(
        "{document} does not reference every installed tutorial scene; missing={missing:?}"
    ))
}

pub(super) fn validate_installed_scene_identity(
    value: &JsonValue,
    scene: &TutorialStaticWorldSceneProof,
) -> Result<()> {
    let object = value.as_object().ok_or_else(|| {
        invalid_error(format!(
            "installed tutorial scene is not an object at {:?}",
            scene.path
        ))
    })?;
    let tile = object
        .get("tile")
        .and_then(JsonValue::as_array)
        .filter(|tile| tile.len() == 2)
        .and_then(|tile| {
            Some([
                i32::try_from(tile[0].as_i64()?).ok()?,
                i32::try_from(tile[1].as_i64()?).ok()?,
            ])
        });
    if object.get("schema").and_then(JsonValue::as_str) != Some(WORLD_SCENE_SCHEMA)
        || object.get("scope").and_then(JsonValue::as_str)
            != Some(StaticWorldScope::of_tile(&scene.tile_id)?.scene_scope())
        || object.get("name").and_then(JsonValue::as_str) != Some(scene.tile_id.as_str())
        || object.get("coverage").and_then(JsonValue::as_str) != Some(STATIC_COVERAGE)
        || tile != Some(tile_coordinates(&scene.tile_id)?)
        || !object
            .get("coordinateContract")
            .is_some_and(JsonValue::is_object)
        || !object
            .get("nativeTerrain")
            .is_some_and(JsonValue::is_object)
        || !object.get("root").is_some_and(JsonValue::is_object)
        || required_array(value, "models", &scene.tile_id)?.len() as u64 != scene.model_count
        || required_array(value, "visuals", &scene.tile_id)?.len() as u64 != scene.visual_count
        || required_array(value, "colliders", &scene.tile_id)?.len() as u64 != scene.collider_count
    {
        return invalid(format!(
            "installed tutorial scene has the wrong identity or shape at {:?}",
            scene.path
        ));
    }
    Ok(())
}

pub(super) fn validate_cleanup_noop_source(
    previous: &TutorialStaticWorldOwnership,
    tiles: &[TutorialStaticWorldTileProof],
    owned_files: &[TutorialStaticWorldOwnedFile],
    scenes: &[TutorialStaticWorldSceneProof],
) -> Result<()> {
    let mut previous_tiles = previous.tiles.clone();
    let mut previous_owned = previous.owned_files.clone();
    previous_tiles.sort_by(|left, right| left.tile_id.cmp(&right.tile_id));
    previous_owned.sort_by(|left, right| left.path.cmp(&right.path));
    if previous_tiles != tiles || previous_owned != owned_files {
        return invalid("cleanup-archived tutorial source proofs differ from the exact export");
    }
    let previous_scenes = previous
        .scenes
        .iter()
        .map(|scene| (scene.tile_id.as_str(), scene))
        .collect::<BTreeMap<_, _>>();
    if previous_scenes.len() != scenes.len() {
        return invalid("cleanup-archived tutorial scene proof coverage differs");
    }
    for scene in scenes {
        let Some(previous) = previous_scenes.get(scene.tile_id.as_str()).copied() else {
            return invalid("cleanup-archived tutorial scene proof is missing");
        };
        if previous.path != scene.path
            || previous.source_scene_blake3 != scene.source_scene_blake3
            || previous.static_fields_blake3 != scene.static_fields_blake3
            || previous.model_count != scene.model_count
            || previous.visual_count != scene.visual_count
            || previous.collider_count != scene.collider_count
        {
            return invalid(format!(
                "cleanup-archived tutorial scene source proof differs for {:?}",
                scene.tile_id
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_first_install_destinations(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    contracts: &[TileContract],
    previous: Option<&TutorialStaticWorldOwnership>,
) -> Result<()> {
    if previous.is_some() {
        return Ok(());
    }
    let scope = StaticWorldScope::of_contracts(contracts)?;
    let prefixes = contracts
        .iter()
        .map(|contract| format!("{}/", model_tile_root(&contract.id)))
        .chain(std::iter::once(format!("{}/", scope.static_root())))
        .collect::<Vec<_>>();
    if let Some(entry) = manifest
        .files
        .iter()
        .find(|entry| prefixes.iter().any(|prefix| entry.path.starts_with(prefix)))
    {
        return invalid(format!(
            "first static-world install would collide with manifest path {:?}",
            entry.path
        ));
    }
    for contract in contracts {
        let path = safe_join(asset_root, &model_tile_root(&contract.id))?;
        if path.exists() {
            return invalid(format!(
                "first static-world install would collide with {}",
                path.display()
            ));
        }
    }
    let static_root = safe_join(asset_root, scope.static_root())?;
    if static_root.exists() {
        return invalid(format!(
            "first static-world install would collide with {}",
            static_root.display()
        ));
    }
    Ok(())
}

pub(super) fn validate_owned_trees(
    asset_root: &Path,
    contracts: &[TileContract],
    owned_paths: &BTreeSet<String>,
    ownership_present: bool,
) -> Result<()> {
    let mut observed = BTreeSet::new();
    for contract in contracts {
        let relative = model_tile_root(&contract.id);
        let root = safe_join(asset_root, &relative)?;
        collect_files(&root, asset_root, &mut observed)?;
    }
    let scope = StaticWorldScope::of_contracts(contracts)?;
    let static_root = safe_join(asset_root, scope.static_root())?;
    if static_root.exists() {
        collect_files(&static_root, asset_root, &mut observed)?;
    }
    let mut expected = owned_paths.clone();
    if ownership_present {
        expected.insert(scope.ownership_path().to_owned());
    }
    if observed != expected {
        return invalid("owned static-world trees contain missing or unowned files");
    }
    Ok(())
}

pub(super) fn validate_export_root_entries(export_root: &Path, contracts: &[TileContract]) -> Result<()> {
    let expected = contracts
        .iter()
        .map(|contract| contract.id.as_ref())
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(export_root).map_err(|error| io_at(export_root, error))? {
        let entry = entry.map_err(|error| io_at(export_root, error))?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|error| io_at(entry.path(), error))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return invalid(format!(
                "static world export root contains a non-directory or symlink: {}",
                entry.path().display()
            ));
        }
        let name = entry
            .file_name()
            .to_str()
            .ok_or_else(|| invalid_error("static world tile name is not UTF-8"))?
            .to_owned();
        actual.insert(name);
    }
    if actual.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return invalid(format!(
            "static world export root tile set differs: actual={actual:?}, expected={expected:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_complete_export_tree(
    tile_root: &Path,
    files: &BTreeMap<String, &ExportManifestFile>,
) -> Result<()> {
    let mut observed = BTreeSet::new();
    collect_files(tile_root, tile_root, &mut observed)?;
    observed.remove(EXPORT_MANIFEST_FILE);
    let declared = files.keys().cloned().collect::<BTreeSet<_>>();
    if observed != declared {
        return invalid(format!(
            "static world export tree differs from manifest at {}",
            tile_root.display()
        ));
    }
    Ok(())
}

pub(super) fn validate_project_manifest(manifest: &ProjectAssetManifest) -> Result<()> {
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid(format!(
            "project asset manifest schema is {:?}, expected {:?}",
            manifest.schema, PROJECT_ASSET_SCHEMA
        ));
    }
    let mut casefold = BTreeSet::new();
    for entry in &manifest.files {
        validate_relative(&entry.path)?;
        validate_blake3(&entry.blake3, "project asset")?;
        if !casefold.insert(entry.path.to_ascii_lowercase()) {
            return invalid(format!(
                "project asset manifest has a case-folded collision at {:?}",
                entry.path
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_metadata_identity(
    path: &Path,
    schema: &str,
    source_build: &str,
    tile_id: &str,
) -> Result<()> {
    let value: JsonValue = read_json(path, "static world metadata")?;
    if value.get("schema").and_then(JsonValue::as_str) != Some(schema)
        || value.get("sourceBuild").and_then(JsonValue::as_str) != Some(source_build)
        || value.get("tileId").and_then(JsonValue::as_str) != Some(tile_id)
    {
        return invalid(format!(
            "static world metadata has wrong identity at {}",
            path.display()
        ));
    }
    Ok(())
}

pub(super) fn validate_owned_path(path: &str, contracts: &[TileContract]) -> Result<()> {
    validate_relative(path)?;
    let model_owned = contracts
        .iter()
        .any(|contract| path.starts_with(&format!("{}/", model_tile_root(&contract.id))));
    let metadata_owned = contracts.iter().any(|contract| {
        matches!(
            path,
            candidate if candidate == static_metadata_path(&contract.id, "catalog.json")
                || candidate == static_metadata_path(&contract.id, "hierarchy.json")
                || candidate == static_metadata_path(&contract.id, "materials.json")
        )
    });
    if !model_owned && !metadata_owned {
        return invalid(format!(
            "ownership manifest claims out-of-scope path {path:?}"
        ));
    }
    Ok(())
}

pub(super) fn reject_symlink(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if metadata.file_type().is_symlink() {
        return invalid(format!("{label} must not be a symlink: {}", path.display()));
    }
    Ok(())
}

pub(super) fn validate_relative(path: &str) -> Result<()> {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains('\0')
    {
        return invalid(format!("unsafe tutorial static-world path {path:?}"));
    }
    for component in path.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains(':')
            || component.ends_with('.')
            || component.ends_with(' ')
            || is_windows_device_name(component)
        {
            return invalid(format!("unsafe tutorial static-world path {path:?}"));
        }
    }
    Ok(())
}

pub(super) fn validate_blake3(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return invalid(format!("{label} BLAKE3 is not lowercase hexadecimal"));
    }
    Ok(())
}

pub(super) fn generated_json_error(source: serde_json::Error) -> PipelineError {
    PipelineError::Json {
        path: "<generated tutorial static-world JSON>".to_owned(),
        source,
    }
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::InvalidManifest(message.into())
}
