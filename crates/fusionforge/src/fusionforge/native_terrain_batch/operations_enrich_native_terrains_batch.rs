use super::*;

/// Add exact scene-owned ambience, terrain-render option bindings, and placement audit evidence
/// to an already committed native TerrainData batch.
///
/// The source batch is immutable. Every unchanged payload is hard-linked into one hidden outer
/// staging directory; every JSON document that changes is replaced by a new inode before it is
/// committed. This prevents an in-place write through a hard link from mutating the source batch.
pub fn enrich_native_terrains_batch(
    input: impl AsRef<Path>,
    source_batch_root: impl AsRef<Path>,
    output_root: impl AsRef<Path>,
) -> Result<NativeTerrainEnrichmentSummary, String> {
    let input = input.as_ref();
    let source_batch_root = source_batch_root.as_ref();
    let output_root = output_root.as_ref();
    let (build_root, _) = resolve_effective_build_root(input)?;
    if !source_batch_root.is_dir() {
        return Err(format!(
            "native terrain source batch is not a directory: {}",
            source_batch_root.display()
        ));
    }
    reject_existing_output(output_root)?;

    let source_manifest_path = source_batch_root.join("manifest.json");
    let source_plan_path = source_batch_root.join("publication-plan.json");
    let source_manifest_bytes = fs::read(&source_manifest_path)
        .map_err(|err| format!("could not read {}: {err}", source_manifest_path.display()))?;
    let source_plan_bytes = fs::read(&source_plan_path)
        .map_err(|err| format!("could not read {}: {err}", source_plan_path.display()))?;
    let source_manifest_hash_before = hash_bytes(&source_manifest_bytes);
    let source_plan_hash_before = hash_bytes(&source_plan_bytes);
    let mut manifest: JsonValue = serde_json::from_slice(&source_manifest_bytes)
        .map_err(|err| format!("could not parse {}: {err}", source_manifest_path.display()))?;
    let mut publication_plan: JsonValue = serde_json::from_slice(&source_plan_bytes)
        .map_err(|err| format!("could not parse {}: {err}", source_plan_path.display()))?;
    validate_enrichment_source_documents(&manifest, &publication_plan)?;

    let preflight = preflight_build(&build_root, &NativeTerrainBatchOptions::default())?;
    if !preflight.blocked.is_empty() {
        return Err(format!(
            "build preflight produced {} blocker(s); refusing to enrich against a different source set",
            preflight.blocked.len()
        ));
    }
    let terrain_identities =
        published_terrain_identities(source_batch_root, &manifest, &preflight.candidates)?;
    let candidates = select_enrichment_candidates(&manifest, preflight.candidates)?;
    let archive_index = build_archive_index(&build_root);
    let repo_root = std::env::current_dir().map_err(|err| err.to_string())?;

    let parent = output_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "could not create enrichment output parent {}: {err}",
            parent.display()
        )
    })?;
    let nonce = unique_nonce()?;
    let output_name = output_root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("invalid enrichment output root: {}", output_root.display()))?;
    let staging_path = parent.join(format!(
        ".{output_name}.native-terrain-enrichment-staging-{}-{nonce:x}",
        std::process::id()
    ));
    let work_path = parent.join(format!(
        ".{output_name}.native-terrain-enrichment-work-{}-{nonce:x}",
        std::process::id()
    ));
    for path in [output_root, staging_path.as_path(), work_path.as_path()] {
        if path.exists() {
            return Err(format!(
                "refusing existing native terrain enrichment path: {}",
                path.display()
            ));
        }
    }
    fs::create_dir(&work_path)
        .map_err(|err| format!("could not create {}: {err}", work_path.display()))?;
    let work = SessionDirectory::new(work_path);
    let scans = process_environment_scans_parallel(
        candidates,
        terrain_identities,
        &build_root,
        &repo_root,
        &archive_index,
        work.path(),
    )?;
    let scanned_scene_count = scans.len();

    fs::create_dir(&staging_path)
        .map_err(|err| format!("could not create {}: {err}", staging_path.display()))?;
    let mut staging = SessionDirectory::new(staging_path);
    let (hard_linked_file_count, hard_linked_byte_count) =
        hard_link_tree_exact(source_batch_root, staging.path())?;

    let mut scan_by_key = BTreeMap::new();
    for scan in scans {
        let key = (scan.scope.to_string(), scan.tile_id.clone());
        if scan_by_key.insert(key.clone(), scan).is_some() {
            return Err(format!(
                "duplicate environment scan for {} {}",
                key.0, key.1
            ));
        }
    }
    let mut enrichment_blockers = Vec::new();
    let mut exact_ambience_count = 0usize;
    let mut exact_terrain_detail_count = 0usize;
    let mut exact_placement_count = 0usize;
    let mut source_resource_scene_mismatch_count = 0usize;
    let mut environment_document_count = 0usize;

    let exported = manifest
        .get_mut("exported")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| "source batch manifest has no exported array".to_string())?;
    for exported_entry in exported {
        let scope =
            required_json_str(exported_entry, "scope", "manifest exported entry")?.to_string();
        let tile_id =
            required_json_str(exported_entry, "tileId", "manifest exported entry")?.to_string();
        let terrain_root =
            required_json_str(exported_entry, "terrainRoot", "manifest exported entry")?
                .to_string();
        let key = (scope.clone(), tile_id.clone());
        let scan = scan_by_key.remove(&key).ok_or_else(|| {
            format!("source batch exported entry has no environment scan: {scope} {tile_id}")
        })?;
        let root = staging
            .path()
            .join(path_from_forward_slashes(&terrain_root));
        if !root.is_dir() {
            return Err(format!(
                "enrichment destination root is missing: {}",
                root.display()
            ));
        }
        for sidecar in &scan.sidecars {
            let path = root.join(path_from_forward_slashes(&sidecar.relative_path));
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
            }
            write_bytes_new(&path, &sidecar.bytes)?;
        }
        let environment_relative = "environment/environment.json";
        let environment_path = root.join(path_from_forward_slashes(environment_relative));
        if let Some(parent) = environment_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
        }
        write_json_new(&environment_path, &scan.document)?;
        let environment_hash = hash_output_document(&root, environment_relative)?;
        let environment_reference = json!({
            "schema": ENVIRONMENT_SCHEMA,
            "path": environment_relative,
            "blake3": environment_hash,
            "status": scan.document.get("status").cloned().unwrap_or(JsonValue::Null),
        });
        replace_json_object_field(
            &root.join("terrain.json"),
            "environment",
            environment_reference.clone(),
        )?;
        let scene_instance_path = root.join("scene-instance.json");
        if scene_instance_path.is_file() {
            replace_json_object_field(
                &scene_instance_path,
                "environment",
                environment_reference.clone(),
            )?;
        }
        let exported_object = exported_entry
            .as_object_mut()
            .ok_or_else(|| "manifest exported entry is not an object".to_string())?;
        exported_object.insert(
            "environmentDocument".to_string(),
            JsonValue::String(format!("{terrain_root}/{environment_relative}")),
        );
        exported_object.insert(
            "environmentDocumentBlake3".to_string(),
            environment_reference["blake3"].clone(),
        );
        exported_object.insert(
            "environmentStatus".to_string(),
            environment_reference["status"].clone(),
        );

        environment_document_count += 1;
        exact_ambience_count += usize::from(scan.ambience_exact);
        exact_terrain_detail_count += usize::from(scan.terrain_detail_exact);
        exact_placement_count += usize::from(scan.placement_classification == "linkedExact");
        source_resource_scene_mismatch_count +=
            usize::from(scan.placement_classification == "sourceResourceSceneTerrainMismatch");
        enrichment_blockers.extend(scan.blocked);
    }
    if !scan_by_key.is_empty() {
        return Err(format!(
            "{} environment scans do not correspond to source batch entries",
            scan_by_key.len()
        ));
    }

    replace_manifest_blockers(&mut manifest, enrichment_blockers)?;
    let blocker_count = manifest
        .get("blocked")
        .and_then(JsonValue::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let status = if blocker_count == 0 {
        "complete"
    } else {
        "complete-with-blocked"
    };
    manifest["status"] = JsonValue::String(status.to_string());
    manifest["outputRoot"] = JsonValue::String(output_root.to_string_lossy().to_string());
    if let Some(counts) = manifest
        .get_mut("counts")
        .and_then(JsonValue::as_object_mut)
    {
        counts.insert(
            "blockedCount".to_string(),
            JsonValue::from(blocker_count as u64),
        );
    }
    manifest["enrichment"] = json!({
        "schema": ENRICHMENT_SCHEMA,
        "sourceBatchRoot": canonical_string(source_batch_root)?,
        "sourceManifestBlake3": source_manifest_hash_before,
        "sourcePublicationPlanBlake3": source_plan_hash_before,
        "environmentSchema": ENVIRONMENT_SCHEMA,
        "scannedSceneCount": scanned_scene_count,
        "environmentDocumentCount": environment_document_count,
        "exactAmbienceCount": exact_ambience_count,
        "exactTerrainDetailCount": exact_terrain_detail_count,
        "exactPlacementCount": exact_placement_count,
        "sourceResourceSceneMismatchCount": source_resource_scene_mismatch_count,
        "blockerCount": blocker_count,
        "copyContract": "unchanged payloads are hard-linked; changed documents are replaced by new inodes",
    });

    refresh_publication_plan(
        staging.path(),
        output_root,
        &manifest,
        &mut publication_plan,
    )?;
    replace_json_hardlink_safe(
        &staging.path().join("publication-plan.json"),
        &publication_plan,
    )?;
    replace_json_hardlink_safe(&staging.path().join("manifest.json"), &manifest)?;

    let source_manifest_hash_after =
        hash_bytes(&fs::read(&source_manifest_path).map_err(|err| {
            format!(
                "could not re-read {}: {err}",
                source_manifest_path.display()
            )
        })?);
    let source_plan_hash_after = hash_bytes(
        &fs::read(&source_plan_path)
            .map_err(|err| format!("could not re-read {}: {err}", source_plan_path.display()))?,
    );
    if source_manifest_hash_after != source_manifest_hash_before
        || source_plan_hash_after != source_plan_hash_before
    {
        return Err(
            "source v4 batch manifest/publication plan changed during enrichment".to_string(),
        );
    }

    reject_existing_output(output_root)?;
    fs::rename(staging.path(), output_root).map_err(|err| {
        format!(
            "could not atomically commit native terrain enrichment {} -> {}: {err}",
            staging.path().display(),
            output_root.display()
        )
    })?;
    staging.keep();
    drop(work);
    Ok(NativeTerrainEnrichmentSummary {
        schema: ENRICHMENT_SCHEMA,
        status,
        source_batch_root: canonical_string(source_batch_root)?,
        output_root: output_root.to_string_lossy().to_string(),
        source_manifest_blake3_before: source_manifest_hash_before,
        source_manifest_blake3_after: source_manifest_hash_after,
        source_publication_plan_blake3_before: source_plan_hash_before,
        source_publication_plan_blake3_after: source_plan_hash_after,
        scanned_scene_count,
        environment_document_count,
        exact_ambience_count,
        exact_terrain_detail_count,
        exact_placement_count,
        source_resource_scene_mismatch_count,
        blocker_count,
        hard_linked_file_count,
        hard_linked_byte_count,
    })
}

pub(super) fn select_enrichment_candidates(
    manifest: &JsonValue,
    candidates: Vec<TileSource>,
) -> Result<Vec<TileSource>, String> {
    let exported = manifest
        .get("exported")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "source batch manifest has no exported array".to_string())?;
    let expected = exported
        .iter()
        .map(|entry| {
            Ok((
                required_json_str(entry, "scope", "manifest exported entry")?.to_string(),
                required_json_str(entry, "tileId", "manifest exported entry")?.to_string(),
            ))
        })
        .collect::<Result<BTreeSet<_>, String>>()?;
    if expected.len() != exported.len() {
        return Err("source batch manifest has duplicate scope/tile entries".to_string());
    }
    let mut selected = Vec::with_capacity(expected.len());
    let mut found = BTreeSet::new();
    for candidate in candidates {
        let key = (
            candidate.scope.label().to_string(),
            candidate.tile_id.clone(),
        );
        if expected.contains(&key) {
            if !found.insert(key.clone()) {
                return Err(format!("build preflight has duplicate {} {}", key.0, key.1));
            }
            selected.push(candidate);
        }
    }
    let missing = expected.difference(&found).cloned().collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "source batch entries are absent from current build preflight: {missing:?}"
        ));
    }
    selected.sort_by(|left, right| {
        left.scope
            .label()
            .cmp(right.scope.label())
            .then_with(|| left.tile_id.cmp(&right.tile_id))
    });
    Ok(selected)
}

pub(super) fn process_environment_scans_parallel(
    candidates: Vec<TileSource>,
    identities: BTreeMap<(String, String), PublishedTerrainIdentity>,
    build_root: &Path,
    repo_root: &Path,
    archive_index: &HashMap<String, PathBuf>,
    work_root: &Path,
) -> Result<Vec<EnvironmentScan>, String> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = candidates.len().min(4);
    let queue = Arc::new(Mutex::new(VecDeque::from(candidates)));
    let identities = Arc::new(identities);
    let outcomes = Arc::new(Mutex::new(Vec::<EnvironmentScanOutcome>::new()));
    thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let identities = Arc::clone(&identities);
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
                let key = (source.scope.label().to_string(), source.tile_id.clone());
                let outcome = match identities.get(&key) {
                    Some(identity) => process_environment_scan(
                        &source,
                        identity,
                        build_root,
                        repo_root,
                        archive_index,
                        work_root,
                    ),
                    None => EnvironmentScanOutcome {
                        scan: None,
                        fatal: Some(format!(
                            "missing published TerrainData identity for {} {}",
                            key.0, key.1
                        )),
                    },
                };
                outcomes
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(outcome);
            });
        }
    });
    let outcomes = Arc::try_unwrap(outcomes)
        .map_err(|_| "environment scan outcomes are still shared".to_string())?
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut scans = Vec::with_capacity(outcomes.len());
    for outcome in outcomes {
        if let Some(message) = outcome.fatal {
            return Err(message);
        }
        scans.push(
            outcome
                .scan
                .ok_or_else(|| "environment scan produced no result".to_string())?,
        );
    }
    scans.sort_by(|left, right| {
        left.scope
            .cmp(right.scope)
            .then_with(|| left.tile_id.cmp(&right.tile_id))
    });
    Ok(scans)
}

pub(super) fn process_environment_scan(
    source: &TileSource,
    terrain: &PublishedTerrainIdentity,
    build_root: &Path,
    repo_root: &Path,
    archive_index: &HashMap<String, PathBuf>,
    work_root: &Path,
) -> EnvironmentScanOutcome {
    let session_path = tile_session_path(work_root, source.scope, &source.tile_id);
    let loaded = match load_map_scene(source, build_root, repo_root, archive_index, &session_path) {
        Ok(value) => value,
        Err(message) => {
            return EnvironmentScanOutcome {
                scan: None,
                fatal: Some(format!(
                    "could not load environment source {} {}: {message}",
                    source.scope.label(),
                    source.tile_id
                )),
            };
        }
    };
    match scan_map_scene_environment(&loaded, source, terrain) {
        Ok(scan) => EnvironmentScanOutcome {
            scan: Some(scan),
            fatal: None,
        },
        Err(message) => EnvironmentScanOutcome {
            scan: None,
            fatal: Some(format!(
                "could not scan environment source {} {}: {message}",
                source.scope.label(),
                source.tile_id
            )),
        },
    }
}

pub(super) fn scan_map_scene_environment(
    loaded: &LoadedMapScene,
    source: &TileSource,
    terrain: &PublishedTerrainIdentity,
) -> Result<EnvironmentScan, String> {
    let env = &loaded.env;
    let mut sidecars = Vec::new();
    let mut blocked = Vec::new();
    let (tile_x, tile_y) = parse_tile_coordinates(&source.tile_id)?;

    let dong_objects =
        find_exact_scene_script_objects(env, &loaded.map_asset_names, "DongColorSetup")?;
    let (ambience, ambience_exact) = if dong_objects.len() == 1 {
        let dong = &dong_objects[0];
        let source_object =
            exact_scene_object_document(env, dong, "dong-color-setup", &mut sidecars)?;
        let x = strict_i64(dong.body.get("x"), "DongColorSetup.x")?;
        let y = strict_i64(dong.body.get("y"), "DongColorSetup.y")?;
        let fog_depth = strict_number(dong.body.get("fogDepth"), "DongColorSetup.fogDepth")?;
        let fog_color = strict_color(dong.body.get("fogColor"), "DongColorSetup.fogColor")?;
        let sky_color = strict_color(dong.body.get("skyColor"), "DongColorSetup.skyColor")?;
        let light_color = strict_color(dong.body.get("lightColor"), "DongColorSetup.lightColor")?;
        let coordinates_match = x == tile_x && y == tile_y;
        if !coordinates_match {
            blocked.push(blocked_tile(
                source,
                "environment-extract",
                "dongColorCoordinatesMismatch",
                format!(
                    "DongColorSetup coordinates ({x},{y}) do not match map tile ({tile_x},{tile_y})"
                ),
            ));
        }
        (
            json!({
                "status": if coordinates_match { "exactSource" } else { "blockedCoordinateMismatch" },
                "gridCoordinates": [x, y],
                "fogDepth": fog_depth,
                "fogColor": fog_color,
                "skyColor": sky_color,
                "lightColor": light_color,
                "sourceObject": source_object,
            }),
            coordinates_match,
        )
    } else {
        let code = if dong_objects.is_empty() {
            "dongColorSetupMissing"
        } else {
            "dongColorSetupDuplicate"
        };
        blocked.push(blocked_tile(
            source,
            "environment-extract",
            code,
            format!(
                "expected exactly one map-scene DongColorSetup, found {}",
                dong_objects.len()
            ),
        ));
        let candidates =
            exact_scene_object_candidates(env, &dong_objects, "dong-color-setup", &mut sidecars)?;
        (
            json!({
                "status": "blocked",
                "foundCount": dong_objects.len(),
                "candidates": candidates,
            }),
            false,
        )
    };

    let detail_objects =
        find_exact_scene_script_objects(env, &loaded.map_asset_names, "TerrainDetailManager")?;
    let (terrain_detail, terrain_detail_exact) = if detail_objects.len() == 1 {
        let manager = &detail_objects[0];
        let source_object =
            exact_scene_object_document(env, manager, "terrain-detail-manager", &mut sidecars)?;
        let terrain_components =
            sibling_terrain_render_components(env, &loaded.map_asset_names, manager)?;
        if terrain_components.len() == 1 {
            let renderer = &terrain_components[0];
            let renderer_source =
                exact_scene_object_document(env, renderer, "terrain-renderer", &mut sidecars)?;
            (
                json!({
                    "status": "exactSourceAndRuntimeBindings",
                    "sourceObject": source_object,
                    "terrainRendererSourceObject": renderer_source,
                    "serializedTerrainRenderBaseline": terrain_render_contract(&renderer.body)?,
                    "runtimeOptionBindings": terrain_detail_runtime_contract(),
                }),
                true,
            )
        } else {
            blocked.push(blocked_tile(
                source,
                "environment-extract",
                if terrain_components.is_empty() {
                    "terrainDetailRendererMissing"
                } else {
                    "terrainDetailRendererDuplicate"
                },
                format!(
                    "TerrainDetailManager owner has {} sibling Terrain render components",
                    terrain_components.len()
                ),
            ));
            let candidates = exact_scene_object_candidates(
                env,
                &terrain_components,
                "terrain-renderer",
                &mut sidecars,
            )?;
            (
                json!({
                    "status": "blockedRendererLinkage",
                    "sourceObject": source_object,
                    "terrainRendererCandidates": candidates,
                    "runtimeOptionBindings": terrain_detail_runtime_contract(),
                }),
                false,
            )
        }
    } else {
        let code = if detail_objects.is_empty() {
            "terrainDetailManagerMissing"
        } else {
            "terrainDetailManagerDuplicate"
        };
        blocked.push(blocked_tile(
            source,
            "environment-extract",
            code,
            format!(
                "expected exactly one map-scene TerrainDetailManager, found {}",
                detail_objects.len()
            ),
        ));
        let candidates = exact_scene_object_candidates(
            env,
            &detail_objects,
            "terrain-detail-manager",
            &mut sidecars,
        )?;
        (
            json!({
                "status": "blocked",
                "foundCount": detail_objects.len(),
                "candidates": candidates,
                "runtimeOptionBindings": terrain_detail_runtime_contract(),
            }),
            false,
        )
    };

    let placement = exact_terrain_placement_audit(
        env,
        &loaded.map_asset_names,
        source,
        terrain,
        &mut sidecars,
    )?;
    let placement_classification = match placement
        .get("classificationCode")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| "placement audit has no classificationCode".to_string())?
    {
        "linkedExact" => "linkedExact",
        "sourceResourceSceneTerrainMismatch" => "sourceResourceSceneTerrainMismatch",
        "terrainColliderMissing" => "terrainColliderMissing",
        "terrainColliderAmbiguous" => "terrainColliderAmbiguous",
        "terrainColliderTargetUnresolved" => "terrainColliderTargetUnresolved",
        other => {
            return Err(format!(
                "unsupported terrain placement classification {other:?}"
            ))
        }
    };
    match placement_classification {
        "linkedExact" => {}
        "sourceResourceSceneTerrainMismatch" => blocked.push(blocked_tile(
            source,
            "scene-link",
            "sourceResourceSceneTerrainMismatch",
            placement
                .get("message")
                .and_then(JsonValue::as_str)
                .unwrap_or("source TerrainData and scene collider TerrainData differ")
                .to_string(),
        )),
        "terrainColliderMissing" => blocked.push(blocked_tile(
            source,
            "scene-link",
            "terrainColliderMissing",
            "map scene contains no TerrainCollider".to_string(),
        )),
        "terrainColliderAmbiguous" => blocked.push(blocked_tile(
            source,
            "scene-link",
            "terrainColliderAmbiguous",
            placement
                .get("message")
                .and_then(JsonValue::as_str)
                .unwrap_or("map scene TerrainCollider linkage is ambiguous")
                .to_string(),
        )),
        "terrainColliderTargetUnresolved" => blocked.push(blocked_tile(
            source,
            "scene-link",
            "terrainColliderTargetUnresolved",
            placement
                .get("message")
                .and_then(JsonValue::as_str)
                .unwrap_or("map scene TerrainCollider target is unresolved")
                .to_string(),
        )),
        _ => unreachable!("placement classification was normalized above"),
    }

    let status = if blocked
        .iter()
        .any(|entry| entry.stage == "environment-extract")
    {
        "blocked"
    } else if placement_classification == "linkedExact" {
        "complete"
    } else {
        "complete-with-placement-blocker"
    };
    let document = json!({
        "schema": ENVIRONMENT_SCHEMA,
        "status": status,
        "scope": source.scope.label(),
        "tileId": source.tile_id,
        "mapScene": source.map_scene,
        "ambience": ambience,
        "runtimeAmbienceContract": runtime_ambience_contract(source.scope),
        "terrainDetail": terrain_detail,
        "placementAudit": placement,
        "sourceCodeEvidence": source_code_evidence(),
    });
    Ok(EnvironmentScan {
        scope: source.scope.label(),
        tile_id: source.tile_id.clone(),
        document,
        sidecars,
        blocked,
        ambience_exact,
        terrain_detail_exact,
        placement_classification,
    })
}

pub(super) fn strict_color(value: Option<&UnityValue>, label: &str) -> Result<[f64; 4], String> {
    let object = value
        .and_then(UnityValue::as_object)
        .ok_or_else(|| format!("{label} is not a Color object"))?;
    Ok([
        strict_number(object.get("r"), &format!("{label}.r"))?,
        strict_number(object.get("g"), &format!("{label}.g"))?,
        strict_number(object.get("b"), &format!("{label}.b"))?,
        strict_number(object.get("a"), &format!("{label}.a"))?,
    ])
}

pub(super) fn strict_i64(value: Option<&UnityValue>, label: &str) -> Result<i64, String> {
    value
        .and_then(UnityValue::as_i64)
        .ok_or_else(|| format!("{label} must be an integer"))
}

pub(super) fn source_code_evidence() -> JsonValue {
    json!([
        {
            "workspacePath": "work/ilspy-b8c3-firstpass/DongColorSetup.cs",
            "sha256": "613b969c722d436bc5cafe45a058f447624d8fc121c2deba9723d8b84d5cc6a3",
            "proves": "serialized ambience fields and Start registration",
        },
        {
            "workspacePath": "work/ilspy-b8c3-firstpass/DongLoader.cs",
            "sha256": "47d5208c3e8c69736c834ccf2a779e6d82f06f6c7856583cda5215e6adb2c5b3",
            "proves": "16-wide grid, player-position bilinear sampling, missing-neighbor normalization and fallback",
        },
        {
            "workspacePath": "work/ilspy-b8c3-csharp/cnPlayerCamera.cs",
            "sha256": "f40459550766ed5829bad2e36b3866287974f7010f3fefb516f452d0c1fe080c",
            "proves": "DefaultAmbience light/fog formulas and tutorial fog blend",
        },
        {
            "workspacePath": "work/ilspy-b8c3-firstpass/TerrainDetailManager.cs",
            "sha256": "a49200ffa26c056768b01d101b0e21507ff6490fed52e1164518feb0c91cffd0",
            "proves": "graphics-option bindings applied to the sibling Terrain component",
        }
    ])
}
