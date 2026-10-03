use super::*;

#[derive(Debug, Clone)]
pub(super) struct GlbBuild {
    pub(super) bytes: Vec<u8>,
    pub(super) vertex_count: usize,
    pub(super) index_count: usize,
    pub(super) primitive_count: usize,
}

pub(super) fn read_exact_mesh(
    env: &UnityEnvironment,
    key: ObjectKey,
    context: &str,
) -> Result<MeshData, String> {
    ensure_mesh_type(env, key)?;
    let mesh = env.read_object(key).map_err(|err| {
        format!(
            "{context}: could not read mesh {}: {err}",
            source_id(env, key)
        )
    })?;
    let mesh = extract_mesh(&mesh)
        .ok_or_else(|| format!("{context}: mesh {} cannot be decoded", source_id(env, key)))?;
    if !mesh.normals.is_empty() && mesh.normals.len() != mesh.vertices.len() {
        return Err(format!(
            "{context}: mesh {} normal count {} differs from vertex count {}",
            source_id(env, key),
            mesh.normals.len(),
            mesh.vertices.len()
        ));
    }
    if !mesh.uv1.is_empty() && mesh.uv1.len() != mesh.vertices.len() {
        return Err(format!(
            "{context}: mesh {} UV count {} differs from vertex count {}",
            source_id(env, key),
            mesh.uv1.len(),
            mesh.vertices.len()
        ));
    }
    for (submesh, indices) in mesh.triangles.iter().enumerate() {
        if indices.len() % 3 != 0 {
            return Err(format!(
                "{context}: mesh {} submesh {submesh} index count {} is not divisible by three",
                source_id(env, key),
                indices.len()
            ));
        }
        if let Some(index) = indices
            .iter()
            .copied()
            .find(|index| *index as usize >= mesh.vertices.len())
        {
            return Err(format!(
                "{context}: mesh {} submesh {submesh} index {index} exceeds vertex count {}",
                source_id(env, key),
                mesh.vertices.len()
            ));
        }
    }
    Ok(mesh)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_single_root_glb(
    root_name: &str,
    mesh: &MeshData,
    world_matrix: Matrix4,
    kind: PayloadKind,
    material_slots: &[Option<String>],
    exact_materials: &BTreeMap<String, JsonValue>,
    texture_files: &BTreeMap<String, String>,
    source_instance_id: &str,
) -> Result<GlbBuild, String> {
    ensure_finite_matrix(world_matrix, source_instance_id)?;
    let mut positions = Vec::<[f32; 3]>::with_capacity(mesh.vertices.len());
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    for (index, vertex) in mesh.vertices.iter().enumerate() {
        let transformed = transform_point(world_matrix, *vertex);
        let value = f64_vec3_to_f32(transformed, &format!("{source_instance_id} vertex {index}"))?;
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(value[axis]);
            maximum[axis] = maximum[axis].max(value[axis]);
        }
        positions.push(value);
    }
    let mut normals = Vec::<[f32; 3]>::new();
    if kind == PayloadKind::Visual && !mesh.normals.is_empty() {
        normals.reserve(mesh.normals.len());
        for (index, normal) in mesh.normals.iter().enumerate() {
            let transformed = transform_normal(world_matrix, *normal).ok_or_else(|| {
                format!("{source_instance_id} has a singular normal transform at normal {index}")
            })?;
            normals.push(f64_vec3_to_f32(
                transformed,
                &format!("{source_instance_id} normal {index}"),
            )?);
        }
    }
    let mut uvs = Vec::<[f32; 2]>::new();
    if kind == PayloadKind::Visual && !mesh.uv1.is_empty() {
        uvs.reserve(mesh.uv1.len());
        for (index, uv) in mesh.uv1.iter().enumerate() {
            let value = [uv.0 as f32, uv.1 as f32];
            if !uv.0.is_finite()
                || !uv.1.is_finite()
                || !value[0].is_finite()
                || !value[1].is_finite()
            {
                return Err(format!(
                    "{source_instance_id} UV {index} is non-finite or outside f32"
                ));
            }
            uvs.push(value);
        }
    }

    let mut binary = Vec::<u8>::new();
    let mut buffer_views = Vec::<JsonValue>::new();
    let mut accessors = Vec::<JsonValue>::new();
    let position_accessor = append_vec3_accessor(
        &mut binary,
        &mut buffer_views,
        &mut accessors,
        &positions,
        Some((minimum, maximum)),
    )?;
    let normal_accessor = (!normals.is_empty())
        .then(|| {
            append_vec3_accessor(
                &mut binary,
                &mut buffer_views,
                &mut accessors,
                &normals,
                None,
            )
        })
        .transpose()?;
    let uv_accessor = (!uvs.is_empty())
        .then(|| append_vec2_accessor(&mut binary, &mut buffer_views, &mut accessors, &uvs))
        .transpose()?;

    let mut images = Vec::<JsonValue>::new();
    let mut textures = Vec::<JsonValue>::new();
    let mut samplers = Vec::<JsonValue>::new();
    let mut texture_index_by_id = BTreeMap::<String, usize>::new();
    let mut gltf_materials = Vec::<JsonValue>::new();
    let mut material_index_by_slot = Vec::<usize>::new();
    if kind == PayloadKind::Visual {
        let slot_count = mesh.triangles.len().max(1);
        for slot in 0..slot_count {
            let material_id = material_slots
                .get(slot)
                .or_else(|| material_slots.last())
                .and_then(|value| value.as_deref());
            let material = if let Some(material_id) = material_id {
                let exact = exact_materials.get(material_id).ok_or_else(|| {
                    format!(
                        "{source_instance_id} renderer references absent exact material {material_id:?}"
                    )
                })?;
                gltf_material_from_exact(
                    exact,
                    material_id,
                    texture_files,
                    &mut images,
                    &mut textures,
                    &mut samplers,
                    &mut texture_index_by_id,
                )?
            } else {
                json!({
                    "name": format!("{root_name}__default"),
                    "pbrMetallicRoughness": {
                        "baseColorFactor": [0.545, 0.56, 0.463, 1.0],
                        "metallicFactor": 0.0,
                        "roughnessFactor": 1.0
                    },
                    "extras": { "ffoneNullMaterialSlot": true }
                })
            };
            let material_index = gltf_materials.len();
            gltf_materials.push(material);
            material_index_by_slot.push(material_index);
        }
    }

    let source_submeshes = if kind == PayloadKind::Collider {
        vec![mesh
            .triangles
            .iter()
            .flat_map(|indices| indices.iter().copied())
            .collect::<Vec<_>>()]
    } else {
        mesh.triangles.clone()
    };
    let mut primitives = Vec::<JsonValue>::new();
    let mut total_indices = 0usize;
    for (submesh_index, indices) in source_submeshes.iter().enumerate() {
        if indices.is_empty() {
            continue;
        }
        if indices.len() % 3 != 0 {
            return Err(format!(
                "{source_instance_id} submesh {submesh_index} has non-triangle indices"
            ));
        }
        // Static-world payloads come from several Unity mesh encodings and
        // may also carry a mirrored baked world transform. Do not infer the
        // final glTF winding from H=diag(-1,1,1) alone: decide after positions
        // and normals have reached their published space. A primitive is
        // reversed as a whole when its geometric faces predominantly oppose
        // the authored vertex normals, preserving intentional local topology.
        // Visual payloads without authored normals use the observed legacy
        // source convention and are reversed fail-closed. Colliders are not a
        // render surface and retain their exact source index order.
        let mut converted = indices.clone();
        if kind == PayloadKind::Visual
            && published_visual_winding_needs_reversal(&positions, &normals, &converted)
        {
            for triangle in converted.chunks_exact_mut(3) {
                triangle.swap(1, 2);
            }
        }
        let index_accessor =
            append_index_accessor(&mut binary, &mut buffer_views, &mut accessors, &converted)?;
        let mut attributes = JsonMap::new();
        attributes.insert("POSITION".to_string(), json!(position_accessor));
        if let Some(accessor) = normal_accessor {
            attributes.insert("NORMAL".to_string(), json!(accessor));
        }
        if let Some(accessor) = uv_accessor {
            attributes.insert("TEXCOORD_0".to_string(), json!(accessor));
        }
        let mut primitive = JsonMap::new();
        primitive.insert("attributes".to_string(), JsonValue::Object(attributes));
        primitive.insert("indices".to_string(), json!(index_accessor));
        primitive.insert("mode".to_string(), json!(4));
        if kind == PayloadKind::Visual {
            let index = *material_index_by_slot
                .get(submesh_index)
                .or_else(|| material_index_by_slot.last())
                .ok_or_else(|| "internal GLB material slot miss".to_string())?;
            primitive.insert("material".to_string(), json!(index));
        }
        primitive.insert(
            "extras".to_string(),
            json!({ "ffoneSourceSubmesh": submesh_index }),
        );
        primitives.push(JsonValue::Object(primitive));
        total_indices = total_indices
            .checked_add(converted.len())
            .ok_or_else(|| "GLB index count overflow".to_string())?;
    }
    if primitives.is_empty() {
        return Err(format!("{source_instance_id} produced no GLB primitives"));
    }
    pad_four(&mut binary, 0);

    let mut root = JsonMap::new();
    root.insert(
        "asset".to_string(),
        json!({
            "version": "2.0",
            "generator": "FusionForge native_world_static exact exporter",
            "extras": {
                "schema": EXPORT_SCHEMA,
                "sourceInstanceId": source_instance_id,
                "transformPolicy": "native world matrix baked exactly once; node transform identity"
            }
        }),
    );
    root.insert("scene".to_string(), json!(0));
    root.insert("scenes".to_string(), json!([{ "nodes": [0] }]));
    root.insert(
        "nodes".to_string(),
        json!([{
            "name": root_name,
            "mesh": 0,
            "extras": { "sourceInstanceId": source_instance_id }
        }]),
    );
    root.insert(
        "meshes".to_string(),
        json!([{
            "name": root_name,
            "primitives": primitives,
            "extras": { "sourceInstanceId": source_instance_id }
        }]),
    );
    root.insert(
        "buffers".to_string(),
        json!([{ "byteLength": binary.len() }]),
    );
    root.insert("bufferViews".to_string(), json!(buffer_views));
    root.insert("accessors".to_string(), json!(accessors));
    if !gltf_materials.is_empty() {
        root.insert("materials".to_string(), json!(gltf_materials));
    }
    if !images.is_empty() {
        root.insert("images".to_string(), json!(images));
        root.insert("textures".to_string(), json!(textures));
        root.insert("samplers".to_string(), json!(samplers));
    }
    let bytes = encode_glb(JsonValue::Object(root), binary)?;
    Ok(GlbBuild {
        bytes,
        vertex_count: positions.len(),
        index_count: total_indices,
        primitive_count: source_submeshes
            .iter()
            .filter(|value| !value.is_empty())
            .count(),
    })
}

pub(super) fn validate_single_root_glb(bytes: &[u8], root_name: &str, path: &str) -> Result<(), String> {
    let (document, binary_range) = parse_glb(bytes, path)?;
    let nodes = document
        .get("nodes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| format!("{path} has no nodes array"))?;
    let meshes = document
        .get("meshes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| format!("{path} has no meshes array"))?;
    let roots = document
        .pointer("/scenes/0/nodes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| format!("{path} has no scene root array"))?;
    if nodes.len() != 1
        || meshes.len() != 1
        || roots.as_slice() != [json!(0)]
        || nodes[0].get("name").and_then(JsonValue::as_str) != Some(root_name)
        || meshes[0].get("name").and_then(JsonValue::as_str) != Some(root_name)
        || nodes[0].get("mesh").and_then(JsonValue::as_u64) != Some(0)
    {
        return Err(format!(
            "{path} is not the exact single-root/single-mesh GLB {root_name:?}"
        ));
    }
    let buffer_length = document
        .pointer("/buffers/0/byteLength")
        .and_then(JsonValue::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("{path} has invalid buffer byteLength"))?;
    if buffer_length != binary_range.len() {
        return Err(format!(
            "{path} GLB binary length {} differs from declared {buffer_length}",
            binary_range.len()
        ));
    }
    let primitives = meshes[0]
        .get("primitives")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| format!("{path} mesh has no primitives"))?;
    if primitives.is_empty() {
        return Err(format!("{path} mesh has no primitives"));
    }
    for primitive in primitives {
        if primitive.get("mode").and_then(JsonValue::as_u64) != Some(4)
            || primitive.pointer("/attributes/POSITION").is_none()
            || primitive.get("indices").is_none()
        {
            return Err(format!("{path} has a non-triangle/incomplete primitive"));
        }
    }
    Ok(())
}

pub(super) fn parse_glb<'a>(bytes: &'a [u8], path: &str) -> Result<(JsonValue, &'a [u8]), String> {
    if bytes.len() < 28
        || bytes.get(..4) != Some(b"glTF")
        || read_u32(bytes, 4)? != 2
        || read_u32(bytes, 8)? as usize != bytes.len()
        || read_u32(bytes, 16)? != 0x4e4f_534a
    {
        return Err(format!("{path} is not a complete GLB 2.0"));
    }
    let json_len = read_u32(bytes, 12)? as usize;
    let json_start = 20usize;
    let json_end = json_start
        .checked_add(json_len)
        .ok_or_else(|| format!("{path} JSON range overflow"))?;
    if json_end
        .checked_add(8)
        .is_none_or(|minimum| minimum > bytes.len())
    {
        return Err(format!("{path} has a truncated JSON chunk"));
    }
    let document = serde_json::from_slice(&bytes[json_start..json_end])
        .map_err(|err| format!("{path} has invalid GLB JSON: {err}"))?;
    let bin_len = read_u32(bytes, json_end)? as usize;
    if read_u32(bytes, json_end + 4)? != 0x004e_4942 {
        return Err(format!("{path} has no GLB BIN chunk"));
    }
    let bin_start = json_end + 8;
    let bin_end = bin_start
        .checked_add(bin_len)
        .ok_or_else(|| format!("{path} BIN range overflow"))?;
    if bin_end != bytes.len() {
        return Err(format!("{path} has a truncated or trailing BIN chunk"));
    }
    Ok((document, &bytes[bin_start..bin_end]))
}

pub(super) fn required_mesh_pointer(
    env: &UnityEnvironment,
    value: Option<&UnityValue>,
    context: &str,
) -> Result<ObjectKey, String> {
    let pointer = value
        .and_then(UnityValue::as_pointer)
        .ok_or_else(|| format!("{context} is not a Unity PPtr"))?;
    if pointer.is_null() {
        return Err(format!("{context} is null"));
    }
    env.resolve_pointer(pointer)
        .map_err(|err| format!("{context} cannot resolve: {err}"))
}

pub(super) fn ensure_mesh_type(env: &UnityEnvironment, key: ObjectKey) -> Result<(), String> {
    let actual = object_type(env, key)?;
    if actual == "Mesh" {
        Ok(())
    } else {
        Err(format!(
            "{} resolves to {actual}, expected Mesh",
            source_id(env, key)
        ))
    }
}
