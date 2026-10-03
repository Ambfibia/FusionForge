use super::*;

pub(super) const GLB_JSON_CHUNK: u32 = 0x4e4f_534a;

pub(super) const GLB_BIN_CHUNK: u32 = 0x004e_4942;

pub(super) const GLTF_FLOAT: u64 = 5_126;

pub(super) struct ParsedGlb {
    pub(super) document: JsonValue,
    pub(super) binary: Vec<u8>,
}

pub(super) fn resolved_glb_textures(
    document: &JsonValue,
    model_path: &Path,
    asset_root: &Path,
    allowed_resource_set_root: &Path,
) -> Result<Vec<String>> {
    let mut textures = Vec::new();
    let Some(images) = document.get("images").and_then(JsonValue::as_array) else {
        return Ok(textures);
    };
    let parent = model_path
        .parent()
        .ok_or_else(|| invalid_error("map-object GLB has no parent"))?;
    for image in images {
        let Some(uri) = image.get("uri").and_then(JsonValue::as_str) else {
            continue;
        };
        let path = fs::canonicalize(parent.join(Path::new(uri)))
            .map_err(|error| io_at(parent.join(Path::new(uri)), error))?;
        if !path.starts_with(asset_root)
            || !path.starts_with(allowed_resource_set_root)
            || !path.is_file()
        {
            return invalid(format!(
                "map-object GLB texture escaped its resource set: {uri:?}"
            ));
        }
        textures
            .push(slash_path(path.strip_prefix(asset_root).map_err(|_| {
                invalid_error("texture escaped game asset root")
            })?));
    }
    Ok(textures)
}

pub(super) fn validate_direct_mesh_runtime_contract(
    document: &JsonValue,
    expected_root_name: &str,
    path: &str,
) -> Result<()> {
    let nodes = document
        .get("nodes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("{path:?} has no nodes")))?;
    let scenes = document
        .get("scenes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("{path:?} has no scenes")))?;
    let meshes = document
        .get("meshes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("{path:?} has no meshes")))?;
    let exact_scene_root = scenes.len() == 1
        && scenes[0]
            .get("nodes")
            .and_then(JsonValue::as_array)
            .is_some_and(|roots| roots.len() == 1 && roots[0].as_u64() == Some(0));
    let exact_node = nodes.len() == 1
        && nodes[0].get("name").and_then(JsonValue::as_str) == Some(expected_root_name)
        && nodes[0].get("mesh").and_then(JsonValue::as_u64) == Some(0)
        && [
            "children",
            "skin",
            "matrix",
            "translation",
            "rotation",
            "scale",
        ]
        .into_iter()
        .all(|field| nodes[0].get(field).is_none());
    let exact_mesh = meshes.len() == 1
        && meshes[0].get("name").and_then(JsonValue::as_str) == Some(expected_root_name);
    if !exact_scene_root || !exact_node || !exact_mesh {
        return invalid(format!(
            "{path:?} does not satisfy the identity one-node direct-mesh runtime contract"
        ));
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
    let json_start = 20;
    let json_end = json_start + json_len;
    if json_end + 8 > bytes.len() || le_u32(bytes, json_end + 4, path)? != GLB_BIN_CHUNK {
        return invalid(format!("{path:?} has no BIN chunk"));
    }
    let binary_len = le_u32(bytes, json_end, path)? as usize;
    let binary_start = json_end + 8;
    if binary_start + binary_len != bytes.len() {
        return invalid(format!("{path:?} has invalid GLB chunk lengths"));
    }
    let document = serde_json::from_slice(&bytes[json_start..json_end]).map_err(|source| {
        PipelineError::Json {
            path: path.to_owned(),
            source,
        }
    })?;
    Ok(ParsedGlb {
        document,
        binary: bytes[binary_start..].to_vec(),
    })
}

pub(super) fn unbake_glb_geometry(
    parsed: &mut ParsedGlb,
    world_matrix: [[f64; 4]; 4],
    path: &str,
) -> Result<(WorldPrefabBounds, f64)> {
    let world = dmat4(world_matrix);
    let determinant = DMat3::from_mat4(world).determinant();
    if !determinant.is_finite() || determinant.abs() <= 1.0e-12 {
        return invalid(format!(
            "{path:?} has a singular/non-finite baked world matrix"
        ));
    }
    let inverse = world.inverse();
    let normal_inverse = DMat3::from_mat4(world).transpose();
    let meshes = parsed
        .document
        .get("meshes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("{path:?} has no meshes")))?;
    let mut positions = BTreeSet::new();
    let mut normals = BTreeSet::new();
    for mesh in meshes {
        let primitives = mesh
            .get("primitives")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| invalid_error(format!("{path:?} mesh has no primitives")))?;
        for primitive in primitives {
            let attributes = primitive
                .get("attributes")
                .and_then(JsonValue::as_object)
                .ok_or_else(|| invalid_error(format!("{path:?} primitive has no attributes")))?;
            if attributes.contains_key("TANGENT") {
                return invalid(format!(
                    "{path:?} has an unsupported baked TANGENT accessor"
                ));
            }
            positions.insert(required_u64(attributes.get("POSITION"), "POSITION", path)? as usize);
            if let Some(normal) = attributes.get("NORMAL").and_then(JsonValue::as_u64) {
                normals.insert(normal as usize);
            }
        }
    }
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    let mut maximum_position_error = 0.0_f64;
    for accessor in positions {
        let values = read_vec3_accessor(&parsed.binary, &parsed.document, accessor, path)?;
        let mut converted = Vec::with_capacity(values.len());
        for value in values {
            let local = inverse.transform_point3(DVec3::new(
                value[0] as f64,
                value[1] as f64,
                value[2] as f64,
            ));
            let local = dvec3_f32(local, path)?;
            let reconstructed = world.transform_point3(DVec3::new(
                local[0] as f64,
                local[1] as f64,
                local[2] as f64,
            ));
            let expected = DVec3::new(value[0] as f64, value[1] as f64, value[2] as f64);
            maximum_position_error =
                maximum_position_error.max((reconstructed - expected).length());
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(local[axis]);
                maximum[axis] = maximum[axis].max(local[axis]);
            }
            converted.push(local);
        }
        write_vec3_accessor(
            &mut parsed.binary,
            &mut parsed.document,
            accessor,
            &converted,
            path,
        )?;
    }
    for accessor in normals {
        let values = read_vec3_accessor(&parsed.binary, &parsed.document, accessor, path)?;
        let mut converted = Vec::with_capacity(values.len());
        for value in values {
            let local =
                normal_inverse * DVec3::new(value[0] as f64, value[1] as f64, value[2] as f64);
            if !local.is_finite() || local.length_squared() <= 1.0e-20 {
                return invalid(format!("{path:?} produced an invalid local normal"));
            }
            converted.push(dvec3_f32(local.normalize(), path)?);
        }
        write_vec3_accessor(
            &mut parsed.binary,
            &mut parsed.document,
            accessor,
            &converted,
            path,
        )?;
    }
    if minimum[0] == f32::INFINITY {
        return invalid(format!("{path:?} has no POSITION values"));
    }
    if maximum_position_error > 0.02 {
        return invalid(format!(
            "{path:?} local-to-world reconstruction error {maximum_position_error} exceeds 0.02 units"
        ));
    }
    Ok((
        WorldPrefabBounds { minimum, maximum },
        maximum_position_error,
    ))
}

pub(super) fn relocate_glb_textures(
    document: &mut JsonValue,
    source_glb: &Path,
    output_model_relative: &str,
    staging: &Path,
    textures: &mut BTreeMap<String, String>,
) -> Result<()> {
    let Some(images) = document.get_mut("images").and_then(JsonValue::as_array_mut) else {
        return Ok(());
    };
    let source_parent = source_glb
        .parent()
        .ok_or_else(|| invalid_error("source GLB has no parent"))?;
    let output_parent = Path::new(output_model_relative)
        .parent()
        .ok_or_else(|| invalid_error("output GLB has no parent"))?;
    for image in images {
        let object = image
            .as_object_mut()
            .ok_or_else(|| invalid_error("GLB image is not an object"))?;
        let Some(uri) = object
            .get("uri")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        validate_relative(&uri)?;
        let source = safe_join(source_parent, &uri)?;
        let bytes = read_regular_file(&source, "GLB texture")?;
        let hash = hash_bytes(&bytes);
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("png")
            .to_ascii_lowercase();
        let relative = format!("shared/textures/{hash}.{extension}");
        if let Some(existing) = textures.get(&hash) {
            if existing != &relative {
                return invalid("texture hash mapped to different paths");
            }
        } else {
            write_new(&safe_join(staging, &relative)?, &bytes)?;
            textures.insert(hash, relative.clone());
        }
        let relative_uri = relative_path(output_parent, Path::new(&relative))?;
        object.insert("uri".to_owned(), JsonValue::String(relative_uri));
    }
    Ok(())
}

pub(super) fn set_glb_resource_metadata(
    document: &mut JsonValue,
    id: &str,
    name: &str,
    source_path: &str,
    source_hash: &str,
) -> Result<()> {
    let object = document
        .as_object_mut()
        .ok_or_else(|| invalid_error("GLB JSON root is not an object"))?;
    let asset = object
        .entry("asset")
        .or_insert_with(|| serde_json::json!({"version": "2.0"}))
        .as_object_mut()
        .ok_or_else(|| invalid_error("GLB asset is not an object"))?;
    asset.insert(
        "generator".to_owned(),
        JsonValue::String(WORLD_PREFAB_TOOL.to_owned()),
    );
    let extras = asset
        .entry("extras")
        .or_insert_with(|| JsonValue::Object(JsonMap::new()))
        .as_object_mut()
        .ok_or_else(|| invalid_error("GLB asset extras is not an object"))?;
    extras.insert(
        "worldPrefabResourceId".to_owned(),
        JsonValue::String(id.to_owned()),
    );
    extras.insert(
        "sourceBakedPath".to_owned(),
        JsonValue::String(source_path.to_owned()),
    );
    extras.insert(
        "sourceBakedBlake3".to_owned(),
        JsonValue::String(source_hash.to_owned()),
    );
    extras.insert(
        "transformPolicy".to_owned(),
        JsonValue::String("local native mesh; placement transform is external".to_owned()),
    );
    if let Some(nodes) = object.get_mut("nodes").and_then(JsonValue::as_array_mut) {
        if let Some(root) = nodes.first_mut().and_then(JsonValue::as_object_mut) {
            root.insert("name".to_owned(), JsonValue::String(name.to_owned()));
        }
    }
    Ok(())
}
