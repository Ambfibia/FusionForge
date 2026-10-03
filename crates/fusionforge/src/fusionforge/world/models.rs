use super::*;

pub(super) const WORLD_PREVIEW_MAX_SINGLE_MESH_VERTICES: usize = 90_000;

pub(super) fn mesh_preview_vertex_count(mesh: &JsonValue) -> usize {
    mesh.get("positions")
        .and_then(JsonValue::as_array)
        .map(|positions| positions.len() / 3)
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_prefab_mesh_previews(
    env: &UnityEnvironment,
    game_object_key: ObjectKey,
    parent_matrix: Matrix4,
    instance_id: &str,
    material_cache: &mut HashMap<(String, i64), JsonValue>,
    preview_materials: &mut serde_json::Map<String, JsonValue>,
    meshes: &mut Vec<JsonValue>,
    used_vertices: &mut usize,
    warnings: &mut Vec<String>,
    visited: &mut HashSet<ObjectKey>,
    depth: usize,
) {
    if depth > 96 || !visited.insert(game_object_key) {
        return;
    }

    let Ok(game_object) = env.read_object(game_object_key) else {
        return;
    };
    let mut transform_key = None;
    let mut mesh_pointer = None;
    let mut materials = Vec::<JsonValue>::new();

    for component_pointer in value_array(game_object.get("m_Component"))
        .iter()
        .filter_map(component_pointer)
    {
        let Ok(component_key) = env.resolve_pointer(component_pointer) else {
            continue;
        };
        let Some(component_info) = env
            .assets
            .get(component_key.asset)
            .and_then(|asset| asset.objects.get(&component_key.path_id))
        else {
            continue;
        };
        let component_type = env.assets[component_key.asset].object_type_name(component_info);
        match component_type.as_str() {
            "Transform" => transform_key = Some(component_key),
            "MeshFilter" => {
                if let Ok(body) = env.read_object(component_key) {
                    mesh_pointer = body.get("m_Mesh").and_then(UnityValue::as_pointer).cloned();
                }
            }
            "MeshRenderer" => {
                if let Ok(body) = env.read_object(component_key) {
                    materials = value_array(body.get("m_Materials"))
                        .iter()
                        .filter_map(UnityValue::as_pointer)
                        .map(|pointer| {
                            material_preview(env, pointer, material_cache)
                                .unwrap_or(JsonValue::Null)
                        })
                        .collect();
                }
            }
            "SkinnedMeshRenderer" => {
                if let Ok(body) = env.read_object(component_key) {
                    mesh_pointer = body.get("m_Mesh").and_then(UnityValue::as_pointer).cloned();
                    materials = value_array(body.get("m_Materials"))
                        .iter()
                        .filter_map(UnityValue::as_pointer)
                        .map(|pointer| {
                            material_preview(env, pointer, material_cache)
                                .unwrap_or(JsonValue::Null)
                        })
                        .collect();
                }
            }
            _ => {}
        }
    }

    let mut world_matrix = parent_matrix;
    let mut transform_body = None;
    if let Some(transform_key) = transform_key {
        if let Ok(body) = env.read_object(transform_key) {
            let local = compose_matrix(
                converted_position(body.get("m_LocalPosition")),
                converted_quaternion(body.get("m_LocalRotation")),
                converted_scale(body.get("m_LocalScale")),
            );
            world_matrix = mat_mul_safe(parent_matrix, local);
            transform_body = Some(body);
        }
    }

    if let Some(mesh_pointer) = mesh_pointer {
        if !materials.is_empty() {
            if let Ok(mesh_key) = env.resolve_pointer(&mesh_pointer) {
                if let Ok(mesh) = env.read_object(mesh_key) {
                    let vertex_count = mesh_vertex_count(&mesh);
                    if vertex_count > 0 {
                        if let Some(mut preview) = mesh_to_preview(
                            env,
                            &mesh,
                            env.asset_name(mesh_key.asset),
                            mesh_key.path_id,
                            Some(game_object_key.path_id),
                            world_matrix,
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
                                        "prefab:{instance_id}:{}:{}:{}",
                                        env.asset_name(mesh_key.asset),
                                        game_object_key.path_id,
                                        mesh_key.path_id
                                    )),
                                );
                                let prefab_name = object_name(&game_object);
                                if !prefab_name.is_empty() {
                                    object.insert("name".into(), json!(prefab_name));
                                }
                            }
                            meshes.push(preview);
                            *used_vertices += vertex_count;
                        }
                    }
                }
            } else if warnings.len() < 50 {
                warnings.push(format!(
                    "{}#{}: scripted prefab mesh dependency failed",
                    env.asset_name(game_object_key.asset),
                    game_object_key.path_id
                ));
            }
        }
    }

    let Some(transform_body) = transform_body else {
        return;
    };
    for child_pointer in value_array(transform_body.get("m_Children"))
        .iter()
        .filter_map(UnityValue::as_pointer)
    {
        let Ok(child_transform_key) = env.resolve_pointer(child_pointer) else {
            continue;
        };
        let Ok(child_transform) = env.read_object(child_transform_key) else {
            continue;
        };
        let Some(child_game_object_key) =
            object_key_from_pointer(env, child_transform.get("m_GameObject"))
        else {
            continue;
        };
        append_prefab_mesh_previews(
            env,
            child_game_object_key,
            world_matrix,
            instance_id,
            material_cache,
            preview_materials,
            meshes,
            used_vertices,
            warnings,
            visited,
            depth + 1,
        );
    }
}
