use super::*;

pub fn inspect_world_bundles(options: WorldInspectOptions) -> Result<JsonValue, String> {
    let extract_root = crate::repository_root().to_path_buf()
        .join("work")
        .join("sessions")
        .join("inspect");
    let session_dir = extract_root.join(format!("session_{}", session_id()));
    fs::create_dir_all(&session_dir).map_err(|err| err.to_string())?;

    let resolved_resource = options.resource_bundle.clone().or_else(|| {
        infer_resource_bundle(options.map_bundle.as_deref(), options.build_root.as_deref())
    });

    let mut extracted_bundle_paths = HashSet::new();
    let map_side = if let Some(path) = options.map_bundle.as_ref() {
        extracted_bundle_paths.insert(normalize_path(path));
        Some(extract_bundle_to_session(path, &session_dir))
    } else {
        None
    };
    let resource_side = if let Some(path) = resolved_resource.as_ref() {
        extracted_bundle_paths.insert(normalize_path(path));
        Some(extract_bundle_to_session(path, &session_dir))
    } else {
        None
    };

    let mut context_map_sides = Vec::new();
    let mut context_resource_sides = Vec::new();
    let mut dependency_sides = Vec::new();
    let mut missing_dependency_archives = HashSet::new();
    let mut archive_indexes = HashMap::<PathBuf, HashMap<String, PathBuf>>::new();
    let mut preview_tile_ids = options
        .map_bundle
        .as_ref()
        .and_then(|path| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        })
        .into_iter()
        .collect::<HashSet<_>>();

    for context_map in collect_neighbor_map_bundles(
        options.map_bundle.as_deref(),
        options.build_root.as_deref(),
        options.neighbor_radius,
    ) {
        let normalized = normalize_path(&context_map);
        if extracted_bundle_paths.contains(&normalized) {
            continue;
        }
        extracted_bundle_paths.insert(normalized);
        if let Some(stem) = context_map.file_stem().and_then(|value| value.to_str()) {
            preview_tile_ids.insert(stem.to_string());
        }
        context_map_sides.push(extract_bundle_to_session(&context_map, &session_dir));

        let Some(context_resource) =
            infer_resource_bundle(Some(&context_map), options.build_root.as_deref())
        else {
            continue;
        };
        let normalized = normalize_path(&context_resource);
        if extracted_bundle_paths.contains(&normalized) {
            continue;
        }
        extracted_bundle_paths.insert(normalized);
        context_resource_sides.push(extract_bundle_to_session(&context_resource, &session_dir));
    }

    let mut env = UnityEnvironment::from_dir(&session_dir);
    loop {
        let deps = collect_archive_dependencies(&env);
        if !extract_archives(
            &deps,
            options.map_bundle.as_deref(),
            options.build_root.as_deref(),
            &options.repo_root,
            &session_dir,
            &mut extracted_bundle_paths,
            &mut dependency_sides,
            &mut missing_dependency_archives,
            &mut archive_indexes,
        ) {
            break;
        }
        env = UnityEnvironment::from_dir(&session_dir);
    }

    let tile_id = options.map_bundle.as_ref().and_then(|path| {
        path.file_stem()
            .and_then(|value| value.to_str())
            .map(str::to_string)
    });
    let mut scene_preview = None;
    for _ in 0..4 {
        let preview = build_scene_preview(
            &env,
            tile_id.as_deref(),
            &preview_tile_ids,
            &missing_dependency_archives,
        );
        let missing = preview
            .get("missingReferencedBundles")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter_map(JsonValue::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();
        scene_preview = Some(preview);
        if missing.is_empty()
            || !extract_archives(
                &missing,
                options.map_bundle.as_deref(),
                options.build_root.as_deref(),
                &options.repo_root,
                &session_dir,
                &mut extracted_bundle_paths,
                &mut dependency_sides,
                &mut missing_dependency_archives,
                &mut archive_indexes,
            )
        {
            break;
        }
        env = UnityEnvironment::from_dir(&session_dir);
    }

    let mut map_side = map_side;
    let mut resource_side = resource_side;
    if let Some(side) = map_side.as_mut() {
        inspect_assets_for_side(side, &env);
    }
    if let Some(side) = resource_side.as_mut() {
        inspect_assets_for_side(side, &env);
    }
    for side in &mut context_map_sides {
        inspect_assets_for_side(side, &env);
    }
    for side in &mut context_resource_sides {
        inspect_assets_for_side(side, &env);
    }
    Ok(json!({
        "coordinateSpace": "native",
        "coordinateContract": native_coordinate_contract_json(),
        "repoRoot": options.repo_root,
        "map": map_side,
        "resources": resource_side,
        "contextMaps": context_map_sides,
        "contextResources": context_resource_sides,
        "dependencies": dependency_sides,
        "missingDependencies": missing_dependency_archives.into_iter().collect::<Vec<_>>(),
        "resolvedResourceBundle": resolved_resource.map(|path| path.to_string_lossy().to_string()),
        "scenePreview": scene_preview,
    }))
}

pub(super) fn display_size(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1} MiB", size as f64 / 1024.0 / 1024.0)
    } else if size >= 1024 {
        format!("{:.1} KiB", size as f64 / 1024.0)
    } else {
        format!("{size} B")
    }
}

pub(super) fn inspect_assets_for_side(side: &mut JsonValue, env: &UnityEnvironment) {
    let extracted = side
        .get("buildtool")
        .and_then(|value| value.get("extractedFiles"))
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let names = extracted
        .iter()
        .filter_map(|value| value.get("path").and_then(JsonValue::as_str))
        .filter_map(|path| Path::new(path).file_name().and_then(|value| value.to_str()))
        .map(str::to_string)
        .collect::<HashSet<_>>();

    let mut assets = Vec::new();
    let mut side_errors = side
        .get("errors")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    for (index, asset) in env.assets.iter().enumerate() {
        if !names.contains(&asset.name) {
            continue;
        }
        match inspect_asset(env, index, asset, 24) {
            Ok(asset) => assets.push(asset),
            Err(err) => side_errors.push(json!(format!(
                "{}: fusionforge parse failed: {err}",
                asset.name
            ))),
        }
    }
    if let Some(object) = side.as_object_mut() {
        object.insert("assets".into(), JsonValue::Array(assets));
        object.insert("errors".into(), JsonValue::Array(side_errors));
    }
}

pub(super) fn summarize_named(path_id: i64, obj_type: &str, body: &UnityValue) -> JsonValue {
    let materials = value_array(body.get("m_Materials"))
        .iter()
        .filter_map(|item| pointer_summary(Some(item)))
        .collect::<Vec<_>>();
    json!({
        "reason": obj_type,
        "pathId": path_id,
        "type": obj_type,
        "name": object_name(body),
        "shader": pointer_summary(body.get("m_Shader")),
        "materials": materials,
    })
}

pub(super) fn is_water_candidate(summary: &JsonValue) -> bool {
    summary.to_string().to_lowercase().contains("water")
        || summary.to_string().to_lowercase().contains("ocean")
        || summary.to_string().to_lowercase().contains("river")
        || summary.to_string().to_lowercase().contains("lake")
        || summary.to_string().to_lowercase().contains("pond")
        || summary.to_string().to_lowercase().contains("sea")
        || summary.to_string().to_lowercase().contains("liquid")
}

pub(super) fn build_scene_preview(
    env: &UnityEnvironment,
    tile_id: Option<&str>,
    preview_tile_ids: &HashSet<String>,
    missing_dependency_archives: &HashSet<String>,
) -> JsonValue {
    let mut transform_records = HashMap::<ObjectKey, (Option<ObjectKey>, Matrix4)>::new();
    let mut gameobject_to_transform = HashMap::<ObjectKey, ObjectKey>::new();
    let mut renderer_materials = HashMap::<ObjectKey, Vec<JsonValue>>::new();
    let mut terrain_targets = Vec::<(ObjectKey, ObjectKey)>::new();
    let mut material_cache = HashMap::<(String, i64), JsonValue>::new();
    let mut preview_materials = serde_json::Map::new();
    let mut warnings = Vec::<String>::new();
    let mut missing_asset_warnings = missing_dependency_archives.clone();
    for archive in &missing_asset_warnings {
        warnings.push(format!("Missing referenced bundle: {archive}"));
    }

    for (asset_index, asset) in env.assets.iter().enumerate() {
        for (path_id, info) in &asset.objects {
            let obj_type = asset.object_type_name(info);
            if !matches!(
                obj_type.as_str(),
                "Transform" | "MeshRenderer" | "SkinnedMeshRenderer" | "TerrainCollider"
            ) {
                continue;
            }
            let Ok(body) = asset.read_object(asset_index, info) else {
                continue;
            };
            let game_object_key = object_key_from_pointer(env, body.get("m_GameObject"));
            let object_key = ObjectKey {
                asset: asset_index,
                path_id: *path_id,
            };
            match obj_type.as_str() {
                "Transform" => {
                    let parent = object_key_from_pointer(env, body.get("m_Father"));
                    let local = compose_matrix(
                        converted_position(body.get("m_LocalPosition")),
                        converted_quaternion(body.get("m_LocalRotation")),
                        converted_scale(body.get("m_LocalScale")),
                    );
                    transform_records.insert(object_key, (parent, local));
                    if let Some(game_object_key) = game_object_key {
                        gameobject_to_transform.insert(game_object_key, object_key);
                    }
                }
                "MeshRenderer" | "SkinnedMeshRenderer" => {
                    if let Some(game_object_key) = game_object_key {
                        let materials = value_array(body.get("m_Materials"))
                            .iter()
                            .filter_map(UnityValue::as_pointer)
                            .map(|pointer| {
                                material_preview(env, pointer, &mut material_cache)
                                    .unwrap_or(JsonValue::Null)
                            })
                            .collect::<Vec<_>>();
                        renderer_materials.insert(game_object_key, materials);
                    }
                }
                "TerrainCollider" => {
                    if let (Some(terrain_key), Some(game_object_key)) = (
                        object_key_from_pointer(env, body.get("m_TerrainData")),
                        game_object_key,
                    ) {
                        terrain_targets.push((terrain_key, game_object_key));
                    }
                }
                _ => {}
            }
        }
    }

    let mut world_cache = HashMap::<ObjectKey, Matrix4>::new();
    fn world_for_transform(
        key: Option<ObjectKey>,
        transform_records: &HashMap<ObjectKey, (Option<ObjectKey>, Matrix4)>,
        world_cache: &mut HashMap<ObjectKey, Matrix4>,
    ) -> Matrix4 {
        let Some(key) = key else {
            return identity_matrix();
        };
        if let Some(world) = world_cache.get(&key) {
            return *world;
        }
        let Some((parent, local)) = transform_records.get(&key).copied() else {
            return identity_matrix();
        };
        let world = mat_mul_safe(
            world_for_transform(parent, transform_records, world_cache),
            local,
        );
        world_cache.insert(key, world);
        world
    }

    fn world_for_gameobject(
        key: Option<ObjectKey>,
        gameobject_to_transform: &HashMap<ObjectKey, ObjectKey>,
        transform_records: &HashMap<ObjectKey, (Option<ObjectKey>, Matrix4)>,
        world_cache: &mut HashMap<ObjectKey, Matrix4>,
    ) -> Matrix4 {
        world_for_transform(
            key.and_then(|key| gameobject_to_transform.get(&key).copied()),
            transform_records,
            world_cache,
        )
    }

    let active_tile_ids = if preview_tile_ids.is_empty() {
        tile_id
            .into_iter()
            .map(str::to_string)
            .collect::<HashSet<_>>()
    } else {
        preview_tile_ids.clone()
    };
    let scene_asset_names = env
        .assets
        .iter()
        .filter(|asset| {
            active_tile_ids
                .iter()
                .any(|tile_id| asset.name.starts_with(&format!("BuildPlayer-{tile_id}")))
        })
        .map(|asset| asset.name.clone())
        .collect::<HashSet<_>>();
    let scene_document = build_scene_document(
        env,
        &scene_asset_names,
        &transform_records,
        &gameobject_to_transform,
        &mut material_cache,
        &mut preview_materials,
    );
    let terrain_targets_by_data = terrain_targets.iter().copied().collect::<HashMap<_, _>>();

    let mut terrain = JsonValue::Null;
    let mut terrain_previews = Vec::new();
    let mut meshes = Vec::new();
    let mut used_vertices = 0_usize;
    let mut mesh_candidates = 0_usize;
    let mut skipped_empty = 0_usize;
    let mut combined_mesh_references = 0_usize;
    let mut rendered_mesh_filters = HashSet::<ObjectKey>::new();

    for (asset_index, asset) in env.assets.iter().enumerate() {
        for (path_id, info) in &asset.objects {
            let obj_type = asset.object_type_name(info);
            if obj_type == "TerrainData" {
                let terrain_key = ObjectKey {
                    asset: asset_index,
                    path_id: *path_id,
                };
                let target = terrain_targets_by_data.get(&terrain_key).copied();
                if !terrain_targets.is_empty() && target.is_none() {
                    continue;
                }
                let terrain_tile_id = tile_id_from_scene_asset_name(&asset.name);
                if !active_tile_ids.is_empty()
                    && terrain_tile_id
                        .as_ref()
                        .is_some_and(|value| !active_tile_ids.contains(value))
                {
                    continue;
                }
                match asset.read_object(asset_index, info).ok().and_then(|body| {
                    let matrix = target.map(|game_object| {
                        world_for_gameobject(
                            Some(game_object),
                            &gameobject_to_transform,
                            &transform_records,
                            &mut world_cache,
                        )
                    });
                    terrain_to_document(env, &body, matrix)
                }) {
                    Some(doc) => {
                        terrain_previews.push(json!({ "tileId": terrain_tile_id, "terrain": doc }));
                        if terrain.is_null()
                            && (tile_id.is_none() || terrain_tile_id.as_deref() == tile_id)
                        {
                            terrain = terrain_previews.last().unwrap()["terrain"].clone();
                        }
                    }
                    None => warnings.push(format!(
                        "{}#{}: terrain preview failed",
                        asset.name, path_id
                    )),
                }
            } else if matches!(obj_type.as_str(), "MeshFilter" | "SkinnedMeshRenderer") {
                if !scene_asset_names.is_empty() && !scene_asset_names.contains(&asset.name) {
                    continue;
                }
                let component_key = ObjectKey {
                    asset: asset_index,
                    path_id: *path_id,
                };
                if obj_type == "MeshFilter" && rendered_mesh_filters.contains(&component_key) {
                    continue;
                }
                mesh_candidates += 1;
                let Ok(body) = asset.read_object(asset_index, info) else {
                    continue;
                };
                let Some(mesh_pointer) = body.get("m_Mesh").and_then(UnityValue::as_pointer) else {
                    continue;
                };
                let Ok(mesh_key) = env.resolve_pointer(mesh_pointer) else {
                    note_missing_asset(
                        &mut warnings,
                        &mut missing_asset_warnings,
                        "mesh dependency",
                    );
                    continue;
                };
                let Ok(mesh) = env.read_object(mesh_key) else {
                    continue;
                };
                let vertex_count = mesh_vertex_count(&mesh);
                if vertex_count == 0 {
                    skipped_empty += 1;
                    continue;
                }
                let game_object_key = object_key_from_pointer(env, body.get("m_GameObject"));
                let has_renderer = game_object_key
                    .is_some_and(|key| renderer_materials.contains_key(&key))
                    || obj_type == "SkinnedMeshRenderer";
                if !has_renderer {
                    continue;
                }
                let mut materials = game_object_key
                    .and_then(|key| renderer_materials.get(&key).cloned())
                    .unwrap_or_default();
                if obj_type == "SkinnedMeshRenderer" && materials.is_empty() {
                    materials = value_array(body.get("m_Materials"))
                        .iter()
                        .filter_map(UnityValue::as_pointer)
                        .map(|pointer| {
                            material_preview(env, pointer, &mut material_cache)
                                .unwrap_or(JsonValue::Null)
                        })
                        .collect();
                }
                let matrix = world_for_gameobject(
                    game_object_key,
                    &gameobject_to_transform,
                    &transform_records,
                    &mut world_cache,
                );
                if let Some(preview) = mesh_to_preview(
                    env,
                    &mesh,
                    env.asset_name(mesh_key.asset),
                    mesh_key.path_id,
                    game_object_key.map(|key| key.path_id),
                    matrix,
                    &materials,
                    "mesh",
                ) {
                    for material in &materials {
                        if let Some(id) = material.get("id").and_then(JsonValue::as_str) {
                            preview_materials.insert(id.to_string(), material.clone());
                        }
                    }
                    meshes.push(preview);
                    used_vertices += vertex_count;
                    if obj_type == "MeshFilter" {
                        rendered_mesh_filters.insert(component_key);
                    }
                }
            } else if obj_type == "MeshCollider" {
                if !scene_asset_names.is_empty() && !scene_asset_names.contains(&asset.name) {
                    continue;
                }
                let Ok(body) = asset.read_object(asset_index, info) else {
                    continue;
                };
                let Some(mesh_pointer) = body.get("m_Mesh").and_then(UnityValue::as_pointer) else {
                    continue;
                };
                let Ok(mesh_key) = env.resolve_pointer(mesh_pointer) else {
                    continue;
                };
                let Ok(mesh) = env.read_object(mesh_key) else {
                    continue;
                };
                let vertex_count = mesh_vertex_count(&mesh);
                if vertex_count == 0 {
                    skipped_empty += 1;
                    continue;
                }
                let game_object_key = object_key_from_pointer(env, body.get("m_GameObject"));
                let matrix = world_for_gameobject(
                    game_object_key,
                    &gameobject_to_transform,
                    &transform_records,
                    &mut world_cache,
                );
                if let Some(mut preview) = mesh_to_preview(
                    env,
                    &mesh,
                    env.asset_name(mesh_key.asset),
                    mesh_key.path_id,
                    game_object_key.map(|key| key.path_id),
                    matrix,
                    &[],
                    "collider",
                ) {
                    if let Some(object) = preview.as_object_mut() {
                        object.insert(
                            "id".into(),
                            json!(format!(
                                "collider:{}:{}:{}",
                                asset.name, path_id, mesh_key.path_id
                            )),
                        );
                        object.insert("name".into(), json!(object_name(&mesh)));
                    }
                    meshes.push(preview);
                    used_vertices += vertex_count;
                }
            } else if obj_type == "MonoBehaviour" {
                if !scene_asset_names.is_empty() && !scene_asset_names.contains(&asset.name) {
                    continue;
                }
                let Ok(body) = asset.read_object(asset_index, info) else {
                    continue;
                };
                let game_object_key = object_key_from_pointer(env, body.get("m_GameObject"));
                let matrix = world_for_gameobject(
                    game_object_key,
                    &gameobject_to_transform,
                    &transform_records,
                    &mut world_cache,
                );
                for combine_entry in value_array(body.get("combine")) {
                    for mesh_filter_pointer in value_array(combine_entry.get("meshes"))
                        .iter()
                        .filter_map(UnityValue::as_pointer)
                    {
                        let Ok(mesh_filter_key) = env.resolve_pointer(mesh_filter_pointer) else {
                            note_missing_asset(
                                &mut warnings,
                                &mut missing_asset_warnings,
                                "combined mesh dependency",
                            );
                            continue;
                        };
                        if rendered_mesh_filters.contains(&mesh_filter_key) {
                            continue;
                        }
                        let Some(mesh_filter_info) = env
                            .assets
                            .get(mesh_filter_key.asset)
                            .and_then(|asset| asset.objects.get(&mesh_filter_key.path_id))
                        else {
                            continue;
                        };
                        if env.assets[mesh_filter_key.asset].object_type_name(mesh_filter_info)
                            != "MeshFilter"
                        {
                            continue;
                        }
                        let Ok(mesh_filter) = env.read_object(mesh_filter_key) else {
                            continue;
                        };
                        let Some(mesh_pointer) =
                            mesh_filter.get("m_Mesh").and_then(UnityValue::as_pointer)
                        else {
                            continue;
                        };
                        let Ok(mesh_key) = env.resolve_pointer(mesh_pointer) else {
                            note_missing_asset(
                                &mut warnings,
                                &mut missing_asset_warnings,
                                "combined mesh asset dependency",
                            );
                            continue;
                        };
                        let Ok(mesh) = env.read_object(mesh_key) else {
                            continue;
                        };
                        let vertex_count = mesh_vertex_count(&mesh);
                        if vertex_count == 0 {
                            skipped_empty += 1;
                            continue;
                        }
                        let mesh_game_object_key =
                            object_key_from_pointer(env, mesh_filter.get("m_GameObject"));
                        let materials = mesh_game_object_key
                            .and_then(|key| renderer_materials.get(&key).cloned())
                            .unwrap_or_default();
                        let mesh_matrix = if mesh_game_object_key.is_some() {
                            world_for_gameobject(
                                mesh_game_object_key,
                                &gameobject_to_transform,
                                &transform_records,
                                &mut world_cache,
                            )
                        } else {
                            matrix
                        };
                        if let Some(mut preview) = mesh_to_preview(
                            env,
                            &mesh,
                            env.asset_name(mesh_key.asset),
                            mesh_key.path_id,
                            mesh_game_object_key.map(|key| key.path_id),
                            mesh_matrix,
                            &materials,
                            "mesh",
                        ) {
                            for material in &materials {
                                if let Some(id) = material.get("id").and_then(JsonValue::as_str) {
                                    preview_materials.insert(id.to_string(), material.clone());
                                }
                            }
                            if let Some(object) = preview.as_object_mut() {
                                object.insert(
                                    "id".into(),
                                    json!(format!(
                                        "combined:{}:{}:{}:{}",
                                        asset.name,
                                        path_id,
                                        mesh_filter_key.path_id,
                                        mesh_key.path_id
                                    )),
                                );
                                let mesh_name = object_name(&mesh);
                                if !mesh_name.is_empty() {
                                    object.insert("name".into(), json!(mesh_name));
                                }
                            }
                            meshes.push(preview);
                            used_vertices += vertex_count;
                            combined_mesh_references += 1;
                            rendered_mesh_filters.insert(mesh_filter_key);
                        }
                    }
                }

                if let Some(nif_pointer) = body.get("nifObject").and_then(UnityValue::as_pointer) {
                    let Ok(prefab_key) = env.resolve_pointer(nif_pointer) else {
                        note_missing_asset(
                            &mut warnings,
                            &mut missing_asset_warnings,
                            "script prefab dependency",
                        );
                        continue;
                    };
                    let effect_name = body
                        .get("effectName")
                        .and_then(UnityValue::as_str)
                        .filter(|value| !value.is_empty())
                        .unwrap_or("scripted-prefab");
                    let mut visited = HashSet::new();
                    append_prefab_mesh_previews(
                        env,
                        prefab_key,
                        matrix,
                        &format!("{}:{}:{path_id}", asset.name, effect_name),
                        &mut material_cache,
                        &mut preview_materials,
                        &mut meshes,
                        &mut used_vertices,
                        &mut warnings,
                        &mut visited,
                        0,
                    );
                }
            }
        }
    }

    if terrain.is_null() {
        if let Some(first) = terrain_previews.first() {
            terrain = first["terrain"].clone();
        }
    }
    let context_terrains = terrain_previews
        .iter()
        .filter_map(|item| item.get("terrain"))
        .filter(|item| **item != terrain)
        .cloned()
        .collect::<Vec<_>>();

    let original_preview_vertices = used_vertices;
    let original_preview_meshes = meshes.len();
    let (meshes, used_vertices, skipped_by_budget) =
        limit_scene_preview_payload(meshes, &mut preview_materials);
    if skipped_by_budget > 0 {
        warnings.push(format!(
            "Preview payload limited: showing {} / {} meshes, {} / {} vertices; skipped {} oversized or over-budget objects.",
            meshes.len(),
            original_preview_meshes,
            used_vertices,
            original_preview_vertices,
            skipped_by_budget
        ));
    }

    json!({
        "coordinateSpace": "native",
        "coordinateContract": native_coordinate_contract_json(),
        "tileId": tile_id,
        "terrain": terrain,
        "contextTerrains": context_terrains,
        "scene": scene_document,
        "terrainCount": terrain_previews.len(),
        "meshes": meshes,
        "materials": preview_materials,
        "meshCandidates": mesh_candidates,
        "skippedEmptyMeshes": skipped_empty,
        "skippedByPreviewBudget": skipped_by_budget,
        "combinedMeshReferences": combined_mesh_references,
        "previewVertices": used_vertices,
        "missingReferencedBundles": missing_asset_warnings.into_iter().collect::<Vec<_>>(),
        "warnings": warnings.into_iter().take(50).collect::<Vec<_>>(),
    })
}
