use super::super::*;

pub(in super::super) fn preview_bundle_container_model_with_mode(
    bundle_path: String,
    project_dir: Option<String>,
    container_paths: Vec<String>,
    exact_container_route: bool,
) -> EditorResult<serde_json::Value> {
    let profile_enabled =
        exact_container_route && std::env::var_os("FFONE_PROFILE_EXACT_MODEL_EXPORT").is_some();
    let profile_started = Instant::now();
    let bundle = PathBuf::from(&bundle_path);
    // Retain the old caller signature without reading or writing its cache directory.
    let _ = project_dir;
    let wanted = container_paths.iter().map(|path| normalized_asset_path(path))
        .filter(|path| !path.is_empty()).collect::<BTreeSet<_>>();
    let (environment, root_names) = fusionforge::direct_input::load(&bundle)?;
    let env = std::sync::Arc::new(environment);
    let exact_root_asset_names = exact_container_route.then_some(root_names);
    let profile_setup_milliseconds = profile_started.elapsed().as_millis();
    let profile_container_started = Instant::now();
    let identity = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut meshes = Vec::new();
    let mut matched_paths = Vec::<String>::new();
    let mut selected = BTreeSet::<(usize, i64)>::new();
    let mut queue = VecDeque::<(usize, i64)>::new();
    let mut external_nif_previews = Vec::new();
    let mut kfm_previews = Vec::new();
    let mut warnings = Vec::<String>::new();
    let mut exact_container_targets = Vec::<JsonValue>::new();
    let mut exact_container_target_keys = BTreeSet::<(usize, String, usize, i64)>::new();
    let mut exact_direct_hierarchy_targets = BTreeSet::<(usize, i64)>::new();

    let indexed_container_entries = cached_preview_container_entries(&env);
    for container_entry in indexed_container_entries.iter() {
        let asset_index = container_entry.asset_index;
        let Some(asset) = env.assets.get(asset_index) else {
            continue;
        };
        if exact_root_asset_names
            .as_ref()
            .is_some_and(|names| !names.contains(&asset.name))
        {
            continue;
        }
        let path = container_entry.path.as_str();
        let metadata = &container_entry.metadata;
        let preload_table = container_entry.preload_table.as_ref();
        let lower = normalized_asset_path(path);
        if !wanted.is_empty()
            && !wanted.iter().any(|expected| {
                lower == *expected
                    || (!exact_container_route && lower.ends_with(&format!("/{expected}")))
            })
        {
            continue;
        }
        matched_paths.push(path.to_string());
        if let Some(pointer) = metadata
            .get("asset")
            .and_then(fusionforge::UnityValue::as_pointer)
        {
            let exact_serialized_game_object = exact_container_route
                && env.resolve_pointer(pointer).ok().is_some_and(|key| {
                    env.assets
                        .get(key.asset)
                        .and_then(|target_asset| target_asset.objects.get(&key.path_id))
                        .is_some_and(|target_info| {
                            env.assets[key.asset].object_type_name(target_info) == "GameObject"
                        })
                });
            if exact_container_route {
                if let Ok(key) = env.resolve_pointer(pointer) {
                    if let Some((target_asset, target_info)) =
                        env.assets.get(key.asset).and_then(|target_asset| {
                            target_asset
                                .objects
                                .get(&key.path_id)
                                .map(|target_info| (target_asset, target_info))
                        })
                    {
                        let target_object_type = target_asset.object_type_name(target_info);
                        if exact_container_target_keys.insert((
                            asset_index,
                            normalized_asset_path(path),
                            key.asset,
                            key.path_id,
                        )) {
                            exact_container_targets.push(json!({
                                "proof": "directAssetBundleContainerPointer",
                                "exactRoute": path,
                                "containerAssetIndex": asset_index,
                                "containerAssetName": asset.name,
                                "assetIndex": key.asset,
                                "assetName": target_asset.name,
                                "pathId": key.path_id,
                                "objectType": target_object_type,
                            }));
                        }
                        if matches!(target_object_type.as_str(), "GameObject" | "Transform") {
                            exact_direct_hierarchy_targets.insert((key.asset, key.path_id));
                        }
                    }
                }
            }
            if lower.ends_with(".nif") {
                if !exact_serialized_game_object {
                    if let Ok(key) = env.resolve_pointer(pointer) {
                        let mut visited = BTreeSet::new();
                        if let Some(bytes) =
                            container_asset_bytes(&env, key.asset, key.path_id, &mut visited)
                        {
                            let (preload_start, preload_end) =
                                metadata_preload_range(metadata, preload_table.len());
                            let preload_materials = preload_material_previews(
                                &env,
                                &preload_table,
                                preload_start,
                                preload_end,
                            );
                            match nif_bytes_to_preview_meshes(&env, &bytes, path, &asset.name) {
                                Ok(previews) => external_nif_previews.extend(
                                    apply_exact_preload_materials_to_nif_previews(
                                        previews,
                                        &preload_materials,
                                    ),
                                ),
                                Err(err) => warnings.push(format!("{path}: {err}")),
                            }
                        } else {
                            warnings.push(format!("{path}: could not extract NIF bytes"));
                        }
                    }
                }
            } else if lower.ends_with(".kfm") {
                if let Ok(key) = env.resolve_pointer(pointer) {
                    let mut visited = BTreeSet::new();
                    let pointer_search = if exact_container_route {
                        kfm_text_asset_bytes_with_source_exact(
                            &env,
                            key.asset,
                            key.path_id,
                            &mut visited,
                        )
                    } else {
                        container_asset_bytes_with_source_budget(
                            &env,
                            key.asset,
                            key.path_id,
                            &mut visited,
                            CONTAINER_ASSET_POINTER_LIMIT,
                        )
                    };
                    match pointer_search {
                        ContainerAssetBytesSearch::Found(payload) => {
                            let references = if exact_container_route {
                                kfm_reference_paths_exact(&payload.bytes)
                            } else {
                                kfm_reference_paths(&payload.bytes)
                            };
                            kfm_previews.push(kfm_preview_from_bytes_with_references(
                                path,
                                &asset.name,
                                &payload.bytes,
                                &references,
                            ));
                            let (preload_start, preload_end) =
                                metadata_preload_range(metadata, preload_table.len());
                            external_nif_previews.extend(kfm_preload_nif_previews(
                                &env,
                                &preload_table,
                                preload_start,
                                preload_end,
                                path,
                                &asset.name,
                                &references,
                                &mut warnings,
                            ));
                        }
                        ContainerAssetBytesSearch::GraphExhausted => {
                            let target_is_game_object = env
                                .assets
                                .get(key.asset)
                                .and_then(|target_asset| {
                                    target_asset.objects.get(&key.path_id).map(|info| {
                                        target_asset.object_type_name(info) == "GameObject"
                                    })
                                })
                                .unwrap_or(false);
                            if !exact_container_route || !target_is_game_object {
                                warnings.push(format!(
                                            "{path}: exact pointer graph exhausted after {} visited nodes without a real KFM TextAsset payload",
                                            visited.len(),
                                        ));
                            }
                        }
                        ContainerAssetBytesSearch::BudgetExceeded => {
                            warnings.push(format!(
                                        "{path}: preview KFM pointer traversal exceeded safety budget {} after {} visited nodes",
                                        CONTAINER_ASSET_POINTER_LIMIT,
                                        visited.len()
                                    ));
                        }
                    }
                }
            }
            add_resolved_pointer_root(&env, pointer, &mut selected, &mut queue);
        }
        let (start, end) = metadata_preload_range(metadata, preload_table.len());
        for preload in &preload_table[start..end] {
            if let Some(pointer) = preload.as_pointer() {
                add_resolved_pointer_root(&env, pointer, &mut selected, &mut queue);
            }
        }
    }
    let profile_container_milliseconds = profile_container_started.elapsed().as_millis();
    let profile_closure_started = Instant::now();

    if !exact_container_route && external_nif_previews.is_empty() && !wanted.is_empty() {
        let tokens = preview_name_tokens(&wanted, &matched_paths);
        for (asset_index, asset) in env.assets.iter().enumerate() {
            for info in asset.objects.values() {
                if asset.object_type_name(info) != "AssetBundle" {
                    continue;
                }
                let body = asset.read_object(asset_index, info)?;
                let preload_table = fusionforge::value_array(body.get("m_PreloadTable")).to_vec();
                for entry in fusionforge::value_array(body.get("m_Container")) {
                    let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                        continue;
                    };
                    let lower = normalized_asset_path(path);
                    if !(lower.ends_with(".nif") || lower.ends_with(".kfm")) {
                        continue;
                    }
                    if matched_paths
                        .iter()
                        .any(|matched| normalized_asset_path(matched) == lower)
                    {
                        continue;
                    }
                    if !preview_path_matches_tokens(&lower, &tokens) {
                        continue;
                    }

                    matched_paths.push(path.to_string());
                    if let Some(pointer) = metadata
                        .get("asset")
                        .and_then(fusionforge::UnityValue::as_pointer)
                    {
                        if lower.ends_with(".nif") {
                            if let Ok(key) = env.resolve_pointer(pointer) {
                                let mut visited = BTreeSet::new();
                                if let Some(bytes) = container_asset_bytes(
                                    &env,
                                    key.asset,
                                    key.path_id,
                                    &mut visited,
                                ) {
                                    let (preload_start, preload_end) =
                                        metadata_preload_range(metadata, preload_table.len());
                                    let preload_materials = preload_material_previews(
                                        &env,
                                        &preload_table,
                                        preload_start,
                                        preload_end,
                                    );
                                    match nif_bytes_to_preview_meshes(
                                        &env,
                                        &bytes,
                                        path,
                                        &asset.name,
                                    ) {
                                        Ok(previews) => external_nif_previews.extend(
                                            apply_exact_preload_materials_to_nif_previews(
                                                previews,
                                                &preload_materials,
                                            ),
                                        ),
                                        Err(err) => warnings.push(format!("{path}: {err}")),
                                    }
                                }
                            }
                        } else if lower.ends_with(".kfm") {
                            if let Ok(key) = env.resolve_pointer(pointer) {
                                let mut visited = BTreeSet::new();
                                if let Some(bytes) = container_asset_bytes(
                                    &env,
                                    key.asset,
                                    key.path_id,
                                    &mut visited,
                                ) {
                                    let references = kfm_reference_paths(&bytes);
                                    kfm_previews.push(kfm_preview_from_bytes(
                                        path,
                                        &asset.name,
                                        &bytes,
                                    ));
                                    let (preload_start, preload_end) =
                                        metadata_preload_range(metadata, preload_table.len());
                                    external_nif_previews.extend(kfm_preload_nif_previews(
                                        &env,
                                        &preload_table,
                                        preload_start,
                                        preload_end,
                                        path,
                                        &asset.name,
                                        &references,
                                        &mut warnings,
                                    ));
                                }
                            }
                        }
                        add_resolved_pointer_root(&env, pointer, &mut selected, &mut queue);
                    }
                    let (start, end) = metadata_preload_range(metadata, preload_table.len());
                    for preload in &preload_table[start..end] {
                        if let Some(pointer) = preload.as_pointer() {
                            add_resolved_pointer_root(&env, pointer, &mut selected, &mut queue);
                        }
                    }
                }
            }
        }
    }

    if selected.is_empty() && wanted.is_empty() {
        for (asset_index, asset) in env.assets.iter().enumerate() {
            for info in asset.objects.values() {
                if asset.object_type_name(info) == "Mesh" {
                    selected.insert((asset_index, info.path_id));
                }
            }
        }
    }

    while let Some((asset_index, path_id)) = queue.pop_front() {
        for pointer in cached_exact_object_pointers(&env, (asset_index, path_id)).iter() {
            add_resolved_pointer_root(&env, pointer, &mut selected, &mut queue);
        }
    }
    let mut exact_direct_root_mesh_selection = None;
    if exact_container_route && exact_direct_hierarchy_targets.len() == 1 {
        let target = *exact_direct_hierarchy_targets
            .iter()
            .next()
            .expect("one exact hierarchy target");
        let mut direct_selected = BTreeSet::from([target]);
        let mut direct_queue = VecDeque::from([target]);
        while let Some((asset_index, path_id)) = direct_queue.pop_front() {
            for pointer in cached_exact_object_pointers(&env, (asset_index, path_id)).iter() {
                add_resolved_pointer_root(&env, pointer, &mut direct_selected, &mut direct_queue);
            }
        }
        let preload_meshes = preview_renderable_mesh_keys_from_selection(&env, &selected);
        let direct_meshes = preview_renderable_mesh_keys_from_selection(&env, &direct_selected);
        if !direct_meshes.is_empty() {
            let excluded_candidate_meshes =
                preload_meshes.len().saturating_sub(direct_meshes.len());
            selected = direct_selected;
            if excluded_candidate_meshes > 0 {
                exact_direct_root_mesh_selection =
                    Some((excluded_candidate_meshes, direct_meshes.len()));
            }
        }
    }
    let profile_closure_milliseconds = profile_closure_started.elapsed().as_millis();
    let profile_legacy_started = Instant::now();

    if !exact_container_route && !selected_contains_mesh(&env, &selected) {
        let tokens = preview_name_tokens(&wanted, &matched_paths);
        add_fuzzy_named_meshes(&env, &tokens, &mut selected);
    }

    let renderable_meshes = if exact_container_route {
        preview_renderable_mesh_keys_from_selection(&env, &selected)
    } else {
        preview_renderable_mesh_keys(&env)
    };
    let selected_meshes = selected
        .iter()
        .copied()
        .filter(|(asset_index, path_id)| {
            env.assets
                .get(*asset_index)
                .and_then(|asset| asset.objects.get(path_id).map(|info| (asset, info)))
                .is_some_and(|(asset, info)| asset.object_type_name(info) == "Mesh")
                && renderable_meshes.contains(&(*asset_index, *path_id))
        })
        .collect::<BTreeSet<_>>();
    let legacy_preview = if exact_container_route {
        npc_legacy_animation::build_unity_legacy_npc_preview_from_selection(
            &env,
            &selected_meshes,
            &wanted,
            &selected,
        )
    } else {
        npc_legacy_animation::build_unity_legacy_npc_preview(&env, &selected_meshes, &wanted)
    };
    let npc_legacy_animation::UnityLegacyNpcPreview {
        model_hierarchy,
        mut skeleton,
        mut animations,
        mesh_skins,
        mesh_bindings,
        selected_meshes,
        warnings: animation_warnings,
    } = legacy_preview;
    warnings.extend(animation_warnings);
    let profile_legacy_milliseconds = profile_legacy_started.elapsed().as_millis();
    let profile_material_started = Instant::now();

    let exact_material_export = exact_container_route
        .then(|| {
            logical_model_material::export_exact_mesh_materials_from_selection(
                &env,
                &selected_meshes,
                Some(&selected),
            )
        })
        .transpose()?;
    let mesh_materials = exact_material_export
        .as_ref()
        .map(|export| export.mesh_materials.clone())
        .unwrap_or_else(|| preview_mesh_materials_by_key(&env));
    let mut preview_materials = exact_material_export
        .as_ref()
        .map(|export| export.materials.clone())
        .unwrap_or_default();
    let profile_material_milliseconds = profile_material_started.elapsed().as_millis();
    let profile_mesh_started = Instant::now();
    for (asset_index, path_id) in &selected {
        let Some(asset) = env.assets.get(*asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(path_id) else {
            continue;
        };
        if asset.object_type_name(info) != "Mesh" {
            continue;
        }
        if !selected_meshes.contains(&(*asset_index, *path_id)) {
            continue;
        }
        let body = asset.read_object(*asset_index, info)?;
        let materials = mesh_materials
            .get(&(*asset_index, *path_id))
            .cloned()
            .unwrap_or_default();
        let vertex_transform = if exact_container_route {
            // Exact logical-model export keeps geometry in its authored local
            // space and preserves the Transform hierarchy separately. Baking
            // a rigid attachment pivot here would apply it twice in glTF.
            identity
        } else {
            mesh_skins
                .get(&(*asset_index, *path_id))
                .and_then(|skin| skin.get("restVertexTransform"))
                .and_then(|matrix| serde_json::from_value::<[[f64; 4]; 4]>(matrix.clone()).ok())
                .unwrap_or(identity)
        };
        let mesh_preview = if exact_container_route {
            fusionforge::mesh_to_exact_source(
                &env,
                &body,
                &asset.name,
                *path_id,
                None,
                &materials,
                "mesh",
            )
        } else {
            fusionforge::mesh_to_preview(
                &env,
                &body,
                &asset.name,
                *path_id,
                None,
                vertex_transform,
                &materials,
                "mesh",
            )
        };
        if let Some(mut preview) = mesh_preview {
            if let Some(skin) = mesh_skins.get(&(*asset_index, *path_id)) {
                if let Some(object) = preview.as_object_mut() {
                    object.insert("skin".to_string(), skin.clone());
                }
            }
            if let Some(bindings) = mesh_bindings.get(&(*asset_index, *path_id)) {
                if let Some(object) = preview.as_object_mut() {
                    object.insert("sourceBindings".to_string(), json!(bindings));
                }
            }
            for material in &materials {
                if let Some(id) = material.get("id").and_then(JsonValue::as_str) {
                    preview_materials.insert(id.to_string(), material.clone());
                }
            }
            meshes.push(preview);
        }
    }
    meshes.extend(external_nif_previews);
    for mesh in &meshes {
        let Some(material) = mesh.get("material") else {
            continue;
        };
        let Some(id) = material.get("id").and_then(JsonValue::as_str) else {
            continue;
        };
        preview_materials
            .entry(id.to_string())
            .or_insert_with(|| material.clone());
    }
    if !exact_container_route {
        let dedup = deduplicate_preview_semantic_copies(
            &mut meshes,
            &preview_materials,
            &mut animations,
            &mut skeleton,
            &mut warnings,
        );
        if dedup.meshes + dedup.animations + dedup.joints > 0 {
            warnings.push(format!(
                "Collapsed equivalent preload preview copies: {} mesh(es), {} animation(s), {} skeleton joint(s).",
                dedup.meshes, dedup.animations, dedup.joints
            ));
        }
        animations.truncate(256);
    }

    let status = if meshes.is_empty()
        && matched_paths.iter().any(|path| {
            path.to_ascii_lowercase().ends_with(".kfm")
                || path.to_ascii_lowercase().ends_with(".nif")
        }) {
        "kfm-nif-no-unity-mesh"
    } else if meshes.is_empty() {
        "no-mesh"
    } else {
        "ready"
    };

    let mut result = json!({
        "schema": if exact_container_route { "ffone.logical-model-source.v1" } else { "ffclient.model-preview.v1" },
        "selectionMode": if exact_container_route { "exact-container-route" } else { "preview" },
        "bundlePath": bundle_path,
        "containerPaths": container_paths,
        "matchedPaths": matched_paths,
        "status": status,
        "meshes": meshes,
        "materials": preview_materials,
        "kfm": kfm_previews,
        "modelHierarchy": model_hierarchy,
        "skeleton": skeleton,
        "animations": animations,
        "warnings": warnings,
    });
    if exact_container_route {
        let result = result
            .as_object_mut()
            .expect("logical-model source is an object");
        result.insert(
            "nativeCoordinateContract".to_string(),
            native_coordinate_contract_json(),
        );
        result.insert(
            "exactContainerTargets".to_string(),
            JsonValue::Array(exact_container_targets),
        );
        if let Some((excluded_candidate_meshes, selected_meshes)) = exact_direct_root_mesh_selection
        {
            let source_warning = format!(
                "Ignored {excluded_candidate_meshes} mesh(es) from unrelated character roots in the shared preload range."
            );
            result.insert(
                "exactMeshSelectionProof".to_string(),
                json!({
                    "schema": "ffone.exact-character-root-mesh-selection.v1",
                    "policy": "retain-only-meshes-under-proven-selected-character-roots",
                    "excludedCandidateMeshes": excluded_candidate_meshes,
                    "selectedMeshes": selected_meshes,
                    "sourceWarning": source_warning,
                    "warningDisposition": "resolved-as-positive-character-root-ownership-proof",
                }),
            );
        }
    }
    if let (Some(object), Some(exact)) = (result.as_object_mut(), exact_material_export) {
        object.insert("textures".to_string(), json!(exact.textures));
        object.insert(
            "nonUniformMeshMaterialBindings".to_string(),
            json!(exact.non_uniform_mesh_materials),
        );
        object.insert(
            "rendererMaterialBindings".to_string(),
            json!(exact.renderer_bindings),
        );
        object.insert(
            "exactMaterialContract".to_string(),
            json!({
                "schema": "ffone.exact-material-source.v1",
                "materialSlotsPreserveNulls": true,
                "savedPropertyOrderPreserved": true,
                "rendererMaterialBindingsAreAuthoritative": true,
                "meshLevelMaterialIdsRequireUniformRendererBindings": true,
                "texturePayload": "all-source-mip-levels-as-flip-y-only-png-with-exact-raw-layout-and-hashes",
                "previewImageProcessingApplied": false,
                "geometryPrecision": "extract_mesh-f64-no-rounding-no-transform-bake-no-normal-renormalization",
                "semanticDeduplicationApplied": false,
                "animationCountTruncated": false,
            }),
        );
    }
    if profile_enabled {
        eprintln!(
            "exact model profile: route={} setup={}ms container={}ms closure={}ms legacy={}ms material={}ms mesh+result={}ms total={}ms",
            container_paths.first().map(String::as_str).unwrap_or_default(),
            profile_setup_milliseconds,
            profile_container_milliseconds,
            profile_closure_milliseconds,
            profile_legacy_milliseconds,
            profile_material_milliseconds,
            profile_mesh_started.elapsed().as_millis(),
            profile_started.elapsed().as_millis(),
        );
    }
    Ok(result)
}

pub(in super::super) fn preview_mesh_semantic_key(mesh: &JsonValue, materials: &BTreeMap<String, JsonValue>) -> String {
    let semantic = json!({
        "name": mesh.get("name").cloned().unwrap_or(JsonValue::Null),
        "kind": mesh.get("kind").cloned().unwrap_or(JsonValue::Null),
        "positions": mesh.get("positions").cloned().unwrap_or(JsonValue::Null),
        "normals": mesh.get("normals").cloned().unwrap_or(JsonValue::Null),
        "uvs": mesh.get("uvs").cloned().unwrap_or(JsonValue::Null),
        "indices": mesh.get("indices").cloned().unwrap_or(JsonValue::Null),
        "groups": mesh.get("groups").cloned().unwrap_or(JsonValue::Null),
        "position": mesh.get("position").cloned().unwrap_or(JsonValue::Null),
        "rotation": mesh.get("rotation").cloned().unwrap_or(JsonValue::Null),
        "scale": mesh.get("scale").cloned().unwrap_or(JsonValue::Null),
        "material": preview_material_semantic_value(mesh.get("material")),
        "primaryMaterial": preview_material_reference_semantic(mesh.get("materialId"), materials),
        "materials": preview_material_references_semantic(mesh, "materialIds", materials),
        "submeshMaterials": preview_material_references_semantic(mesh, "submeshMaterialIds", materials),
        "skin": preview_skin_semantic_value(mesh.get("skin")),
    });
    serde_json::to_string(&semantic).unwrap_or_default()
}

pub(in super::super) fn exact_logical_model_environment_uncached(
    bundle: &Path,
    project: &Path,
    wanted: &BTreeSet<String>,
) -> EditorResult<(PathBuf, Rc<fusionforge::UnityEnvironment>)> {
    let extract_dir = if bundle.is_dir() {
        bundle.to_path_buf()
    } else {
        extract_bundle_cached(project, bundle)?
    };
    let mut extract_dirs = vec![extract_dir.clone()];
    extract_dirs.extend(staged_preview_sibling_asset_dirs(project, bundle));
    extract_dirs.extend(explicit_dependency_extract_dirs(project)?);
    let dependency_dirs = preview_dependency_extract_dirs(project, bundle, &extract_dirs, wanted)?;
    extract_dirs.extend(dependency_dirs);
    Ok((
        extract_dir,
        cached_unity_environment_from_extract_dirs(&extract_dirs),
    ))
}

pub(in super::super) fn preview_mesh_materials_by_key(
    env: &fusionforge::UnityEnvironment,
) -> BTreeMap<(usize, i64), Vec<JsonValue>> {
    let mut material_cache = HashMap::<(String, i64), JsonValue>::new();
    let mut mesh_filter_by_game_object = BTreeMap::<(usize, i64), (usize, i64)>::new();
    let mut renderer_materials_by_game_object = BTreeMap::<(usize, i64), Vec<JsonValue>>::new();
    let mut direct_mesh_materials = BTreeMap::<(usize, i64), Vec<JsonValue>>::new();

    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            let object_type = asset.object_type_name(info);
            if !matches!(
                object_type.as_str(),
                "MeshFilter" | "MeshRenderer" | "SkinnedMeshRenderer"
            ) {
                continue;
            }
            let Ok(body) = asset.read_object(asset_index, info) else {
                continue;
            };
            let game_object_key = body
                .get("m_GameObject")
                .and_then(fusionforge::UnityValue::as_pointer)
                .filter(|pointer| pointer_referenced_asset_is_loaded(env, pointer))
                .and_then(|pointer| env.resolve_pointer(pointer).ok())
                .map(|key| (key.asset, key.path_id));

            if object_type == "MeshFilter" {
                if let (Some(game_object), Some(mesh_key)) = (
                    game_object_key,
                    body.get("m_Mesh")
                        .and_then(fusionforge::UnityValue::as_pointer)
                        .filter(|pointer| pointer_referenced_asset_is_loaded(env, pointer))
                        .and_then(|pointer| env.resolve_pointer(pointer).ok())
                        .map(|key| (key.asset, key.path_id)),
                ) {
                    mesh_filter_by_game_object.insert(game_object, mesh_key);
                }
                continue;
            }

            let materials = fusionforge::value_array(body.get("m_Materials"))
                .iter()
                .filter_map(fusionforge::UnityValue::as_pointer)
                .filter(|pointer| pointer_referenced_asset_is_loaded(env, pointer))
                .filter_map(|pointer| {
                    fusionforge::material_preview(env, pointer, &mut material_cache)
                })
                .collect::<Vec<_>>();
            if materials.is_empty() {
                continue;
            }

            if let Some(game_object) = game_object_key {
                renderer_materials_by_game_object.insert(game_object, materials.clone());
            }
            if object_type == "SkinnedMeshRenderer" {
                if let Some(mesh_key) = body
                    .get("m_Mesh")
                    .and_then(fusionforge::UnityValue::as_pointer)
                    .filter(|pointer| pointer_referenced_asset_is_loaded(env, pointer))
                    .and_then(|pointer| env.resolve_pointer(pointer).ok())
                    .map(|key| (key.asset, key.path_id))
                {
                    direct_mesh_materials.insert(mesh_key, materials);
                }
            }
        }
    }

    for (game_object, mesh_key) in mesh_filter_by_game_object {
        if let Some(materials) = renderer_materials_by_game_object.get(&game_object) {
            direct_mesh_materials
                .entry(mesh_key)
                .or_insert_with(|| materials.clone());
        }
    }

    direct_mesh_materials
}
