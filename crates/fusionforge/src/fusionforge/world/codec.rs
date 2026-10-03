use super::*;

pub(super) fn limit_scene_preview_payload(
    meshes: Vec<JsonValue>,
    preview_materials: &mut serde_json::Map<String, JsonValue>,
) -> (Vec<JsonValue>, usize, usize) {
    let mut kept = Vec::new();
    let mut used_vertices = 0_usize;
    let mut used_collider_vertices = 0_usize;
    let mut skipped = 0_usize;
    let mut used_material_ids = HashSet::<String>::new();

    for mesh in meshes {
        let vertex_count = mesh_preview_vertex_count(&mesh);
        let is_collider = mesh
            .get("kind")
            .and_then(JsonValue::as_str)
            .is_some_and(|kind| kind == "collider");
        let single_limit = if is_collider {
            WORLD_PREVIEW_MAX_SINGLE_MESH_VERTICES / 2
        } else {
            WORLD_PREVIEW_MAX_SINGLE_MESH_VERTICES
        };
        let over_budget = kept.len() >= WORLD_PREVIEW_MAX_MESHES
            || vertex_count > single_limit
            || used_vertices.saturating_add(vertex_count) > WORLD_PREVIEW_MAX_VERTICES
            || (is_collider
                && used_collider_vertices.saturating_add(vertex_count)
                    > WORLD_PREVIEW_MAX_COLLIDER_VERTICES);
        if over_budget {
            skipped += 1;
            continue;
        }

        used_vertices += vertex_count;
        if is_collider {
            used_collider_vertices += vertex_count;
        }
        collect_mesh_material_ids(&mesh, &mut used_material_ids);
        kept.push(mesh);
    }

    if !used_material_ids.is_empty() {
        preview_materials.retain(|key, _| used_material_ids.contains(key));
    } else {
        preview_materials.clear();
    }

    (kept, used_vertices, skipped)
}
