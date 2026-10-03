use super::*;

pub(super) fn cook_mesh(
    body: &UnityValue,
    name: &str,
    state: &mut CookState,
) -> Result<EmittedAsset, ObjectCookError> {
    let mesh = extract_mesh(body)
        .ok_or_else(|| ObjectCookError::Object("mesh encoding is unsupported".to_string()))?;
    if mesh.vertices.is_empty() {
        return Err(ObjectCookError::Object("mesh has no positions".to_string()));
    }
    let vertex_count = mesh.vertices.len();
    if mesh
        .vertices
        .iter()
        .any(|value| !value.0.is_finite() || !value.1.is_finite() || !value.2.is_finite())
    {
        return Err(ObjectCookError::Object(
            "mesh contains a non-finite position".to_string(),
        ));
    }
    if !mesh.normals.is_empty() && mesh.normals.len() != vertex_count {
        return Err(ObjectCookError::Object(format!(
            "mesh normal count {} does not match position count {vertex_count}",
            mesh.normals.len()
        )));
    }
    if mesh
        .normals
        .iter()
        .any(|value| !value.0.is_finite() || !value.1.is_finite() || !value.2.is_finite())
    {
        return Err(ObjectCookError::Object(
            "mesh contains a non-finite normal".to_string(),
        ));
    }
    if !mesh.uv1.is_empty() && mesh.uv1.len() != vertex_count {
        return Err(ObjectCookError::Object(format!(
            "mesh UV count {} does not match position count {vertex_count}",
            mesh.uv1.len()
        )));
    }
    if mesh
        .uv1
        .iter()
        .any(|value| !value.0.is_finite() || !value.1.is_finite())
    {
        return Err(ObjectCookError::Object(
            "mesh contains a non-finite UV".to_string(),
        ));
    }
    if let Some(index) = mesh
        .triangles
        .iter()
        .flatten()
        .copied()
        .find(|index| *index as usize >= vertex_count)
    {
        return Err(ObjectCookError::Object(format!(
            "mesh index {index} is outside position count {vertex_count}"
        )));
    }
    let semantic = neutral_semantic_label(semantic_or(name, "mesh"), "mesh");
    let value = json!({
        "schema": "ffone.mesh.v1",
        "name": semantic,
        "positions": mesh.vertices.iter().map(|value| [value.0, value.1, value.2]).collect::<Vec<_>>(),
        "normals": mesh.normals.iter().map(|value| [value.0, value.1, value.2]).collect::<Vec<_>>(),
        "uvs": mesh.uv1.iter().map(|value| [value.0, value.1]).collect::<Vec<_>>(),
        "submeshes": mesh.triangles.iter().map(|indices| json!({ "indices": indices })).collect::<Vec<_>>(),
    });
    let bytes = json_bytes(&value).map_err(ObjectCookError::Object)?;
    state
        .emit_asset("meshes", &semantic, "json", ContentKind::Mesh, &bytes)
        .map_err(ObjectCookError::Output)
}
