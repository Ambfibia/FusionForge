use super::*;

pub(super) fn collect_mesh_material_ids(mesh: &JsonValue, used_material_ids: &mut HashSet<String>) {
    if let Some(id) = mesh.get("materialId").and_then(JsonValue::as_str) {
        used_material_ids.insert(id.to_string());
    }
    for id in mesh
        .get("materialIds")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
    {
        used_material_ids.insert(id.to_string());
    }
}
