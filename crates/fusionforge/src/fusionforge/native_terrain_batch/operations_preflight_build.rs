use super::*;

pub(super) fn process_tiles_parallel(
    candidates: Vec<TileSource>,
    build_root: &Path,
    repo_root: &Path,
    archive_index: &HashMap<String, PathBuf>,
    staging_root: &Path,
    work_root: &Path,
) -> Result<Vec<TileOutcome>, String> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = candidates.len().min(4);
    let queue = Arc::new(Mutex::new(VecDeque::from(candidates)));
    let outcomes = Arc::new(Mutex::new(Vec::new()));
    thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let outcomes = Arc::clone(&outcomes);
            scope.spawn(move || loop {
                let source = {
                    let mut queue = queue
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    queue.pop_front()
                };
                let Some(source) = source else {
                    break;
                };
                let outcome = process_tile(
                    &source,
                    build_root,
                    repo_root,
                    archive_index,
                    staging_root,
                    work_root,
                );
                outcomes
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(outcome);
            });
        }
    });
    let outcomes = Arc::try_unwrap(outcomes)
        .map_err(|_| "native terrain worker outcomes are still shared".to_string())?
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Ok(outcomes)
}

pub(super) fn process_tile(
    source: &TileSource,
    build_root: &Path,
    repo_root: &Path,
    archive_index: &HashMap<String, PathBuf>,
    staging_root: &Path,
    work_root: &Path,
) -> TileOutcome {
    let session_path = tile_session_path(work_root, source.scope, &source.tile_id);
    let loaded = match load_tile(source, build_root, repo_root, archive_index, &session_path) {
        Ok(value) => value,
        Err(message) => {
            return blocked_outcome(blocked_tile(
                source,
                "dependency-load",
                "dependencyClosure",
                message,
            ));
        }
    };
    let nominal_identity =
        match exact_owned_terrain_identity(&loaded.env, &loaded.resource_asset_names, source) {
            Ok(value) => value,
            Err(message) => {
                return blocked_outcome(blocked_tile(
                    source,
                    "terrain-ownership",
                    "wrongTerrainDataOwnership",
                    message,
                ));
            }
        };
    let resolved_source = match resolve_scene_owned_terrain_source(
        &loaded,
        source,
        archive_index,
        nominal_identity,
    ) {
        Ok(value) => value,
        Err(message) => {
            return blocked_outcome(blocked_tile(
                source,
                "terrain-ownership",
                "sceneDependencyTerrainOwnership",
                message,
            ));
        }
    };
    let identity = resolved_source.identity;

    let terrain_root = staging_root.join(path_from_forward_slashes(&source.destination));
    if let Err(message) = export_native_terrain_exact_in_caller_staging(
        &loaded.env,
        &resolved_source.resource_path,
        &terrain_root,
        identity.key,
    ) {
        return blocked_outcome(blocked_tile(
            source,
            "terrain-export",
            "nativeTerrainExport",
            message,
        ));
    }

    let mut blocked = Vec::new();
    match terrain_graph_closure_blockers(&terrain_root.join("terrain.json")) {
        Ok(closure_blockers) => {
            for (code, message) in closure_blockers {
                blocked.push(blocked_tile(source, "asset-closure", code, message));
            }
        }
        Err(message) => {
            return fatal_outcome(format!(
                "could not audit native terrain asset closure for {}: {message}",
                source.tile_id
            ));
        }
    }
    let (placement_status, scene_instance_document, scene_instance) =
        match link_scene_instance(&loaded.env, &loaded.map_asset_names, &identity, source) {
            Ok(scene_link) => {
                for sidecar in &scene_link.sidecars {
                    let path = terrain_root.join(path_from_forward_slashes(&sidecar.relative_path));
                    if let Some(parent) = path.parent() {
                        if let Err(err) = fs::create_dir_all(parent) {
                            return fatal_outcome(format!(
                                "could not create terrain sidecar directory {}: {err}",
                                parent.display()
                            ));
                        }
                    }
                    if let Err(err) = write_bytes_new(&path, &sidecar.bytes) {
                        return fatal_outcome(format!(
                            "could not publish terrain sidecar {}: {err}",
                            path.display()
                        ));
                    }
                }
                if let Some(contract) = scene_link.gameplay_attributes.as_ref() {
                    if let Err(err) =
                        attach_gameplay_attributes(&terrain_root.join("terrain.json"), contract)
                    {
                        return fatal_outcome(err);
                    }
                }
                for (code, message) in &scene_link.semantic_blockers {
                    blocked.push(blocked_tile(
                        source,
                        "scene-components",
                        *code,
                        message.clone(),
                    ));
                }
                if let Err(err) = write_json_new(
                    &terrain_root.join("scene-instance.json"),
                    &scene_link.document,
                ) {
                    return fatal_outcome(format!(
                        "could not publish linked scene instance for {}: {err}",
                        source.tile_id
                    ));
                }
                (
                    "linked",
                    Some(format!("{}/scene-instance.json", source.destination)),
                    Some(scene_link.summary),
                )
            }
            Err(message) => {
                blocked.push(blocked_tile(
                    source,
                    "scene-link",
                    "terrainColliderLinkage",
                    message,
                ));
                ("blocked", None, None)
            }
        };
    TileOutcome {
        exported: Some(BatchExported {
            scope: source.scope.label(),
            tile_id: source.tile_id.clone(),
            true_name: identity.true_name.clone(),
            source_container_route: identity.source_container_route,
            asset_route_tile_id: identity.asset_route_tile_id,
            raw_serialized_m_name: identity.true_name,
            provenance_findings: identity.provenance_findings,
            terrain_data_path_id: identity.path_id,
            resource_bundle: resolved_source.resource,
            map_scene_bundle: source.map_scene.clone(),
            terrain_root: source.destination.clone(),
            terrain_document: format!("{}/terrain.json", source.destination),
            placement_status,
            scene_instance_document,
            scene_instance,
        }),
        blocked,
        fatal: None,
    }
}

pub(super) fn blocked_outcome(blocked: BatchBlocked) -> TileOutcome {
    TileOutcome {
        exported: None,
        blocked: vec![blocked],
        fatal: None,
    }
}

pub(super) fn fatal_outcome(message: String) -> TileOutcome {
    TileOutcome {
        exported: None,
        blocked: Vec::new(),
        fatal: Some(message),
    }
}

pub(super) fn directory_has_dong_resources(root: &Path) -> Result<bool, String> {
    let entries = fs::read_dir(root).map_err(|err| format!("{}: {err}", root.display()))?;
    Ok(entries.flatten().any(|entry| {
        entry.path().is_file()
            && entry
                .file_name()
                .to_str()
                .and_then(parse_dong_tile_name)
                .is_some()
    }))
}

pub(super) fn preflight_build(
    build_root: &Path,
    options: &NativeTerrainBatchOptions,
) -> Result<Preflight, String> {
    let mut discovered = Vec::<(String, PathBuf)>::new();
    let mut blocked = Vec::new();
    let mut entries = fs::read_dir(build_root)
        .map_err(|err| format!("could not enumerate {}: {err}", build_root.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.starts_with("DongResources_") {
            continue;
        }
        match parse_dong_tile_name(file_name) {
            Some(tile_id) => discovered.push((tile_id, path)),
            None => blocked.push(BatchBlocked {
                scope: Some("worldMap"),
                tile_id: None,
                source_path: Some(path.to_string_lossy().to_string()),
                stage: "source-preflight",
                code: "malformedDongResourcesName",
                message: format!(
                    "DongResources source does not match DongResources_<xx>_<yy>.resourceFile: {file_name}"
                ),
            }),
        }
    }
    let discovered_count = discovered.len();
    let selected = discovered
        .into_iter()
        .filter(|(tile_id, _)| {
            options.tile_filter.is_empty() || options.tile_filter.contains(tile_id)
        })
        .collect::<Vec<_>>();
    let selected_world_count = selected.len();
    let tutorial_tile_ids = (0..3)
        .flat_map(|x| (0..3).map(move |y| format!("{x:02}_{y:02}")))
        .collect::<BTreeSet<_>>();
    let selected_tutorial_ids = tutorial_tile_ids
        .iter()
        .filter(|tile_id| {
            options.tile_filter.is_empty() || options.tile_filter.contains(tile_id.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    let selected_tutorial_count = selected_tutorial_ids.len();
    let selected_count = selected_world_count + selected_tutorial_count;

    if !options.tile_filter.is_empty() {
        let mut available = selected
            .iter()
            .map(|(tile_id, _)| tile_id.clone())
            .collect::<BTreeSet<_>>();
        available.extend(selected_tutorial_ids.iter().cloned());
        for missing in options.tile_filter.difference(&available) {
            blocked.push(BatchBlocked {
                scope: Some("worldMap"),
                tile_id: Some(missing.clone()),
                source_path: None,
                stage: "source-preflight",
                code: "requestedTileMissing",
                message: format!("requested DongResources tile {missing} was not found"),
            });
        }
    }

    let mut route_groups = BTreeMap::<String, Vec<(String, PathBuf)>>::new();
    for (tile_id, path) in selected {
        let destination = format!("maps/map_{tile_id}/terrain");
        route_groups
            .entry(destination.to_lowercase())
            .or_default()
            .push((tile_id, path));
    }

    let mut candidates = Vec::new();
    for grouped in route_groups.into_values() {
        if grouped.len() != 1 {
            for (tile_id, path) in grouped {
                blocked.push(BatchBlocked {
                    scope: Some("worldMap"),
                    tile_id: Some(tile_id),
                    source_path: Some(path.to_string_lossy().to_string()),
                    stage: "source-preflight",
                    code: "destinationCollision",
                    message: "multiple DongResources sources resolve to the same case-folded semantic route"
                        .to_string(),
                });
            }
            continue;
        }
        let (tile_id, resource_path) = grouped.into_iter().next().unwrap();
        let map_scene_path = build_root.join(format!("Map_{tile_id}.unity3d"));
        if !map_scene_path.is_file() {
            blocked.push(BatchBlocked {
                scope: Some("worldMap"),
                tile_id: Some(tile_id),
                source_path: Some(resource_path.to_string_lossy().to_string()),
                stage: "source-preflight",
                code: "mapSceneMissing",
                message: format!(
                    "required map scene is missing: {}",
                    map_scene_path.display()
                ),
            });
            continue;
        }
        let resource = match source_evidence(&resource_path) {
            Ok(value) => value,
            Err(message) => {
                blocked.push(BatchBlocked {
                    scope: Some("worldMap"),
                    tile_id: Some(tile_id),
                    source_path: Some(resource_path.to_string_lossy().to_string()),
                    stage: "source-preflight",
                    code: "resourceHashFailed",
                    message,
                });
                continue;
            }
        };
        let map_scene = match source_evidence(&map_scene_path) {
            Ok(value) => value,
            Err(message) => {
                blocked.push(BatchBlocked {
                    scope: Some("worldMap"),
                    tile_id: Some(tile_id),
                    source_path: Some(map_scene_path.to_string_lossy().to_string()),
                    stage: "source-preflight",
                    code: "mapSceneHashFailed",
                    message,
                });
                continue;
            }
        };
        candidates.push(TileSource {
            scope: TerrainScope::WorldMap,
            destination: format!("maps/map_{tile_id}/terrain"),
            tile_id,
            resource,
            resource_path,
            map_scene,
            map_scene_path,
        });
    }

    let tutorial_path = build_root.join(TUTORIAL_RESOURCE_FILE);
    let discovered_tutorial_count = if tutorial_path.is_file() { 9 } else { 0 };
    if !selected_tutorial_ids.is_empty() {
        if !tutorial_path.is_file() {
            blocked.push(BatchBlocked {
                scope: Some("tutorial"),
                tile_id: None,
                source_path: Some(tutorial_path.to_string_lossy().to_string()),
                stage: "source-preflight",
                code: "tutorialResourceMissing",
                message: format!(
                    "required tutorial terrain owner is missing: {}",
                    tutorial_path.display()
                ),
            });
        } else {
            let tutorial_evidence = source_evidence(&tutorial_path)?;
            for tile_id in selected_tutorial_ids {
                let map_scene_path = build_root.join(format!("Map_{tile_id}.unity3d"));
                if !map_scene_path.is_file() {
                    blocked.push(BatchBlocked {
                        scope: Some("tutorial"),
                        tile_id: Some(tile_id),
                        source_path: Some(tutorial_evidence.path.clone()),
                        stage: "source-preflight",
                        code: "tutorialMapSceneMissing",
                        message: format!(
                            "required tutorial map scene is missing: {}",
                            map_scene_path.display()
                        ),
                    });
                    continue;
                }
                let map_scene = match source_evidence(&map_scene_path) {
                    Ok(value) => value,
                    Err(message) => {
                        blocked.push(BatchBlocked {
                            scope: Some("tutorial"),
                            tile_id: Some(tile_id),
                            source_path: Some(map_scene_path.to_string_lossy().to_string()),
                            stage: "source-preflight",
                            code: "tutorialMapSceneHashFailed",
                            message,
                        });
                        continue;
                    }
                };
                candidates.push(TileSource {
                    scope: TerrainScope::Tutorial,
                    destination: format!("world/tutorial/terrain/tiles/tile_{tile_id}"),
                    tile_id,
                    resource: tutorial_evidence.clone(),
                    resource_path: tutorial_path.clone(),
                    map_scene,
                    map_scene_path,
                });
            }
        }
    }
    candidates.sort_by(|left, right| {
        left.scope
            .label()
            .cmp(right.scope.label())
            .then_with(|| left.tile_id.cmp(&right.tile_id))
    });
    Ok(Preflight {
        discovered_count,
        discovered_tutorial_count,
        selected_count,
        selected_world_count,
        selected_tutorial_count,
        candidates,
        blocked,
    })
}

pub(super) fn normalized_dependency_requests(dependencies: Vec<String>) -> Vec<String> {
    let mut unique = BTreeMap::<String, String>::new();
    for dependency in dependencies {
        unique
            .entry(normalize_bundle_name(&dependency))
            .or_insert(dependency);
    }
    unique.into_values().collect()
}

pub(super) fn scene_dependency_alias_required(nominal: ObjectKey, target: ObjectKey) -> bool {
    nominal != target
}
