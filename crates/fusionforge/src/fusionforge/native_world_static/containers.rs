use super::*;

#[derive(Debug, Clone)]
pub(super) struct GameObjectRecord {
    pub(super) key: ObjectKey,
    pub(super) name: String,
    pub(super) active: bool,
    pub(super) layer: Option<i64>,
    pub(super) tag: Option<i64>,
    pub(super) components: Vec<ComponentRecord>,
    pub(super) transform: ObjectKey,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn add_game_object_payloads(
    env: &UnityEnvironment,
    game_object: ObjectKey,
    game_object_name: &str,
    hierarchy_node_id: &str,
    world_matrix: Matrix4,
    effective_active: bool,
    mesh_filters: Option<&Vec<ObjectKey>>,
    renderers: Option<&Vec<ObjectKey>>,
    colliders: Option<&Vec<ObjectKey>>,
    payloads: &mut Vec<PendingPayload>,
    selected_meshes: &mut BTreeSet<(usize, i64)>,
    selected_material_objects: &mut BTreeSet<(usize, i64)>,
    used_payload_ids: &mut BTreeSet<String>,
    counts: &mut ExportCounts,
    instance_prefix: &str,
) -> Result<(), String> {
    let filters = mesh_filters.cloned().unwrap_or_default();
    if let Some(renderers) = renderers {
        for renderer_key in renderers {
            let renderer_type = object_type(env, *renderer_key)?;
            let renderer = env.read_object(*renderer_key).map_err(|err| {
                format!(
                    "could not read renderer {}: {err}",
                    source_id(env, *renderer_key)
                )
            })?;
            let (mesh_filter, mesh_key) = if renderer_type == "SkinnedMeshRenderer" {
                (
                    None,
                    required_mesh_pointer(
                        env,
                        renderer.get("m_Mesh"),
                        &format!("{}.m_Mesh", source_id(env, *renderer_key)),
                    )?,
                )
            } else {
                if filters.len() != 1 {
                    return Err(format!(
                        "MeshRenderer {} owner {} has {} MeshFilters; exact pairing is ambiguous",
                        source_id(env, *renderer_key),
                        source_id(env, game_object),
                        filters.len()
                    ));
                }
                let filter_key = filters[0];
                let filter = env.read_object(filter_key).map_err(|err| {
                    format!(
                        "could not read MeshFilter {}: {err}",
                        source_id(env, filter_key)
                    )
                })?;
                (
                    Some(filter_key),
                    required_mesh_pointer(
                        env,
                        filter.get("m_Mesh"),
                        &format!("{}.m_Mesh", source_id(env, filter_key)),
                    )?,
                )
            };
            ensure_mesh_type(env, mesh_key)?;
            let id = format!(
                "{instance_prefix}:visual:{}:{}",
                source_id(env, *renderer_key),
                source_id(env, mesh_key)
            );
            if !used_payload_ids.insert(id.clone()) {
                continue;
            }
            payloads.push(PendingPayload {
                id,
                name: game_object_name.to_string(),
                hierarchy_node_id: hierarchy_node_id.to_string(),
                kind: PayloadKind::Visual,
                source_component: *renderer_key,
                source_game_object: game_object,
                source_mesh: mesh_key,
                source_mesh_filter: mesh_filter,
                source_renderer: Some(*renderer_key),
                world_matrix,
                effective_active,
                component_enabled: enabled_component(&renderer),
                is_trigger: false,
            });
            selected_meshes.insert((mesh_key.asset, mesh_key.path_id));
            selected_material_objects.insert((renderer_key.asset, renderer_key.path_id));
            if let Some(filter) = mesh_filter {
                selected_material_objects.insert((filter.asset, filter.path_id));
            }
            match renderer_type.as_str() {
                "MeshRenderer" => counts.source_mesh_renderers += 1,
                "SkinnedMeshRenderer" => counts.source_skinned_mesh_renderers += 1,
                _ => unreachable!("renderer type was checked"),
            }
        }
    }
    if let Some(colliders) = colliders {
        for collider_key in colliders {
            let collider = env.read_object(*collider_key).map_err(|err| {
                format!(
                    "could not read MeshCollider {}: {err}",
                    source_id(env, *collider_key)
                )
            })?;
            let mesh_key = required_mesh_pointer(
                env,
                collider.get("m_Mesh"),
                &format!("{}.m_Mesh", source_id(env, *collider_key)),
            )?;
            ensure_mesh_type(env, mesh_key)?;
            let id = format!(
                "{instance_prefix}:collider:{}:{}",
                source_id(env, *collider_key),
                source_id(env, mesh_key)
            );
            if !used_payload_ids.insert(id.clone()) {
                continue;
            }
            payloads.push(PendingPayload {
                id,
                name: game_object_name.to_string(),
                hierarchy_node_id: hierarchy_node_id.to_string(),
                kind: PayloadKind::Collider,
                source_component: *collider_key,
                source_game_object: game_object,
                source_mesh: mesh_key,
                source_mesh_filter: None,
                source_renderer: None,
                world_matrix,
                effective_active,
                component_enabled: enabled_component(&collider),
                is_trigger: collider
                    .get("m_IsTrigger")
                    .and_then(UnityValue::as_i64)
                    .unwrap_or(0)
                    != 0,
            });
            selected_meshes.insert((mesh_key.asset, mesh_key.path_id));
            counts.source_mesh_colliders += 1;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_prefab_instance(
    env: &UnityEnvironment,
    game_object_key: ObjectKey,
    parent_matrix: Matrix4,
    parent_active: bool,
    instance_root: bool,
    instance_prefix: &str,
    parent_node_id: Option<String>,
    visited: &mut HashSet<ObjectKey>,
    hierarchy_nodes: &mut Vec<JsonValue>,
    payloads: &mut Vec<PendingPayload>,
    selected_meshes: &mut BTreeSet<(usize, i64)>,
    selected_material_objects: &mut BTreeSet<(usize, i64)>,
    used_payload_ids: &mut BTreeSet<String>,
    counts: &mut ExportCounts,
) -> Result<(), String> {
    if !visited.insert(game_object_key) {
        return Err(format!(
            "scripted prefab hierarchy contains a cycle/repeated GameObject at {}",
            source_id(env, game_object_key)
        ));
    }
    let game_object = env.read_object(game_object_key).map_err(|err| {
        format!(
            "could not read scripted prefab GameObject {}: {err}",
            source_id(env, game_object_key)
        )
    })?;
    let name = object_name(&game_object);
    let active_self = game_object
        .get("m_IsActive")
        .and_then(UnityValue::as_i64)
        .unwrap_or(1)
        != 0;
    // EffectEmitterController.Start instantiates `nifObject` as a live scene
    // instance. Every GameObject in both referenced Tutorial.resourceFile
    // hierarchies is serialized with m_IsActive=0: that is the old
    // AssetBundle's prefab-storage state, not 177 intentional child disables.
    // Materialize the complete instance just as the legacy Instantiate path
    // did, while retaining sourceActiveSelf as audit evidence.
    let runtime_active_self = scripted_prefab_runtime_active(active_self);
    let active = parent_active && runtime_active_self;
    let mut transforms = Vec::new();
    let mut filters = Vec::new();
    let mut renderers = Vec::new();
    let mut colliders = Vec::new();
    let mut components = Vec::new();
    for component in value_array(game_object.get("m_Component")) {
        let pointer = component_pointer(component).ok_or_else(|| {
            format!(
                "scripted prefab {} has a non-PPtr component",
                source_id(env, game_object_key)
            )
        })?;
        let key = env.resolve_pointer(pointer).map_err(|err| {
            format!(
                "scripted prefab {} component cannot resolve: {err}",
                source_id(env, game_object_key)
            )
        })?;
        let ty = object_type(env, key)?;
        match ty.as_str() {
            "Transform" => transforms.push(key),
            "MeshFilter" => filters.push(key),
            "MeshRenderer" | "SkinnedMeshRenderer" => renderers.push(key),
            "MeshCollider" => colliders.push(key),
            _ => {}
        }
        components.push(ComponentRecord {
            key,
            object_type: ty,
        });
    }
    if transforms.len() != 1 {
        return Err(format!(
            "scripted prefab {} has {} Transforms, expected one",
            source_id(env, game_object_key),
            transforms.len()
        ));
    }
    let transform_key = transforms[0];
    let transform = env.read_object(transform_key).map_err(|err| {
        format!(
            "could not read scripted prefab Transform {}: {err}",
            source_id(env, transform_key)
        )
    })?;
    let authored = authored_transform(&transform)?;
    // EffectEmitterController.Start uses the Instantiate(prefab, position,
    // rotation) overload, parents the clone to the emitter, and then forces
    // localScale=Vector3.one. That replaces the serialized prefab root TRS;
    // only descendant transforms retain their authored local values.
    let runtime_authored = scripted_prefab_runtime_transform(authored, instance_root);
    let local = compose_matrix(
        (
            runtime_authored.translation[0],
            runtime_authored.translation[1],
            runtime_authored.translation[2],
        ),
        (
            runtime_authored.rotation[0],
            runtime_authored.rotation[1],
            runtime_authored.rotation[2],
            runtime_authored.rotation[3],
        ),
        (
            runtime_authored.scale[0],
            runtime_authored.scale[1],
            runtime_authored.scale[2],
        ),
    );
    let world_matrix = mat_mul(parent_matrix, local);
    ensure_finite_matrix(world_matrix, "scripted prefab world matrix")?;
    let node_id = format!("{instance_prefix}:{}", source_id(env, game_object_key));
    hierarchy_nodes.push(json!({
        "id": node_id,
        "kind": "scriptedPrefabInstance",
        "name": name,
        "source": source_identity(env, game_object_key)?,
        "parentId": parent_node_id,
        "activeSelf": runtime_active_self,
        "sourceActiveSelf": active_self,
        "activeInHierarchy": active,
        "instanceRoot": instance_root,
        "activationPolicy":
            "EffectEmitterController.Instantiate materializes inactive AssetBundle prefab storage",
        "transform": {
            "source": source_identity(env, transform_key)?,
            "sourceLocalNativeTrs": authored,
            "localNativeTrs": runtime_authored,
            "worldNativeMatrix": world_matrix,
        },
        "components": components.iter().map(|component| json!({
            "source": source_identity(env, component.key).expect("validated component identity"),
            "type": component.object_type,
        })).collect::<Vec<_>>(),
    }));
    counts.scripted_prefab_nodes += 1;
    add_game_object_payloads(
        env,
        game_object_key,
        &name,
        &node_id,
        world_matrix,
        active,
        Some(&filters),
        Some(&renderers),
        Some(&colliders),
        payloads,
        selected_meshes,
        selected_material_objects,
        used_payload_ids,
        counts,
        instance_prefix,
    )?;
    for child in value_array(transform.get("m_Children")) {
        let pointer = child.as_pointer().ok_or_else(|| {
            format!(
                "scripted prefab Transform {} has a non-PPtr child",
                source_id(env, transform_key)
            )
        })?;
        let child_transform = env.resolve_pointer(pointer).map_err(|err| {
            format!(
                "scripted prefab Transform {} child cannot resolve: {err}",
                source_id(env, transform_key)
            )
        })?;
        let child_transform_body = env.read_object(child_transform).map_err(|err| {
            format!(
                "could not read scripted prefab child Transform {}: {err}",
                source_id(env, child_transform)
            )
        })?;
        let child_game_object =
            object_key_from_optional_pointer(env, child_transform_body.get("m_GameObject"))?
                .ok_or_else(|| {
                    format!(
                        "scripted prefab child Transform {} has no GameObject",
                        source_id(env, child_transform)
                    )
                })?;
        collect_prefab_instance(
            env,
            child_game_object,
            world_matrix,
            active,
            false,
            instance_prefix,
            Some(node_id.clone()),
            visited,
            hierarchy_nodes,
            payloads,
            selected_meshes,
            selected_material_objects,
            used_payload_ids,
            counts,
        )?;
    }
    Ok(())
}

pub(super) fn scripted_prefab_runtime_active(_source_active_self: bool) -> bool {
    true
}

pub(super) fn scripted_prefab_runtime_transform(source: JsonTransform, instance_root: bool) -> JsonTransform {
    if instance_root {
        IDENTITY_TRANSFORM
    } else {
        source
    }
}

pub(super) fn collect_effect_prefab_closure(
    env: &UnityEnvironment,
    root: ObjectKey,
    closures: &mut BTreeMap<String, JsonValue>,
) -> Result<String, String> {
    let id = source_id(env, root);
    if closures.contains_key(&id) {
        return Ok(id);
    }
    if object_type(env, root)? != "GameObject" {
        return Err(format!(
            "particle prefab {id} resolves to {}, expected GameObject",
            object_type(env, root)?
        ));
    }

    let mut queue = VecDeque::from([root]);
    let mut visited = HashSet::<ObjectKey>::new();
    let mut objects = Vec::<(String, i64, JsonValue)>::new();
    while let Some(key) = queue.pop_front() {
        if !visited.insert(key) {
            continue;
        }
        let body = env.read_object(key).map_err(|err| {
            format!(
                "could not read world effect closure {}: {err}",
                source_id(env, key)
            )
        })?;
        let mut pointers = Vec::new();
        collect_unity_pointers(&body, &mut pointers);
        for pointer in pointers {
            if pointer.is_null() {
                continue;
            }
            let target = env.resolve_pointer(&pointer).map_err(|err| {
                format!(
                    "world effect closure {id} has unresolved PPtr from {}: {err}",
                    source_id(env, key)
                )
            })?;
            queue.push_back(target);
        }

        let asset = env
            .assets
            .get(key.asset)
            .ok_or_else(|| format!("effect closure object uses absent asset {}", key.asset))?;
        let info = asset
            .objects
            .get(&key.path_id)
            .ok_or_else(|| format!("effect closure object {} disappeared", source_id(env, key)))?;
        let value = unity_value_to_json(&body);
        let canonical = serde_json::to_vec(&value)
            .map_err(|err| format!("could not canonicalize {}: {err}", source_id(env, key)))?;
        objects.push((
            asset.name.clone(),
            key.path_id,
            json!({
                "asset": asset.name,
                "pathId": key.path_id,
                "typeId": info.type_id,
                "classId": info.class_id,
                "objectType": asset.object_type_name(info),
                "name": object_name(&body),
                "canonicalBlake3": blake3_hex(&canonical),
                "value": value,
            }),
        ));
    }
    objects.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    closures.insert(
        id.clone(),
        json!({
            "schema": "ffone.world-effect-prefab-closure.v1",
            "id": id,
            "root": source_json(env, root)?,
            "objects": objects.into_iter().map(|(_, _, value)| value).collect::<Vec<_>>(),
        }),
    );
    Ok(id)
}

pub(super) fn collect_unity_pointers(value: &UnityValue, output: &mut Vec<super::super::unity::Pointer>) {
    match value {
        UnityValue::Pointer(pointer) => output.push(pointer.clone()),
        UnityValue::Array(values) => {
            for value in values {
                collect_unity_pointers(value, output);
            }
        }
        UnityValue::Object(values) => {
            for value in values.values() {
                collect_unity_pointers(value, output);
            }
        }
        UnityValue::Pair(left, right) => {
            collect_unity_pointers(left, output);
            collect_unity_pointers(right, output);
        }
        UnityValue::Bool(_)
        | UnityValue::Int(_)
        | UnityValue::UInt(_)
        | UnityValue::Float(_)
        | UnityValue::String(_)
        | UnityValue::Bytes(_) => {}
    }
}

pub(super) fn unity_value_to_json(value: &UnityValue) -> JsonValue {
    match value {
        UnityValue::Bool(value) => json!(value),
        UnityValue::Int(value) => json!(value),
        UnityValue::UInt(value) => json!(value),
        UnityValue::Float(value) => json!(value),
        UnityValue::String(value) => json!(value),
        UnityValue::Bytes(value) => json!({
            "base64": STANDARD.encode(value),
            "bytes": value.len(),
        }),
        UnityValue::Array(values) => {
            JsonValue::Array(values.iter().map(unity_value_to_json).collect())
        }
        UnityValue::Object(values) => JsonValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), unity_value_to_json(value)))
                .collect(),
        ),
        UnityValue::Pair(left, right) => {
            json!([unity_value_to_json(left), unity_value_to_json(right)])
        }
        UnityValue::Pointer(pointer) => json!({
            "fileId": pointer.file_id,
            "pathId": pointer.path_id,
        }),
    }
}

pub(super) fn parse_map_bundle_identity(path: &Path) -> Result<(String, String, [i32; 2]), String> {
    if path.extension().and_then(|value| value.to_str()) != Some("unity3d") {
        return Err(format!(
            "map bundle must have exact lowercase .unity3d extension: {}",
            path.display()
        ));
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("map bundle name is not UTF-8: {}", path.display()))?;
    let parts = stem.split('_').collect::<Vec<_>>();
    if parts.len() != 3
        || parts[0] != "Map"
        || parts[1].is_empty()
        || parts[2].is_empty()
        || !parts[1].bytes().all(|byte| byte.is_ascii_digit())
        || !parts[2].bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!(
            "map bundle must be named exactly Map_<decimal>_<decimal>.unity3d: {}",
            path.display()
        ));
    }
    let x = parts[1]
        .parse::<i32>()
        .map_err(|err| format!("invalid map X coordinate {:?}: {err}", parts[1]))?;
    let y = parts[2]
        .parse::<i32>()
        .map_err(|err| format!("invalid map Y coordinate {:?}: {err}", parts[2]))?;
    Ok((
        stem.to_string(),
        format!("tile_{}_{}", parts[1], parts[2]),
        [x, y],
    ))
}

pub(super) fn object_type(env: &UnityEnvironment, key: ObjectKey) -> Result<String, String> {
    let asset = env
        .assets
        .get(key.asset)
        .ok_or_else(|| format!("asset index {} is out of range", key.asset))?;
    let object = asset
        .objects
        .get(&key.path_id)
        .ok_or_else(|| format!("{}#{} does not exist", asset.name, key.path_id))?;
    Ok(asset.object_type_name(object))
}

pub(super) fn object_key_from_optional_pointer(
    env: &UnityEnvironment,
    value: Option<&UnityValue>,
) -> Result<Option<ObjectKey>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        UnityValue::Pointer(pointer) if pointer.is_null() => Ok(None),
        UnityValue::Pointer(pointer) => env.resolve_pointer(pointer).map(Some),
        _ => Err("expected a Unity PPtr".to_string()),
    }
}
