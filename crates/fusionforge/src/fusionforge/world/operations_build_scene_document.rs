use super::*;

pub(super) fn build_scene_document(
    env: &UnityEnvironment,
    scene_asset_names: &HashSet<String>,
    transform_records: &HashMap<ObjectKey, (Option<ObjectKey>, Matrix4)>,
    gameobject_to_transform: &HashMap<ObjectKey, ObjectKey>,
    material_cache: &mut HashMap<(String, i64), JsonValue>,
    preview_materials: &mut serde_json::Map<String, JsonValue>,
) -> JsonValue {
    let transform_to_gameobject = gameobject_to_transform
        .iter()
        .map(|(game_object, transform)| (*transform, *game_object))
        .collect::<HashMap<_, _>>();
    let mut world_cache = HashMap::<ObjectKey, Matrix4>::new();
    let mut nodes = Vec::<JsonValue>::new();
    let mut root_ids = Vec::<String>::new();

    for (asset_index, asset) in env.assets.iter().enumerate() {
        if !scene_asset_names.is_empty() && !scene_asset_names.contains(&asset.name) {
            continue;
        }
        for (path_id, info) in &asset.objects {
            if asset.object_type_name(info) != "GameObject" {
                continue;
            }
            let object_key = ObjectKey {
                asset: asset_index,
                path_id: *path_id,
            };
            let Ok(body) = asset.read_object(asset_index, info) else {
                continue;
            };
            let transform_key = gameobject_to_transform.get(&object_key).copied();
            let parent_transform = transform_key
                .and_then(|key| transform_records.get(&key).and_then(|(parent, _)| *parent));
            let parent_game_object =
                parent_transform.and_then(|key| transform_to_gameobject.get(&key).copied());
            let parent_id = parent_game_object.map(scene_node_id);
            let node_id = scene_node_id(object_key);
            if parent_id.is_none() {
                root_ids.push(node_id.clone());
            }
            let world_matrix =
                world_for_transform_record(transform_key, transform_records, &mut world_cache);
            let local_transform = transform_key
                .and_then(|key| env.read_object(key).ok())
                .map(|transform| transform_component_json(&transform, world_matrix))
                .unwrap_or_else(|| {
                    json!({
                        "coordinateSpace": "native",
                        "coordinateContract": native_coordinate_contract_json(),
                        "worldPosition": vec3_json(transform_point(world_matrix, (0.0, 0.0, 0.0)))
                    })
                });
            let mut components = Vec::<JsonValue>::new();
            for component in value_array(body.get("m_Component"))
                .iter()
                .filter_map(component_pointer)
            {
                if let Some(summary) =
                    scene_component_summary(env, component, material_cache, preview_materials)
                {
                    components.push(summary);
                }
            }
            let component_types = components
                .iter()
                .filter_map(|value| value.get("type").and_then(JsonValue::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>();
            nodes.push(json!({
                "id": node_id,
                "name": object_name(&body),
                "sourceAsset": asset.name,
                "pathId": path_id,
                "parentId": parent_id,
                "active": body.get("m_IsActive").and_then(UnityValue::as_i64).unwrap_or(1) != 0,
                "layer": body.get("m_Layer").and_then(UnityValue::as_i64),
                "tag": body.get("m_Tag").and_then(UnityValue::as_i64),
                "transform": local_transform,
                "componentTypes": component_types,
                "components": components,
            }));
        }
    }

    root_ids.sort();
    nodes.sort_by(|left, right| {
        let left_name = left.get("name").and_then(JsonValue::as_str).unwrap_or("");
        let right_name = right.get("name").and_then(JsonValue::as_str).unwrap_or("");
        left_name.cmp(right_name).then_with(|| {
            left.get("id")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .cmp(right.get("id").and_then(JsonValue::as_str).unwrap_or(""))
        })
    });

    let node_count = nodes.len();
    json!({
        "coordinateSpace": "native",
        "coordinateContract": native_coordinate_contract_json(),
        "schemaVersion": 1,
        "rootIds": root_ids,
        "nodes": nodes,
        "nodeCount": node_count,
    })
}

pub(super) fn scene_node_id(key: ObjectKey) -> String {
    key.path_id.to_string()
}

pub(super) fn world_for_transform_record(
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
        world_for_transform_record(parent, transform_records, world_cache),
        local,
    );
    world_cache.insert(key, world);
    world
}

pub(super) fn vec3_json(value: super::super::preview::Vec3) -> JsonValue {
    json!({
        "x": (value.0 * 1000000.0).round() / 1000000.0,
        "y": (value.1 * 1000000.0).round() / 1000000.0,
        "z": (value.2 * 1000000.0).round() / 1000000.0,
    })
}

pub(super) fn transform_component_json(body: &UnityValue, world_matrix: Matrix4) -> JsonValue {
    let local_position = converted_position(body.get("m_LocalPosition"));
    let local_rotation = converted_quaternion(body.get("m_LocalRotation"));
    let local_scale = converted_scale(body.get("m_LocalScale"));
    let world_position = transform_point(world_matrix, (0.0, 0.0, 0.0));
    json!({
        "coordinateSpace": "native",
        "coordinateContract": native_coordinate_contract_json(),
        "localPosition": vec3_json(local_position),
        "localRotation": quaternion_json(local_rotation),
        "localScale": vec3_json(local_scale),
        "worldPosition": vec3_json(world_position),
        "childCount": value_array(body.get("m_Children")).len(),
        "parent": pointer_summary(body.get("m_Father")),
    })
}

pub(super) fn scene_component_summary(
    env: &UnityEnvironment,
    pointer: &super::super::unity::Pointer,
    material_cache: &mut HashMap<(String, i64), JsonValue>,
    preview_materials: &mut serde_json::Map<String, JsonValue>,
) -> Option<JsonValue> {
    let key = env.resolve_pointer(pointer).ok()?;
    let asset = env.assets.get(key.asset)?;
    let info = asset.objects.get(&key.path_id)?;
    let component_type = asset.object_type_name(info);
    let body = asset.read_object(key.asset, info).ok()?;
    let mut component = json!({
        "id": format!("{}:{}", asset.name, key.path_id),
        "type": component_type,
        "sourceAsset": asset.name,
        "pathId": key.path_id,
        "name": object_name(&body),
        "gameObject": pointer_summary(body.get("m_GameObject")),
    });

    if let Some(object) = component.as_object_mut() {
        match component_type.as_str() {
            "Transform" => {
                object.insert("summary".into(), summarize_transform(key.path_id, &body));
            }
            "MeshFilter" => {
                object.insert(
                    "mesh".into(),
                    pointer_summary(body.get("m_Mesh")).unwrap_or(JsonValue::Null),
                );
            }
            "MeshRenderer" | "SkinnedMeshRenderer" => {
                let mut material_ids = Vec::<JsonValue>::new();
                let mut material_refs = Vec::<JsonValue>::new();
                for material_pointer in value_array(body.get("m_Materials"))
                    .iter()
                    .filter_map(UnityValue::as_pointer)
                {
                    let preview = material_preview(env, material_pointer, material_cache)
                        .unwrap_or(JsonValue::Null);
                    if let Some(id) = preview.get("id").and_then(JsonValue::as_str) {
                        preview_materials.insert(id.to_string(), preview.clone());
                        material_ids.push(json!(id));
                        material_refs.push(json!({
                            "id": id,
                            "name": preview.get("name").cloned().unwrap_or(JsonValue::Null),
                            "color": preview.get("color").cloned().unwrap_or(JsonValue::Null),
                            "shaderName": preview.get("shaderName").cloned().unwrap_or(JsonValue::Null),
                            "alphaMode": preview.get("alphaMode").cloned().unwrap_or(JsonValue::Null),
                        }));
                    } else {
                        material_refs.push(
                            pointer_summary(Some(&UnityValue::Pointer(material_pointer.clone())))
                                .unwrap_or(JsonValue::Null),
                        );
                    }
                }
                object.insert("materialRefs".into(), JsonValue::Array(material_refs));
                object.insert("materialIds".into(), JsonValue::Array(material_ids));
                if component_type == "SkinnedMeshRenderer" {
                    object.insert(
                        "mesh".into(),
                        pointer_summary(body.get("m_Mesh")).unwrap_or(JsonValue::Null),
                    );
                }
            }
            value if value.ends_with("Collider") || value == "CharacterController" => {
                object.insert(
                    "collider".into(),
                    summarize_collider(key.path_id, &component_type, &body),
                );
            }
            "Terrain" | "TerrainCollider" => {
                object.insert(
                    "terrainData".into(),
                    pointer_summary(body.get("m_TerrainData")).unwrap_or(JsonValue::Null),
                );
            }
            "MonoBehaviour" => {
                object.insert("script".into(), mono_behaviour_summary(&body));
                object.insert("behavior".into(), classify_script_behavior(&body));
            }
            _ => {}
        }
    }
    Some(component)
}

pub(super) fn mono_behaviour_summary(body: &UnityValue) -> JsonValue {
    let fields = body
        .as_object()
        .map(|object| object.keys().take(32).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    json!({
        "name": object_name(body),
        "script": pointer_summary(body.get("m_Script")),
        "effectName": body.get("effectName").and_then(UnityValue::as_str),
        "nifObject": pointer_summary(body.get("nifObject")),
        "combineCount": value_array(body.get("combine")).len(),
        "fields": fields,
    })
}

pub(super) fn classify_script_behavior(body: &UnityValue) -> JsonValue {
    let name = object_name(body);
    let effect_name = body
        .get("effectName")
        .and_then(UnityValue::as_str)
        .unwrap_or_default();
    let fields = body
        .as_object()
        .map(|object| object.keys().cloned().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    let label = format!("{name} {effect_name} {fields}").to_ascii_lowercase();
    let has_combine = !value_array(body.get("combine")).is_empty();
    let has_prefab = body
        .get("nifObject")
        .and_then(UnityValue::as_pointer)
        .is_some();
    let mut behavior_type = "unknown";
    let mut preview_mode = "metadataOnly";
    let mut simulated = false;

    if has_combine {
        behavior_type = "combinedMesh";
        preview_mode = "staticCombinedMesh";
        simulated = true;
    } else if has_prefab {
        behavior_type = "scriptedPrefab";
        preview_mode = "prefabMesh";
        simulated = true;
    } else if label.contains("water")
        || label.contains("ocean")
        || label.contains("river")
        || label.contains("liquid")
    {
        behavior_type = "water";
        preview_mode = "uvScroll";
        simulated = true;
    } else if label.contains("move")
        || label.contains("moving")
        || label.contains("rotate")
        || label.contains("spin")
        || label.contains("path")
    {
        behavior_type = "motion";
        preview_mode = "loopHint";
        simulated = true;
    } else if label.contains("effect")
        || label.contains("particle")
        || label.contains("glow")
        || label.contains("eff_")
    {
        behavior_type = "effect";
        preview_mode = "pulse";
        simulated = true;
    }

    json!({
        "type": behavior_type,
        "previewMode": preview_mode,
        "simulated": simulated,
        "label": if name.is_empty() { effect_name } else { name.as_str() },
    })
}

pub(super) fn component_pointer(value: &UnityValue) -> Option<&super::super::unity::Pointer> {
    match value {
        UnityValue::Pair(_, right) => right.as_pointer(),
        UnityValue::Array(items) if items.len() >= 2 => items[1].as_pointer(),
        _ => value.as_pointer(),
    }
}

pub(super) fn mat_mul_safe(a: Matrix4, b: Matrix4) -> Matrix4 {
    super::super::preview::mat_mul(a, b)
}

pub(super) fn format_signed_decimal_component(value: i32, width: usize) -> String {
    if value < 0 {
        format!("-{:0width$}", value.abs(), width = width)
    } else {
        format!("{value:0width$}")
    }
}

pub(super) fn map_tile_id(x: i32, y: i32, x_width: usize, y_width: usize) -> String {
    format!(
        "Map_{}_{}",
        format_signed_decimal_component(x, x_width),
        format_signed_decimal_component(y, y_width)
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn extract_archives(
    archives: &[String],
    map_bundle: Option<&Path>,
    build_root: Option<&Path>,
    repo_root: &Path,
    session_dir: &Path,
    extracted_bundle_paths: &mut HashSet<String>,
    dependency_sides: &mut Vec<JsonValue>,
    missing_dependency_archives: &mut HashSet<String>,
    archive_indexes: &mut HashMap<PathBuf, HashMap<String, PathBuf>>,
) -> bool {
    let mut extracted_any = false;
    let fallback = map_bundle.and_then(Path::parent).unwrap_or(repo_root);
    for archive in archives {
        let Some(dep_path) =
            find_bundle_for_archive(archive, build_root, fallback, repo_root, archive_indexes)
        else {
            missing_dependency_archives.insert(archive.clone());
            continue;
        };
        missing_dependency_archives.remove(archive);
        let normalized = normalize_path(&dep_path);
        if extracted_bundle_paths.contains(&normalized) {
            continue;
        }
        extracted_bundle_paths.insert(normalized);
        let side = extract_bundle_to_session(&dep_path, session_dir);
        alias_extracted_files(&side, session_dir, archive);
        dependency_sides.push(side);
        extracted_any = true;
    }
    extracted_any
}

pub(super) fn alias_extracted_files(side: &JsonValue, session_dir: &Path, archive: &str) {
    let Some(files) = side
        .get("buildtool")
        .and_then(|value| value.get("extractedFiles"))
        .and_then(JsonValue::as_array)
    else {
        return;
    };
    if files.len() != 1 {
        return;
    }
    let Some(source) = files[0]
        .get("path")
        .and_then(JsonValue::as_str)
        .map(PathBuf::from)
    else {
        return;
    };
    if !source.is_file() {
        return;
    }
    let alias = session_dir.join(archive.to_lowercase());
    if normalize_path(&source) == normalize_path(&alias) || alias.exists() {
        return;
    }
    let _ = fs::copy(source, alias);
}
