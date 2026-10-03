use super::*;

pub(super) const GLB_JSON_CHUNK: u32 = 0x4e4f_534a;

pub(super) const GLB_BIN_CHUNK: u32 = 0x004e_4942;

pub(super) const GLB_TRIANGLES: u64 = 4;

pub(super) const GLTF_UNSIGNED_BYTE: u64 = 5_121;

pub(super) const GLTF_UNSIGNED_SHORT: u64 = 5_123;

pub(super) const GLTF_UNSIGNED_INT: u64 = 5_125;

pub(super) const GLTF_FLOAT: u64 = 5_126;

pub(super) struct ParsedGlb {
    pub(super) document: JsonValue,
    pub(super) binary_start: usize,
    pub(super) binary_len: usize,
}

pub(super) fn analyze_glb(bytes: &[u8], path: &str) -> Result<GeometryStats> {
    let parsed = parse_glb(bytes, path)?;
    let mut stats = GeometryStats::default();
    let meshes = required_array(&parsed.document, "meshes", path)?;
    let mut seen_indices = BTreeSet::new();
    for mesh in meshes {
        for primitive in required_array(mesh, "primitives", path)? {
            let mode = primitive
                .get("mode")
                .and_then(JsonValue::as_u64)
                .unwrap_or(GLB_TRIANGLES);
            if mode != GLB_TRIANGLES {
                return invalid(format!(
                    "{path:?} contains non-triangle primitive mode {mode}"
                ));
            }
            let attributes = primitive
                .get("attributes")
                .and_then(JsonValue::as_object)
                .ok_or_else(|| invalid_error(format!("{path:?} primitive has no attributes")))?;
            stats.tangent_accessors += u64::from(attributes.contains_key("TANGENT"));
            let position_index =
                required_u64(attributes.get("POSITION"), "POSITION", path)? as usize;
            let normal_index = attributes
                .get("NORMAL")
                .and_then(JsonValue::as_u64)
                .map(|value| value as usize);
            let index_index = required_u64(primitive.get("indices"), "indices", path)? as usize;
            if seen_indices.insert(index_index) {
                stats.index_accessors += 1;
            }
            let positions = read_vec3_accessor(bytes, &parsed, position_index, path)?;
            let normals = normal_index
                .map(|index| read_vec3_accessor(bytes, &parsed, index, path))
                .transpose()?;
            let indices = read_index_accessor(bytes, &parsed, index_index, path)?;
            stats.triangles += (indices.len() / 3) as u64;
            if let Some(normals) = normals.filter(|values| values.len() == positions.len()) {
                stats.normal_bearing = true;
                for triangle in indices.chunks_exact(3) {
                    let Some(a) = positions.get(triangle[0] as usize) else {
                        return invalid(format!("{path:?} index exceeds POSITION count"));
                    };
                    let Some(b) = positions.get(triangle[1] as usize) else {
                        return invalid(format!("{path:?} index exceeds POSITION count"));
                    };
                    let Some(c) = positions.get(triangle[2] as usize) else {
                        return invalid(format!("{path:?} index exceeds POSITION count"));
                    };
                    let na = normals[triangle[0] as usize];
                    let nb = normals[triangle[1] as usize];
                    let nc = normals[triangle[2] as usize];
                    let edge_a = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let edge_b = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                    let geometric = [
                        edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1],
                        edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2],
                        edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0],
                    ];
                    let authored = [
                        na[0] + nb[0] + nc[0],
                        na[1] + nb[1] + nc[1],
                        na[2] + nb[2] + nc[2],
                    ];
                    let face_len = dot3(geometric, geometric);
                    let normal_len = dot3(authored, authored);
                    if face_len <= 1.0e-20 || normal_len <= 1.0e-20 {
                        stats.degenerate += 1;
                        continue;
                    }
                    let agreement = dot3(geometric, authored);
                    if agreement > 1.0e-10 {
                        stats.aligned += 1;
                    } else if agreement < -1.0e-10 {
                        stats.opposed += 1;
                    } else {
                        stats.degenerate += 1;
                    }
                }
            }
        }
    }
    Ok(stats)
}

pub(super) fn reverse_glb_indices(bytes: &mut [u8], path: &str) -> Result<()> {
    let parsed = parse_glb(bytes, path)?;
    let meshes = required_array(&parsed.document, "meshes", path)?;
    let mut accessors = BTreeSet::new();
    for mesh in meshes {
        for primitive in required_array(mesh, "primitives", path)? {
            let mode = primitive
                .get("mode")
                .and_then(JsonValue::as_u64)
                .unwrap_or(GLB_TRIANGLES);
            if mode != GLB_TRIANGLES {
                return invalid(format!(
                    "{path:?} contains non-triangle primitive mode {mode}"
                ));
            }
            accessors.insert(required_u64(primitive.get("indices"), "indices", path)? as usize);
        }
    }
    for accessor_index in accessors {
        reverse_index_accessor(bytes, &parsed, accessor_index, path)?;
    }
    Ok(())
}

pub(super) fn parse_glb(bytes: &[u8], path: &str) -> Result<ParsedGlb> {
    if bytes.len() < 28 || &bytes[0..4] != b"glTF" {
        return invalid(format!("{path:?} is not GLB 2.0"));
    }
    if le_u32(bytes, 4, path)? != 2 || le_u32(bytes, 8, path)? as usize != bytes.len() {
        return invalid(format!("{path:?} has an invalid GLB header"));
    }
    let json_len = le_u32(bytes, 12, path)? as usize;
    if le_u32(bytes, 16, path)? != GLB_JSON_CHUNK {
        return invalid(format!("{path:?} has no leading JSON chunk"));
    }
    let json_start = 20_usize;
    let json_end = json_start
        .checked_add(json_len)
        .ok_or_else(|| invalid_error(format!("{path:?} JSON length overflow")))?;
    let binary_header_end = json_end
        .checked_add(8)
        .ok_or_else(|| invalid_error(format!("{path:?} BIN header overflow")))?;
    if binary_header_end > bytes.len() {
        return invalid(format!("{path:?} has a truncated JSON chunk"));
    }
    let binary_len = le_u32(bytes, json_end, path)? as usize;
    if le_u32(bytes, json_end + 4, path)? != GLB_BIN_CHUNK {
        return invalid(format!("{path:?} has no BIN chunk"));
    }
    let binary_start = binary_header_end;
    let binary_end = binary_start
        .checked_add(binary_len)
        .ok_or_else(|| invalid_error(format!("{path:?} BIN length overflow")))?;
    if binary_end != bytes.len() {
        return invalid(format!("{path:?} has unexpected trailing GLB chunks"));
    }
    let document = serde_json::from_slice(&bytes[json_start..json_end]).map_err(|source| {
        PipelineError::Json {
            path: path.to_owned(),
            source,
        }
    })?;
    Ok(ParsedGlb {
        document,
        binary_start,
        binary_len,
    })
}

pub(super) fn is_visual_glb(path: &str, scope: &str) -> bool {
    let expected = if scope == "worldMap" {
        "models/world/maps/"
    } else {
        "models/world/tutorial/"
    };
    path.starts_with(expected)
        && path.ends_with(".glb")
        && path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("v-"))
}
