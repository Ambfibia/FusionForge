use super::*;

pub(super) fn extract_world_environment(
    map_bundle: &Path,
    build_root: &Path,
    session_dir: &Path,
) -> Result<(), String> {
    let side = world::extract_bundle_to_session(map_bundle, session_dir);
    if side
        .get("errors")
        .and_then(JsonValue::as_array)
        .is_some_and(|errors| !errors.is_empty())
    {
        return Err(format!(
            "map bundle extraction failed: {}",
            side.get("errors").cloned().unwrap_or(JsonValue::Null)
        ));
    }
    let stem = map_bundle
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "map bundle has no UTF-8 stem".to_string())?;
    let suffix = stem
        .strip_prefix("Map_")
        .ok_or_else(|| format!("map bundle stem {stem:?} is not canonical Map_XX_YY"))?;
    let resource = build_root.join(format!("DongResources_{suffix}.resourceFile"));
    let mut extracted = HashSet::from([world::normalize_path(map_bundle)]);
    if resource.is_file() {
        let resource_side = world::extract_bundle_to_session(&resource, session_dir);
        if resource_side
            .get("errors")
            .and_then(JsonValue::as_array)
            .is_some_and(|errors| !errors.is_empty())
        {
            return Err(format!(
                "paired resource extraction failed: {}",
                resource_side
                    .get("errors")
                    .cloned()
                    .unwrap_or(JsonValue::Null)
            ));
        }
        extracted.insert(world::normalize_path(&resource));
    }

    let repo_root = crate::repository_root()
        .parent()
        .ok_or_else(|| "could not resolve FusionFallProject root".to_string())?;
    let mut dependency_sides = Vec::new();
    let mut missing = HashSet::new();
    let mut archive_indexes = HashMap::<PathBuf, HashMap<String, PathBuf>>::new();
    for _ in 0..64 {
        let env = UnityEnvironment::from_dir(session_dir);
        let dependencies = collect_archive_dependencies(&env);
        let changed = world::extract_archives(
            &dependencies,
            Some(map_bundle),
            Some(build_root),
            repo_root,
            session_dir,
            &mut extracted,
            &mut dependency_sides,
            &mut missing,
            &mut archive_indexes,
        );
        if !changed {
            break;
        }
    }
    if !missing.is_empty() {
        let mut names = missing.into_iter().collect::<Vec<_>>();
        names.sort();
        return Err(format!(
            "static-world dependency closure is incomplete: {}",
            names.join(", ")
        ));
    }
    Ok(())
}

pub(super) fn published_visual_winding_needs_reversal(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    indices: &[u32],
) -> bool {
    if normals.len() != positions.len() || normals.is_empty() {
        return true;
    }

    let mut aligned = 0_u64;
    let mut opposed = 0_u64;
    for triangle in indices.chunks_exact(3) {
        let Some(a) = positions.get(triangle[0] as usize) else {
            continue;
        };
        let Some(b) = positions.get(triangle[1] as usize) else {
            continue;
        };
        let Some(c) = positions.get(triangle[2] as usize) else {
            continue;
        };
        let Some(na) = normals.get(triangle[0] as usize) else {
            continue;
        };
        let Some(nb) = normals.get(triangle[1] as usize) else {
            continue;
        };
        let Some(nc) = normals.get(triangle[2] as usize) else {
            continue;
        };

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
        let geometric_length_sq = geometric.iter().map(|value| value * value).sum::<f32>();
        let authored_length_sq = authored.iter().map(|value| value * value).sum::<f32>();
        if geometric_length_sq <= 1.0e-20 || authored_length_sq <= 1.0e-20 {
            continue;
        }
        let agreement =
            geometric[0] * authored[0] + geometric[1] * authored[1] + geometric[2] * authored[2];
        if agreement > 1.0e-10 {
            aligned += 1;
        } else if agreement < -1.0e-10 {
            opposed += 1;
        }
    }

    // A payload with no usable orientation evidence follows the same
    // fail-closed legacy convention as a payload without normals.
    opposed >= aligned
}

pub(super) fn append_vec3_accessor(
    binary: &mut Vec<u8>,
    views: &mut Vec<JsonValue>,
    accessors: &mut Vec<JsonValue>,
    values: &[[f32; 3]],
    bounds: Option<([f32; 3], [f32; 3])>,
) -> Result<usize, String> {
    pad_four(binary, 0);
    let offset = binary.len();
    for value in values {
        for component in value {
            binary.extend_from_slice(&component.to_le_bytes());
        }
    }
    let byte_length = binary
        .len()
        .checked_sub(offset)
        .ok_or_else(|| "vec3 buffer range underflow".to_string())?;
    let view = views.len();
    views.push(json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": byte_length,
        "target": 34962
    }));
    let accessor = accessors.len();
    let mut value = JsonMap::new();
    value.insert("bufferView".to_string(), json!(view));
    value.insert("byteOffset".to_string(), json!(0));
    value.insert("componentType".to_string(), json!(5126));
    value.insert("count".to_string(), json!(values.len()));
    value.insert("type".to_string(), json!("VEC3"));
    if let Some((minimum, maximum)) = bounds {
        value.insert("min".to_string(), json!(minimum));
        value.insert("max".to_string(), json!(maximum));
    }
    accessors.push(JsonValue::Object(value));
    Ok(accessor)
}

pub(super) fn append_vec2_accessor(
    binary: &mut Vec<u8>,
    views: &mut Vec<JsonValue>,
    accessors: &mut Vec<JsonValue>,
    values: &[[f32; 2]],
) -> Result<usize, String> {
    pad_four(binary, 0);
    let offset = binary.len();
    for value in values {
        for component in value {
            binary.extend_from_slice(&component.to_le_bytes());
        }
    }
    let byte_length = binary
        .len()
        .checked_sub(offset)
        .ok_or_else(|| "vec2 buffer range underflow".to_string())?;
    let view = views.len();
    views.push(json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": byte_length,
        "target": 34962
    }));
    let accessor = accessors.len();
    accessors.push(json!({
        "bufferView": view,
        "byteOffset": 0,
        "componentType": 5126,
        "count": values.len(),
        "type": "VEC2"
    }));
    Ok(accessor)
}

pub(super) fn pad_four(bytes: &mut Vec<u8>, fill: u8) {
    while bytes.len() % 4 != 0 {
        bytes.push(fill);
    }
}

pub(super) fn f64_vec3_to_f32(value: (f64, f64, f64), context: &str) -> Result<[f32; 3], String> {
    let converted = [value.0 as f32, value.1 as f32, value.2 as f32];
    if !value.0.is_finite()
        || !value.1.is_finite()
        || !value.2.is_finite()
        || converted.iter().any(|component| !component.is_finite())
    {
        return Err(format!("{context} contains a non-finite/out-of-f32 value"));
    }
    Ok(converted)
}

pub(super) fn color_factor(value: &JsonValue) -> Result<[f64; 4], String> {
    let channel = |name: &str, default: f64| {
        value
            .get(name)
            .and_then(json_lossless_number)
            .unwrap_or(default)
    };
    let result = [
        channel("r", 1.0),
        channel("g", 1.0),
        channel("b", 1.0),
        channel("a", 1.0),
    ];
    if result.iter().any(|component| !component.is_finite()) {
        return Err("material color contains a non-finite channel".to_string());
    }
    Ok(result)
}

pub(super) fn json_lossless_number(value: &JsonValue) -> Option<f64> {
    value.as_f64().or_else(|| {
        value
            .get("$float")
            .and_then(JsonValue::as_f64)
            .or_else(|| value.get("value").and_then(JsonValue::as_f64))
    })
}

pub(super) fn source_json(env: &UnityEnvironment, key: ObjectKey) -> Result<JsonValue, String> {
    let asset = env
        .assets
        .get(key.asset)
        .ok_or_else(|| format!("object key references unknown asset index {}", key.asset))?;
    Ok(json!({
        "asset": asset.name,
        "pathId": key.path_id,
    }))
}

pub(super) fn enrich_native_scene(
    scene: &mut JsonValue,
    scope: &str,
    tile_id: &str,
    tile: [i32; 2],
    models: Vec<JsonValue>,
    visuals: Vec<JsonValue>,
    colliders: Vec<JsonValue>,
    replace_verified_static: bool,
) -> Result<(), String> {
    let object = scene
        .as_object_mut()
        .ok_or_else(|| "native terrain scene root is not an object".to_string())?;
    if object.get("schema").and_then(JsonValue::as_str) != Some(SCENE_SCHEMA)
        || object.get("scope").and_then(JsonValue::as_str) != Some(scope)
        || object.get("name").and_then(JsonValue::as_str) != Some(tile_id)
        || object.get("tile") != Some(&json!(tile))
        || object.get("nativeTerrain").is_none()
    {
        return Err(format!(
            "native terrain scene does not match {scope} tile {tile_id}"
        ));
    }
    for field in ["models", "visuals", "colliders"] {
        let values = object.get(field).and_then(JsonValue::as_array);
        if values.is_none()
            || (!replace_verified_static && values.is_some_and(|values| !values.is_empty()))
        {
            return Err(format!(
                "native terrain scene {tile_id} already contains {field}; refusing stale/mixed overwrite"
            ));
        }
    }
    let native_terrain_before = object
        .get("nativeTerrain")
        .cloned()
        .ok_or_else(|| "native terrain field disappeared".to_string())?;
    object.insert("models".to_string(), JsonValue::Array(models));
    object.insert("visuals".to_string(), JsonValue::Array(visuals));
    object.insert("colliders".to_string(), JsonValue::Array(colliders));
    object.insert(
        "coverage".to_string(),
        json!("native-heightmap+exact-static-scene"),
    );
    if object.get("nativeTerrain") != Some(&native_terrain_before) {
        return Err("static-world enrichment mutated nativeTerrain".to_string());
    }
    Ok(())
}

pub(super) fn verified_existing_static_publication(
    asset_root: &Path,
    layout: &TileLayout,
    source_build: &str,
    source_archive_blake3: &str,
) -> Result<bool, String> {
    let scene_relative = format!("{}/scene.json", layout.source_tile_relative);
    let scene_path = join_relative(asset_root, &scene_relative)?;
    let scene_bytes = fs::read(&scene_path).map_err(|err| {
        format!(
            "could not read native scene {}: {err}",
            scene_path.display()
        )
    })?;
    let scene: JsonValue = serde_json::from_slice(&scene_bytes)
        .map_err(|err| format!("invalid native scene {}: {err}", scene_path.display()))?;
    let populated = ["models", "visuals", "colliders"].iter().any(|field| {
        scene
            .get(field)
            .and_then(JsonValue::as_array)
            .is_some_and(|values| !values.is_empty())
    });
    if !populated {
        return Ok(false);
    }
    if scene.get("coverage").and_then(JsonValue::as_str)
        != Some("native-heightmap+exact-static-scene")
    {
        return Err(format!(
            "native scene {} is populated but is not an exact static-world publication",
            scene_path.display()
        ));
    }

    let catalog_relative = format!("{}/catalog.json", layout.static_relative_root);
    let catalog_path = join_relative(asset_root, &catalog_relative)?;
    let catalog_bytes = fs::read(&catalog_path).map_err(|err| {
        format!(
            "populated native scene {} has no readable static catalog {}: {err}",
            scene_path.display(),
            catalog_path.display()
        )
    })?;
    let catalog: JsonValue = serde_json::from_slice(&catalog_bytes)
        .map_err(|err| format!("invalid static catalog {}: {err}", catalog_path.display()))?;
    let expected_scene_hash = catalog
        .get("scene")
        .and_then(|value| value.get("blake3"))
        .and_then(JsonValue::as_str);
    let identities = [
        (
            "schema",
            catalog.get("schema").and_then(JsonValue::as_str),
            CATALOG_SCHEMA,
        ),
        (
            "sourceBuild",
            catalog.get("sourceBuild").and_then(JsonValue::as_str),
            source_build,
        ),
        (
            "sourceArchiveBlake3",
            catalog
                .get("sourceArchiveBlake3")
                .and_then(JsonValue::as_str),
            source_archive_blake3,
        ),
        (
            "tileId",
            catalog.get("tileId").and_then(JsonValue::as_str),
            layout.tile_id.as_str(),
        ),
        (
            "scene.path",
            catalog
                .get("scene")
                .and_then(|value| value.get("path"))
                .and_then(JsonValue::as_str),
            scene_relative.as_str(),
        ),
    ];
    if let Some((field, observed, expected)) = identities
        .into_iter()
        .find(|(_, observed, expected)| *observed != Some(*expected))
    {
        return Err(format!(
            "static catalog {} has {field}={observed:?}, expected {expected:?}; refusing replacement",
            catalog_path.display()
        ));
    }
    let expected_scene_hash = expected_scene_hash.ok_or_else(|| {
        format!(
            "static catalog {} has no scene.blake3; refusing replacement",
            catalog_path.display()
        )
    })?;
    if !matches_published_bytes(&scene_bytes, expected_scene_hash) {
        verify_static_scene_against_catalog(asset_root, layout, &scene, &catalog)?;
    }
    Ok(true)
}

pub(super) fn matches_published_bytes(bytes: &[u8], expected_blake3: &str) -> bool {
    if blake3_hex(bytes) == expected_blake3 {
        return true;
    }
    crlf_normalized_bytes(bytes)
        .is_some_and(|normalized| blake3_hex(&normalized) == expected_blake3)
}

pub(super) fn crlf_normalized_bytes(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    let mut saw_crlf = false;
    while cursor < bytes.len() {
        if bytes[cursor] == b'\r' {
            if bytes.get(cursor + 1) != Some(&b'\n') {
                return None;
            }
            saw_crlf = true;
            cursor += 1;
        }
        normalized.push(bytes[cursor]);
        cursor += 1;
    }
    saw_crlf.then_some(normalized)
}

pub(super) fn verify_complete_output(
    root: &Path,
    scene_relative: &str,
    catalog_relative: &str,
    manifest_relative: &str,
    expected_models: usize,
    expected_visuals: usize,
    expected_colliders: usize,
) -> Result<(), String> {
    reject_links_recursive(root)?;
    let scene_path = resolve_case_exact(root, scene_relative)?;
    let scene_bytes = fs::read(&scene_path).map_err(|err| {
        format!(
            "could not read staged scene {}: {err}",
            scene_path.display()
        )
    })?;
    let scene: JsonValue = serde_json::from_slice(&scene_bytes)
        .map_err(|err| format!("invalid staged scene {}: {err}", scene_path.display()))?;
    let models = scene
        .get("models")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "staged scene has no models array".to_string())?;
    let visuals = scene
        .get("visuals")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "staged scene has no visuals array".to_string())?;
    let colliders = scene
        .get("colliders")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "staged scene has no colliders array".to_string())?;
    if models.len() != expected_models
        || visuals.len() != expected_visuals
        || colliders.len() != expected_colliders
    {
        return Err(format!(
            "staged scene counts differ: models {}/{expected_models}, visuals {}/{expected_visuals}, colliders {}/{expected_colliders}",
            models.len(),
            visuals.len(),
            colliders.len()
        ));
    }
    let mut model_ids = BTreeSet::new();
    let mut model_by_id = BTreeMap::new();
    for model in models {
        let id = required_json_string(model, "id", "scene model")?;
        let path = required_json_string(model, "path", "scene model")?;
        let hash = required_json_string(model, "blake3", "scene model")?;
        let root_name = required_json_string(model, "rootName", "scene model")?;
        validate_relative_path(path, Some("glb"))?;
        if !path.starts_with("models/") || !model_ids.insert(id.to_string()) {
            return Err(format!("invalid/duplicate scene model {id:?} at {path:?}"));
        }
        let disk = resolve_case_exact(root, path)?;
        let bytes = fs::read(&disk)
            .map_err(|err| format!("could not read staged model {}: {err}", disk.display()))?;
        if blake3_hex(&bytes) != hash {
            return Err(format!("staged model hash mismatch for {path}"));
        }
        let (document, _) = parse_glb(&bytes, path)?;
        validate_single_root_glb(&bytes, root_name, path)?;
        for image in document
            .get("images")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
        {
            let uri = required_json_string(image, "uri", "GLB image")?;
            validate_relative_path(uri, Some("png"))?;
            let model_parent = Path::new(path)
                .parent()
                .ok_or_else(|| format!("model path {path:?} has no parent"))?;
            let combined = slash_path(&model_parent.join(uri));
            resolve_case_exact(root, &combined)?;
        }
        model_by_id.insert(id.to_string(), model);
    }
    let mut runtime_names = BTreeSet::new();
    for visual in visuals {
        let name = required_json_string(visual, "name", "scene visual")?;
        let model = required_json_string(visual, "model", "scene visual")?;
        if !runtime_names.insert(name.to_string()) || !model_by_id.contains_key(model) {
            return Err(format!(
                "visual {name:?} has duplicate name or missing model {model:?}"
            ));
        }
        if visual.get("transform") != Some(&json!(IDENTITY_TRANSFORM)) {
            return Err(format!(
                "visual {name:?} is not identity after world-matrix bake"
            ));
        }
    }
    for collider in colliders {
        let name = required_json_string(collider, "name", "scene collider")?;
        let model = required_json_string(collider, "model", "scene collider")?;
        if !runtime_names.insert(name.to_string()) || !model_by_id.contains_key(model) {
            return Err(format!(
                "collider {name:?} has duplicate name or missing model {model:?}"
            ));
        }
        let vertices = collider
            .get("expectedVertexCount")
            .and_then(JsonValue::as_u64)
            .unwrap_or(0);
        let indices = collider
            .get("expectedIndexCount")
            .and_then(JsonValue::as_u64)
            .unwrap_or(0);
        if vertices == 0
            || indices == 0
            || indices % 3 != 0
            || collider.get("kind").and_then(JsonValue::as_str) != Some("triangleMesh")
            || collider.get("transform") != Some(&json!(IDENTITY_TRANSFORM))
        {
            return Err(format!("collider {name:?} has an invalid runtime contract"));
        }
    }
    if scene
        .pointer("/nativeTerrain/path")
        .and_then(JsonValue::as_str)
        .is_none_or(|path| path.to_ascii_lowercase().ends_with(".glb"))
        || scene
            .to_string()
            .to_ascii_lowercase()
            .contains("terrain.glb")
    {
        return Err("staged scene duplicated native terrain through a GLB route".to_string());
    }
    let catalog_path = resolve_case_exact(root, catalog_relative)?;
    let catalog: JsonValue = serde_json::from_slice(
        &fs::read(&catalog_path)
            .map_err(|err| format!("could not read catalog {}: {err}", catalog_path.display()))?,
    )
    .map_err(|err| format!("invalid static-world catalog: {err}"))?;
    if catalog.get("schema").and_then(JsonValue::as_str) != Some(CATALOG_SCHEMA)
        || catalog
            .get("terrainGlbGenerated")
            .and_then(JsonValue::as_bool)
            != Some(false)
    {
        return Err("static-world catalog schema/terrain contract is invalid".to_string());
    }

    let manifest_path = resolve_case_exact(root, manifest_relative)?;
    let manifest: JsonValue =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|err| {
            format!("could not read manifest {}: {err}", manifest_path.display())
        })?)
        .map_err(|err| format!("invalid static-world manifest: {err}"))?;
    if manifest.get("schema").and_then(JsonValue::as_str) != Some(MANIFEST_SCHEMA) {
        return Err("static-world manifest schema is invalid".to_string());
    }
    let declared = manifest
        .get("files")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "static-world manifest has no files array".to_string())?;
    let mut declared_paths = BTreeSet::new();
    for entry in declared {
        let path = required_json_string(entry, "path", "manifest entry")?;
        let length = entry
            .get("byteLength")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| format!("manifest entry {path:?} has invalid byteLength"))?;
        let hash = required_json_string(entry, "blake3", "manifest entry")?;
        validate_relative_path(path, None)?;
        if path == manifest_relative || !declared_paths.insert(path.to_string()) {
            return Err(format!("manifest contains self/duplicate path {path:?}"));
        }
        let disk = resolve_case_exact(root, path)?;
        let bytes = fs::read(&disk)
            .map_err(|err| format!("could not verify manifest file {}: {err}", disk.display()))?;
        if bytes.len() as u64 != length || blake3_hex(&bytes) != hash {
            return Err(format!("manifest byte/hash mismatch for {path:?}"));
        }
    }
    let mut actual = collect_regular_files(root)?;
    actual.retain(|path| path != manifest_relative);
    let actual = actual.into_iter().collect::<BTreeSet<_>>();
    if actual != declared_paths {
        return Err(format!(
            "manifest coverage mismatch: actual={}, declared={}",
            actual.len(),
            declared_paths.len()
        ));
    }
    Ok(())
}

pub(super) fn output_file_kind(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "glb" => "model",
        "png" => "texture",
        "json" => "data",
        "bin" => "data",
        other if other.starts_with("raw") => "data",
        _ => "data",
    }
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|err| format!("{label} {}: {err}", path.display()))?;
    if metadata.file_type().is_symlink()
        || metadata_is_reparse_point(&metadata)
        || !metadata.is_dir()
    {
        return Err(format!(
            "{label} is not a real directory: {}",
            path.display()
        ));
    }
    fs::canonicalize(path).map_err(|err| format!("{label} {}: {err}", path.display()))
}

pub(super) fn canonical_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|err| format!("{label} {}: {err}", path.display()))?;
    if metadata.file_type().is_symlink()
        || metadata_is_reparse_point(&metadata)
        || !metadata.is_file()
    {
        return Err(format!("{label} is not a real file: {}", path.display()));
    }
    fs::canonicalize(path).map_err(|err| format!("{label} {}: {err}", path.display()))
}

pub(super) fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub(super) fn unique_nonce() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .map_err(|err| format!("system clock is before UNIX epoch: {err}"))
}

pub(super) fn join_relative(root: &Path, relative: &str) -> Result<PathBuf, String> {
    validate_relative_path(relative, None)?;
    Ok(root.join(Path::new(relative)))
}

pub(super) fn replace_regular_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|err| format!("could not inspect {}: {err}", path.display()))?;
    if metadata.file_type().is_symlink()
        || metadata_is_reparse_point(&metadata)
        || !metadata.is_file()
    {
        return Err(format!(
            "refusing to replace non-regular file {}",
            path.display()
        ));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|err| format!("could not replace {}: {err}", path.display()))?;
    file.write_all(bytes)
        .map_err(|err| format!("could not write replacement {}: {err}", path.display()))?;
    file.sync_all()
        .map_err(|err| format!("could not sync replacement {}: {err}", path.display()))
}

pub(super) fn pretty_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn json_string(value: &JsonValue, key: &str) -> String {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .unwrap_or_default()
        .to_string()
}

pub(super) fn safe_name(value: &str, fallback: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
            result.push(character);
            separator = false;
        } else if !separator && !result.is_empty() {
            result.push('_');
            separator = true;
        }
    }
    while result.ends_with('_') {
        result.pop();
    }
    if result.is_empty() {
        fallback.to_string()
    } else {
        result
    }
}

pub(super) fn nonempty_name<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

pub(super) fn source_id(env: &UnityEnvironment, key: ObjectKey) -> String {
    format!("{}#{}", env.asset_name(key.asset), key.path_id)
}
