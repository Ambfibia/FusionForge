use super::*;

#[derive(Default)]
pub struct MeshData {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uv1: Vec<(f64, f64)>,
    pub triangles: Vec<Vec<u32>>,
}

pub fn mesh_to_obj(mesh: &UnityValue) -> Option<String> {
    let mesh_data = extract_mesh(mesh)?;
    if mesh_data.vertices.is_empty() {
        return None;
    }

    let mut output = String::new();
    for vertex in &mesh_data.vertices {
        output.push_str(&format!("v {} {} {}\n", vertex.0, vertex.1, vertex.2));
    }
    for normal in &mesh_data.normals {
        output.push_str(&format!("vn {} {} {}\n", normal.0, normal.1, normal.2));
    }
    for uv in &mesh_data.uv1 {
        output.push_str(&format!("vt {} {}\n", uv.0, uv.1));
    }

    output.push('\n');
    output.push_str(&format!("g {}\n", object_name(mesh)));
    output.push_str("s 1\n");
    for (submesh_index, triangles) in mesh_data.triangles.iter().enumerate() {
        let material_name = if mesh_data.triangles.len() == 1 {
            object_name(mesh)
        } else {
            format!("{}_{}", object_name(mesh), submesh_index)
        };
        output.push_str(&format!("usemtl {}\n", material_name));
        for face in triangles.chunks_exact(3) {
            let reversed = [face[2] + 1, face[1] + 1, face[0] + 1];
            output.push_str("f ");
            for index in reversed {
                output.push_str(&format!("{index}"));
                if !mesh_data.uv1.is_empty() || !mesh_data.normals.is_empty() {
                    output.push('/');
                    if !mesh_data.uv1.is_empty() {
                        output.push_str(&format!("{index}"));
                    }
                    if !mesh_data.normals.is_empty() {
                        output.push('/');
                        output.push_str(&format!("{index}"));
                    }
                }
                output.push(' ');
            }
            output.push('\n');
        }
        output.push('\n');
    }
    Some(output)
}

pub fn mesh_to_preview(
    env: &UnityEnvironment,
    mesh: &UnityValue,
    source_asset: &str,
    path_id: i64,
    game_object_path_id: Option<i64>,
    world_matrix: Matrix4,
    materials: &[JsonValue],
    kind: &str,
) -> Option<JsonValue> {
    mesh_to_preview_impl(
        env,
        mesh,
        source_asset,
        path_id,
        game_object_path_id,
        world_matrix,
        materials,
        kind,
        false,
    )
}

/// Exact logical-model geometry projection. Unlike the interactive preview,
/// this preserves the f64 values returned by `extract_mesh` byte-for-value:
/// no display rounding, transform bake or normal re-normalization is applied.
pub fn mesh_to_exact_source(
    env: &UnityEnvironment,
    mesh: &UnityValue,
    source_asset: &str,
    path_id: i64,
    game_object_path_id: Option<i64>,
    materials: &[JsonValue],
    kind: &str,
) -> Option<JsonValue> {
    mesh_to_preview_impl(
        env,
        mesh,
        source_asset,
        path_id,
        game_object_path_id,
        identity_matrix(),
        materials,
        kind,
        true,
    )
}

pub(super) fn mesh_to_preview_impl(
    env: &UnityEnvironment,
    mesh: &UnityValue,
    source_asset: &str,
    path_id: i64,
    game_object_path_id: Option<i64>,
    world_matrix: Matrix4,
    materials: &[JsonValue],
    kind: &str,
    exact_geometry: bool,
) -> Option<JsonValue> {
    let mesh_data = extract_mesh(mesh)?;
    if mesh_data.vertices.is_empty() {
        return None;
    }

    let mut positions = Vec::with_capacity(mesh_data.vertices.len() * 3);
    for vertex in &mesh_data.vertices {
        if exact_geometry {
            positions.extend([vertex.0, vertex.1, vertex.2]);
        } else {
            let point = transform_point(world_matrix, *vertex);
            positions.extend([round(point.0, 4), round(point.1, 4), round(point.2, 4)]);
        }
    }

    let mut normals = Vec::with_capacity(mesh_data.normals.len() * 3);
    let mut normal_transform_status = if mesh_data.normals.is_empty() {
        "absent"
    } else if exact_geometry {
        "preserved-local-space"
    } else {
        "inverse-transpose-normalized"
    };
    for normal in &mesh_data.normals {
        if exact_geometry {
            normals.extend([normal.0, normal.1, normal.2]);
        } else {
            let Some(vec) = transform_normal(world_matrix, *normal) else {
                normals.clear();
                normal_transform_status = "singular-linear-transform-normals-omitted";
                break;
            };
            normals.extend([round(vec.0, 5), round(vec.1, 5), round(vec.2, 5)]);
        }
    }

    let mut uvs = Vec::with_capacity(mesh_data.uv1.len() * 2);
    for uv in &mesh_data.uv1 {
        if exact_geometry {
            uvs.extend([uv.0, uv.1]);
        } else {
            uvs.extend([round(uv.0, 6), round(uv.1, 6)]);
        }
    }

    let material_ids = materials
        .iter()
        .map(|material| material.get("id").cloned().unwrap_or(JsonValue::Null))
        .collect::<Vec<_>>();
    let mut groups = Vec::new();
    let mut indices = Vec::new();
    for (submesh_index, submesh) in mesh_data.triangles.iter().enumerate() {
        let start = indices.len();
        indices.extend(submesh.iter().map(|value| json!(value)));
        let count = indices.len() - start;
        if count > 0 {
            let material_index = if submesh_index < materials.len() {
                submesh_index
            } else {
                materials.len().saturating_sub(1)
            };
            groups.push(json!({ "start": start, "count": count, "materialIndex": material_index }));
        }
    }
    if indices.is_empty() {
        return None;
    }
    let max_index = indices
        .iter()
        .filter_map(JsonValue::as_u64)
        .max()
        .unwrap_or(0);
    if max_index as usize >= mesh_data.vertices.len() {
        return None;
    }

    let material = materials.iter().find(|value| !value.is_null()).cloned();
    let material_summary = material.as_ref().map(|material| {
        json!({
            "name": material.get("name").and_then(JsonValue::as_str).unwrap_or(""),
            "color": material.get("color").and_then(JsonValue::as_str).unwrap_or("#8b8f76")
        })
    });
    let name = object_name(mesh);
    let _ = env;
    let mut preview = json!({
        "id": format!("{}:{}", source_asset, path_id),
        "name": name,
        "kind": kind,
        "sourceAsset": source_asset,
        "pathId": path_id,
        "gameObjectPathId": game_object_path_id,
        "positions": positions,
        "normals": if normals.len() == positions.len() { json!(normals) } else { json!([]) },
        "uvs": if uvs.len() == positions.len() / 3 * 2 { json!(uvs) } else { json!([]) },
        "indices": indices,
        "position": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "rotation": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "scale": { "x": 1.0, "y": 1.0, "z": 1.0 },
        "materialId": material.as_ref().and_then(|value| value.get("id")).cloned().unwrap_or(JsonValue::Null),
        "materialIds": material_ids,
        "groups": groups,
        "material": material_summary,
    });
    if !exact_geometry {
        preview.as_object_mut()?.insert(
            "normalTransformStatus".into(),
            json!(normal_transform_status),
        );
    }
    Some(preview)
}

pub fn extract_mesh(mesh: &UnityValue) -> Option<MeshData> {
    let compression = mesh
        .get("m_MeshCompression")
        .and_then(UnityValue::as_i64)
        .unwrap_or(0);
    if compression != 0 {
        extract_compressed_mesh(mesh)
    } else {
        extract_simple_mesh(mesh)
    }
}

pub(super) fn extract_simple_mesh(mesh: &UnityValue) -> Option<MeshData> {
    let mut data = MeshData::default();
    for vertex in value_array(mesh.get("m_Vertices")) {
        let vec = vector(Some(vertex))?;
        data.vertices.push(unity_to_native_vec3(vec));
    }
    for normal in value_array(mesh.get("m_Normals")) {
        let vec = vector(Some(normal))?;
        data.normals.push(unity_to_native_vec3(vec));
    }
    for uv in value_array(mesh.get("m_UV")) {
        let vec = vector2(Some(uv))?;
        data.uv1.push((vec.0, 1.0 - vec.1));
    }

    let index_buffer = mesh
        .get("m_IndexBuffer")
        .and_then(UnityValue::as_bytes)
        .unwrap_or(&[]);
    for submesh in value_array(mesh.get("m_SubMeshes")) {
        let first_byte = submesh
            .get("firstByte")
            .and_then(UnityValue::as_i64)
            .unwrap_or(0)
            .max(0) as usize;
        let index_count = submesh
            .get("indexCount")
            .and_then(UnityValue::as_i64)
            .unwrap_or(0)
            .max(0) as usize;
        let is_strip = submesh
            .get("isTriStrip")
            .and_then(UnityValue::as_i64)
            .unwrap_or(0)
            > 0;
        let mut indices = Vec::new();
        for i in 0..index_count {
            let offset = first_byte + i * 2;
            let Some(bytes) = index_buffer.get(offset..offset + 2) else {
                break;
            };
            indices.push(u16::from_le_bytes([bytes[0], bytes[1]]) as u32);
        }
        if is_strip {
            data.triangles.push(strip_to_triangles(&indices));
        } else {
            data.triangles.push(indices);
        }
    }
    Some(data)
}

pub(super) fn extract_compressed_mesh(mesh: &UnityValue) -> Option<MeshData> {
    let compressed = mesh.get("m_CompressedMesh")?;
    let mut data = MeshData::default();
    let vertices = read_packed_floats(compressed.get("m_Vertices")?);
    for chunk in vertices.chunks_exact(3) {
        data.vertices
            .push(unity_to_native_vec3((chunk[0], chunk[1], chunk[2])));
    }
    let normals = read_normals(
        compressed.get("m_Normals")?,
        compressed.get("m_NormalSigns")?,
    );
    for chunk in normals.chunks_exact(3) {
        data.normals
            .push(unity_to_native_vec3((chunk[0], chunk[1], chunk[2])));
    }
    let uvs = read_packed_floats(compressed.get("m_UV")?);
    // Legacy Unity stores every compressed UV channel back-to-back in the
    // single m_UV stream.  The first vertex_count * 2 floats are UV0; any
    // remaining values belong to UV1+ and must not make the primary stream
    // look longer than the vertex buffer (which previously caused UV0 to be
    // omitted from exported GLBs such as npc_ben).
    for chunk in primary_compressed_uv_channel(&uvs, data.vertices.len()).chunks_exact(2) {
        data.uv1.push((chunk[0], 1.0 - chunk[1]));
    }
    let mut triangles = read_packed_bits(compressed.get("m_Triangles")?);
    match triangles.len() % 3 {
        1 => {
            if let Some(last) = triangles.last().copied() {
                triangles.push(last);
            }
            if triangles.len() >= 2 {
                triangles.push(triangles[triangles.len() - 2]);
            }
        }
        2 => {
            if let Some(last) = triangles.last().copied() {
                triangles.push(last);
            }
        }
        _ => {}
    }
    let submeshes = value_array(mesh.get("m_SubMeshes"));
    for submesh in submeshes {
        let first_byte = submesh
            .get("firstByte")
            .and_then(UnityValue::as_i64)
            .unwrap_or(0)
            .max(0) as usize;
        let index_count = submesh
            .get("indexCount")
            .and_then(UnityValue::as_i64)
            .unwrap_or(0)
            .max(0) as usize;
        if index_count == 0 {
            continue;
        }
        let Some(start) = compressed_submesh_start(first_byte, index_count, triangles.len()) else {
            continue;
        };
        let end = (start + index_count).min(triangles.len());
        if end <= start {
            continue;
        }
        let is_strip = submesh
            .get("isTriStrip")
            .and_then(UnityValue::as_i64)
            .unwrap_or(0)
            > 0;
        let indices = &triangles[start..end];
        if is_strip {
            data.triangles.push(strip_to_triangles(indices));
        } else {
            data.triangles.push(indices.to_vec());
        }
    }

    if data.triangles.is_empty() {
        if submeshes
            .first()
            .and_then(|submesh| submesh.get("isTriStrip"))
            .and_then(UnityValue::as_i64)
            .unwrap_or(0)
            > 0
        {
            data.triangles.push(strip_to_triangles(&triangles));
        } else {
            data.triangles.push(triangles);
        }
    }
    Some(data)
}

pub fn mesh_vertex_count(mesh: &UnityValue) -> usize {
    let direct = value_array(mesh.get("m_Vertices")).len();
    if direct > 0 {
        return direct;
    }
    mesh.get("m_CompressedMesh")
        .and_then(|value| value.get("m_Vertices"))
        .and_then(|value| value.get("m_NumItems"))
        .and_then(UnityValue::as_i64)
        .unwrap_or(0)
        .max(0) as usize
        / 3
}
