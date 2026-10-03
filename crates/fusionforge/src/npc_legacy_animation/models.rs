use super::*;

pub(super) fn build_model_hierarchy(
    env: &fusionforge::UnityEnvironment,
    selected_meshes: &BTreeSet<ObjectKey>,
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    renderers: &[SkinnedRenderer],
    selected_objects: Option<&BTreeSet<ObjectKey>>,
) -> JsonValue {
    let transform_by_game_object = transforms
        .values()
        .filter_map(|node| node.game_object.map(|game_object| (game_object, node.key)))
        .collect::<BTreeMap<_, _>>();
    let mut bound_transforms = renderers
        .iter()
        .filter_map(|renderer| renderer.transform)
        .collect::<BTreeSet<_>>();

    for_each_selected_object!(env, selected_objects, asset_index, asset, info, {
        if asset.object_type_name(info) != "MeshFilter" {
            continue;
        }
        let Ok(body) = asset.read_object(asset_index, info) else {
            continue;
        };
        let Some(mesh) = resolved_key(env, body.get("m_Mesh")) else {
            continue;
        };
        if !selected_meshes.contains(&mesh) {
            continue;
        }
        if let Some(transform) = resolved_key(env, body.get("m_GameObject"))
            .and_then(|game_object| transform_by_game_object.get(&game_object).copied())
        {
            bound_transforms.insert(transform);
        }
    });

    let roots = bound_transforms
        .iter()
        .filter_map(|key| character_root(*key, transforms))
        .collect::<BTreeSet<_>>();
    let included = transforms
        .keys()
        .copied()
        .filter(|key| character_root(*key, transforms).is_some_and(|root| roots.contains(&root)))
        .collect::<BTreeSet<_>>();
    let mut ordered = included.iter().copied().collect::<Vec<_>>();
    ordered.sort_by_key(|key| (ancestor_chain(transforms, *key).len(), *key));

    let nodes = ordered
        .iter()
        .filter_map(|key| {
            let node = transforms.get(key)?;
            let path = full_transform_path(transforms, *key)?;
            let parent = node
                .parent
                .filter(|parent| included.contains(parent))
                .and_then(|parent| full_transform_path(transforms, parent));
            Some(json!({
                "name": node.name,
                "path": path,
                "parent": parent,
                "translation": [-node.translation[0], node.translation[1], node.translation[2]],
                "rotation": normalize_quat([node.rotation[0], -node.rotation[1], -node.rotation[2], node.rotation[3]]),
                "scale": node.scale,
                "sourceAssetIndex": key.0,
                "transformPathId": key.1,
                "gameObjectPathId": node.game_object.map(|game_object| game_object.1),
            }))
        })
        .collect::<Vec<_>>();
    let root_values = roots
        .iter()
        .filter_map(|key| {
            let node = transforms.get(key)?;
            Some(json!({
                "name": node.name,
                "path": full_transform_path(transforms, *key)?,
                "sourceAssetIndex": key.0,
                "transformPathId": key.1,
                "gameObjectPathId": node.game_object.map(|game_object| game_object.1),
            }))
        })
        .collect::<Vec<_>>();

    json!({
        "source": "unity-transform-hierarchy",
        "roots": root_values,
        "nodes": nodes,
    })
}

pub(super) fn collect_mesh_bindings(
    env: &fusionforge::UnityEnvironment,
    selected_meshes: &BTreeSet<ObjectKey>,
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    renderers: &[SkinnedRenderer],
    selected_objects: Option<&BTreeSet<ObjectKey>>,
) -> BTreeMap<ObjectKey, Vec<JsonValue>> {
    let transform_by_game_object = transforms
        .values()
        .filter_map(|node| node.game_object.map(|game_object| (game_object, node.key)))
        .collect::<BTreeMap<_, _>>();
    let mut result = BTreeMap::<ObjectKey, Vec<JsonValue>>::new();

    for renderer in renderers {
        let transform = renderer.transform;
        result.entry(renderer.mesh).or_default().push(json!({
            "componentType": "SkinnedMeshRenderer",
            "rendererAssetIndex": renderer.key.0,
            "rendererPathId": renderer.key.1,
            "transformAssetIndex": transform.map(|key| key.0),
            "transformPathId": transform.map(|key| key.1),
            "transformPath": transform.and_then(|key| full_transform_path(transforms, key)),
            "rootTransformPathId": transform.and_then(|key| character_root(key, transforms)).map(|key| key.1),
            "rootTransformName": transform.and_then(|key| character_root(key, transforms)).and_then(|key| transforms.get(&key)).map(|node| node.name.clone()),
            "boneCount": renderer.bones.len(),
            "rootBonePathId": renderer.root_bone.map(|key| key.1),
        }));
    }

    for_each_selected_object!(env, selected_objects, asset_index, asset, info, {
        if asset.object_type_name(info) != "MeshFilter" {
            continue;
        }
        let Ok(body) = asset.read_object(asset_index, info) else {
            continue;
        };
        let Some(mesh) = resolved_key(env, body.get("m_Mesh")) else {
            continue;
        };
        if !selected_meshes.contains(&mesh) {
            continue;
        }
        let transform = resolved_key(env, body.get("m_GameObject"))
            .and_then(|game_object| transform_by_game_object.get(&game_object).copied());
        result.entry(mesh).or_default().push(json!({
                "componentType": "MeshFilter",
                "componentAssetIndex": asset_index,
                "componentPathId": info.path_id,
                "transformAssetIndex": transform.map(|key| key.0),
                "transformPathId": transform.map(|key| key.1),
                "transformPath": transform.and_then(|key| full_transform_path(transforms, key)),
                "rootTransformPathId": transform.and_then(|key| character_root(key, transforms)).map(|key| key.1),
                "rootTransformName": transform.and_then(|key| character_root(key, transforms)).and_then(|key| transforms.get(&key)).map(|node| node.name.clone()),
            }));
    });

    for bindings in result.values_mut() {
        bindings.sort_by_key(|binding| {
            (
                binding
                    .get("componentType")
                    .and_then(JsonValue::as_str)
                    .unwrap_or_default()
                    .to_string(),
                binding
                    .get("rendererPathId")
                    .or_else(|| binding.get("componentPathId"))
                    .and_then(JsonValue::as_i64)
                    .unwrap_or_default(),
            )
        });
    }
    result
}

pub(super) fn collect_rigid_mesh_skins(
    env: &fusionforge::UnityEnvironment,
    selected_meshes: &BTreeSet<ObjectKey>,
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    joint_paths: &BTreeMap<ObjectKey, String>,
    existing_skins: &BTreeMap<ObjectKey, JsonValue>,
    preferred_roots: Option<&BTreeSet<ObjectKey>>,
    selected_objects: Option<&BTreeSet<ObjectKey>>,
) -> BTreeMap<ObjectKey, JsonValue> {
    let transform_by_game_object = transforms
        .values()
        .filter_map(|node| node.game_object.map(|game_object| (game_object, node.key)))
        .collect::<BTreeMap<_, _>>();
    let mut result = BTreeMap::new();

    for_each_selected_object!(env, selected_objects, asset_index, asset, info, {
        if asset.object_type_name(info) != "MeshFilter" {
            continue;
        }
        let Ok(filter) = asset.read_object(asset_index, info) else {
            continue;
        };
        let Some(mesh) = resolved_key(env, filter.get("m_Mesh")) else {
            continue;
        };
        if !selected_meshes.contains(&mesh) || existing_skins.contains_key(&mesh) {
            continue;
        }
        let Some(game_object) = resolved_key(env, filter.get("m_GameObject")) else {
            continue;
        };
        let Some(mut current) = transform_by_game_object.get(&game_object).copied() else {
            continue;
        };
        if preferred_roots.is_some_and(|roots| {
            character_root(current, transforms).is_none_or(|root| !roots.contains(&root))
        }) {
            continue;
        }
        let joint = loop {
            if let Some(path) = joint_paths.get(&current) {
                break Some((current, path.clone()));
            }
            let Some(parent) = transforms.get(&current).and_then(|node| node.parent) else {
                break None;
            };
            current = parent;
        };
        let Some((joint_key, joint_path)) = joint else {
            continue;
        };
        let Some(rest_vertex_transform) = rest_global_matrix_in_skeleton_space(
            joint_key,
            transforms,
            joint_paths,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        ) else {
            continue;
        };
        let Some(mesh_asset) = env.assets.get(mesh.0) else {
            continue;
        };
        let Some(mesh_info) = mesh_asset.objects.get(&mesh.1) else {
            continue;
        };
        let Ok(mesh_body) = mesh_asset.read_object(mesh.0, mesh_info) else {
            continue;
        };
        let vertex_count = fusionforge::mesh_vertex_count(&mesh_body);
        if vertex_count == 0 {
            continue;
        }
        let mut bone_indices = Vec::with_capacity(vertex_count * 4);
        let mut weights = Vec::with_capacity(vertex_count * 4);
        for _ in 0..vertex_count {
            bone_indices.extend([0, 0, 0, 0]);
            weights.extend([1.0, 0.0, 0.0, 0.0]);
        }
        result.insert(
            mesh,
            json!({
                "source": "unity-rigid-mesh-attachment",
                "meshFilterAssetIndex": asset_index,
                "meshFilterPathId": info.path_id,
                "attachmentTransformPathId": joint_key.1,
                "jointPaths": [joint_path],
                "boneIndices": bone_indices,
                "weights": weights,
                // MeshFilter vertices are local to their Transform. Bake
                // that rest transform into preview geometry; the sampler
                // then applies currentGlobal * inverse(restGlobal).
                "restVertexTransform": rest_vertex_transform,
            }),
        );
    });
    result
}
