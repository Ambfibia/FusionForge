use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};

const HASH_SUFFIX_LEN: usize = 18;

pub(super) fn run(command_args: &[String]) -> Result<(), String> {
    let mut args = command_args.iter().map(std::ffi::OsString::from);
    let project_root = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let apply = args.any(|arg| arg == "--apply");
    let project_root = fs::canonicalize(&project_root)
        .map_err(|error| format!("cannot resolve project root: {error}"))?;
    let asset_root = project_root.join("assets/game");
    let map_root = asset_root.join("map");
    let catalog_path = map_root.join("catalog.json");
    let catalog_bytes = fs::read(&catalog_path)
        .map_err(|error| format!("cannot read {}: {error}", catalog_path.display()))?;
    let mut catalog: Value = serde_json::from_slice(&catalog_bytes)
        .map_err(|error| format!("cannot parse {}: {error}", catalog_path.display()))?;
    if catalog.get("schema").and_then(Value::as_str) != Some("ffone.map-catalog.v1") {
        return Err("map catalog has an unsupported schema".to_owned());
    }

    let object_routes = object_route_plan(&catalog, &asset_root)?;
    let terrain_routes = terrain_route_plan(&map_root)?;
    let mut routes = object_routes.clone();
    for (source, destination) in &terrain_routes {
        if routes.insert(source.clone(), destination.clone()).is_some() {
            return Err(format!("duplicate normalization source {source:?}"));
        }
    }
    let destinations = routes.values().cloned().collect::<BTreeSet<_>>();
    if destinations.len() != routes.len() {
        return Err("readable map routes are not unique".to_owned());
    }

    if apply {
        move_packages(&asset_root, &object_routes, "objects")?;
        move_packages(&asset_root, &terrain_routes, "terrain")?;
        normalize_tiles(&map_root, &routes, &mut catalog)?;
        replace_strings(&mut catalog, &routes);
        register_shared_files(&map_root, &mut catalog)?;
        let catalog_bytes = pretty(&catalog)?;
        write_atomic(&catalog_path, &catalog_bytes)?;
    }

    let final_catalog_bytes = fs::read(&catalog_path)
        .map_err(|error| format!("cannot read final map catalog: {error}"))?;
    let report = json!({
        "schema": "ffone.map-readable-routes.v1",
        "mode": if apply { "apply" } else { "plan" },
        "sourceAlias": "primary",
        "sourceBuild": "retrobution-20260613",
        "policy": "content identity remains in object.json/catalog metadata; unique packages use plain names and genuine same-name alternatives use readable variant_NNNN directories",
        "counts": {
            "objectPackages": object_routes.len(),
            "terrainLayerPackages": terrain_routes.len(),
            "routes": routes.len()
        },
        "catalogBlake3": blake3::hash(&final_catalog_bytes).to_hex().to_string(),
        "routes": routes.iter().map(|(source, destination)| json!({
            "source": source,
            "destination": destination
        })).collect::<Vec<_>>()
    });
    let report_path = project_root.join(format!(
        "target/ffone-migration/map-organization/readable-routes-{}.json",
        if apply { "apply" } else { "plan" }
    ));
    write_atomic(&report_path, &pretty(&report)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "report": slash_path(&report_path.strip_prefix(&project_root).unwrap_or(&report_path)),
            "objectPackages": object_routes.len(),
            "terrainLayerPackages": terrain_routes.len(),
            "catalogBlake3": blake3::hash(&final_catalog_bytes).to_hex().to_string()
        }))
        .map_err(|error| error.to_string())?
    );
    Ok(())
}

fn object_route_plan(
    catalog: &Value,
    asset_root: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let prefabs = catalog
        .get("objects")
        .and_then(Value::as_array)
        .ok_or_else(|| "map catalog has no objects array".to_owned())?;
    let mut groups = BTreeMap::<String, Vec<(String, u64)>>::new();
    let mut result = BTreeMap::new();
    for prefab in prefabs {
        let definition = prefab
            .pointer("/definition/path")
            .and_then(Value::as_str)
            .ok_or_else(|| "map prefab has no definition path".to_owned())?;
        let source = definition
            .strip_suffix("/object.json")
            .ok_or_else(|| format!("invalid map object definition route {definition:?}"))?;
        let occurrences = prefab
            .get("occurrenceCount")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if !join_asset(asset_root, source)?.is_dir() {
            return Err(format!("map object package is absent: {source}"));
        }
        if let Some(base) = strip_content_suffix(source) {
            groups
                .entry(base.to_owned())
                .or_default()
                .push((source.to_owned(), occurrences));
        } else if let Some((base, variant)) = source.rsplit_once("/variant_")
            && variant.len() == 4
            && variant.bytes().all(|byte| byte.is_ascii_digit())
        {
            result.insert(source.to_owned(), format!("{base}_variant_{variant}"));
        }
    }
    for (base, mut variants) in groups {
        variants.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for (index, (source, _)) in variants.into_iter().enumerate() {
            let destination = if index == 0 {
                base.clone()
            } else {
                format!("{base}_variant_{:04}", index + 1)
            };
            result.insert(source, destination);
        }
    }
    Ok(result)
}

fn terrain_route_plan(map_root: &Path) -> Result<BTreeMap<String, String>, String> {
    let layers_root = map_root.join("shared/terrain/layers");
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(&layers_root)
        .map_err(|error| format!("cannot enumerate terrain layers: {error}"))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            continue;
        }
        let package_name = entry.file_name().to_string_lossy().to_string();
        if has_content_suffix(&package_name) {
            continue;
        }
        for child in fs::read_dir(entry.path())
            .map_err(|error| format!("cannot inspect terrain layer variants: {error}"))?
        {
            let child = child.map_err(|error| error.to_string())?;
            if !child
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                continue;
            }
            let variant = child.file_name().to_string_lossy().to_string();
            if let Some(number) = variant.strip_prefix("variant_")
                && number.len() == 2
                && number.bytes().all(|byte| byte.is_ascii_digit())
            {
                result.insert(
                    format!("map/shared/terrain/layers/{package_name}/{variant}"),
                    format!("map/shared/terrain/layers/{package_name}_variant_{number}"),
                );
            }
        }
    }
    let mut uses = BTreeMap::<String, u64>::new();
    for terrain in collect_files(&map_root.join("tiles"), "terrain.json")? {
        let text = fs::read_to_string(&terrain)
            .map_err(|error| format!("cannot read {}: {error}", terrain.display()))?;
        for entry in fs::read_dir(&layers_root)
            .map_err(|error| format!("cannot enumerate terrain layers: {error}"))?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            if !entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if has_content_suffix(&name) {
                let route = format!("map/shared/terrain/layers/{name}");
                let count = text.match_indices(&route).count() as u64;
                *uses.entry(route).or_default() += count;
            }
        }
    }
    let mut groups = BTreeMap::<String, Vec<(String, u64)>>::new();
    for entry in fs::read_dir(&layers_root)
        .map_err(|error| format!("cannot enumerate terrain layers: {error}"))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !has_content_suffix(&name) {
            continue;
        }
        let source = format!("map/shared/terrain/layers/{name}");
        let base = strip_content_suffix(&source)
            .ok_or_else(|| format!("invalid terrain package {source:?}"))?;
        groups
            .entry(base.to_owned())
            .or_default()
            .push((source.clone(), *uses.get(&source).unwrap_or(&0)));
    }
    for (base, mut variants) in groups {
        variants.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for (index, (source, _)) in variants.into_iter().enumerate() {
            let destination = if index == 0 {
                base.clone()
            } else {
                format!("{base}_variant_{:02}", index + 1)
            };
            result.insert(source, destination);
        }
    }
    Ok(result)
}

fn move_packages(
    asset_root: &Path,
    routes: &BTreeMap<String, String>,
    label: &str,
) -> Result<(), String> {
    if routes.is_empty() {
        return Ok(());
    }
    let stage = asset_root.join(format!(".map-route-normalization-{label}"));
    if stage.exists() {
        return Err(format!(
            "normalization stage already exists: {}",
            stage.display()
        ));
    }
    fs::create_dir(&stage).map_err(|error| format!("cannot create stage: {error}"))?;
    let mut staged = Vec::new();
    for (index, (source, destination)) in routes.iter().enumerate() {
        let source_path = join_asset(asset_root, source)?;
        let staged_path = stage.join(format!("{index:05}"));
        fs::rename(&source_path, &staged_path)
            .map_err(|error| format!("cannot stage {}: {error}", source_path.display()))?;
        staged.push((staged_path, destination.clone()));
    }
    staged.sort_by(|left, right| {
        left.1
            .matches('/')
            .count()
            .cmp(&right.1.matches('/').count())
            .then_with(|| left.1.cmp(&right.1))
    });
    for (staged_path, destination) in staged {
        let destination_path = join_asset(asset_root, &destination)?;
        if destination_path.exists() {
            return Err(format!(
                "readable map destination already exists: {}",
                destination_path.display()
            ));
        }
        fs::create_dir_all(
            destination_path
                .parent()
                .ok_or_else(|| "map destination has no parent".to_owned())?,
        )
        .map_err(|error| format!("cannot create readable route parent: {error}"))?;
        fs::rename(&staged_path, &destination_path).map_err(|error| {
            format!(
                "cannot publish readable route {}: {error}",
                destination_path.display()
            )
        })?;
    }
    fs::remove_dir(&stage).map_err(|error| format!("cannot remove stage: {error}"))?;
    Ok(())
}

fn normalize_tiles(
    map_root: &Path,
    routes: &BTreeMap<String, String>,
    catalog: &mut Value,
) -> Result<(), String> {
    let placement_sets = catalog
        .get_mut("tiles")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "map catalog has no tiles array".to_owned())?;
    for placement_set in placement_sets {
        let tile_id = placement_set
            .get("tileId")
            .and_then(Value::as_str)
            .ok_or_else(|| "placement set has no tileId".to_owned())?
            .to_owned();
        let tile_root = map_root.join("tiles").join(&tile_id);
        let terrain_path = tile_root.join("terrain/terrain.json");
        let scene_path = tile_root.join("scene.json");
        let manifest_path = tile_root.join("tile.json");

        let mut terrain = read_json(&terrain_path)?;
        replace_strings(&mut terrain, routes);
        let terrain_bytes = pretty(&terrain)?;
        write_atomic(&terrain_path, &terrain_bytes)?;
        let terrain_hash = blake3::hash(&terrain_bytes).to_hex().to_string();

        let mut scene = read_json(&scene_path)?;
        replace_strings(&mut scene, routes);
        if let Some(native_terrain) = scene
            .get_mut("nativeTerrain")
            .and_then(Value::as_object_mut)
        {
            native_terrain.insert("blake3".to_owned(), Value::String(terrain_hash.clone()));
        }
        let scene_bytes = pretty(&scene)?;
        write_atomic(&scene_path, &scene_bytes)?;
        let scene_hash = blake3::hash(&scene_bytes).to_hex().to_string();

        let mut manifest = read_json(&manifest_path)?;
        replace_strings(&mut manifest, routes);
        update_artifact(
            manifest
                .get_mut("terrain")
                .ok_or_else(|| "tile manifest has no terrain artifact".to_owned())?,
            &terrain_bytes,
            &terrain_hash,
        )?;
        update_artifact(
            manifest
                .get_mut("scene")
                .ok_or_else(|| "tile manifest has no scene artifact".to_owned())?,
            &scene_bytes,
            &scene_hash,
        )?;
        if let Some(files) = manifest.get_mut("files").and_then(Value::as_array_mut) {
            for artifact in files {
                match artifact.get("path").and_then(Value::as_str) {
                    Some(path) if path.ends_with("/terrain/terrain.json") => {
                        update_artifact(artifact, &terrain_bytes, &terrain_hash)?
                    }
                    Some(path) if path.ends_with("/scene.json") => {
                        update_artifact(artifact, &scene_bytes, &scene_hash)?
                    }
                    _ => {}
                }
            }
        }
        let manifest_bytes = pretty(&manifest)?;
        write_atomic(&manifest_path, &manifest_bytes)?;
        let manifest_hash = blake3::hash(&manifest_bytes).to_hex().to_string();
        update_artifact(
            placement_set
                .get_mut("manifest")
                .ok_or_else(|| "placement set has no manifest artifact".to_owned())?,
            &manifest_bytes,
            &manifest_hash,
        )?;
    }
    Ok(())
}

fn register_shared_files(map_root: &Path, catalog: &mut Value) -> Result<(), String> {
    let shared_root = map_root.join("shared");
    let mut artifacts = BTreeMap::<String, Value>::new();
    let existing = catalog
        .get_mut("sharedFiles")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "map catalog has no sharedFiles array".to_owned())?;
    for artifact in existing.drain(..) {
        let route = artifact
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "shared map artifact has no path".to_owned())?
            .to_owned();
        artifacts.insert(route, artifact);
    }
    for file in collect_all_files(&shared_root)? {
        let relative = file
            .strip_prefix(map_root)
            .map_err(|_| format!("shared file escaped map root: {}", file.display()))?;
        let route = format!("map/{}", slash_path(relative));
        let bytes = fs::read(&file)
            .map_err(|error| format!("cannot read shared map file {}: {error}", file.display()))?;
        let artifact = json!({
            "path": route,
            "bytes": bytes.len() as u64,
            "blake3": blake3::hash(&bytes).to_hex().to_string()
        });
        artifacts.insert(
            artifact["path"]
                .as_str()
                .expect("artifact path is a string")
                .to_owned(),
            artifact,
        );
    }
    existing.extend(artifacts.into_values());
    Ok(())
}

fn update_artifact(artifact: &mut Value, bytes: &[u8], hash: &str) -> Result<(), String> {
    let object = artifact
        .as_object_mut()
        .ok_or_else(|| "map artifact is not an object".to_owned())?;
    object.insert("bytes".to_owned(), json!(bytes.len() as u64));
    object.insert("blake3".to_owned(), Value::String(hash.to_owned()));
    Ok(())
}

fn replace_strings(value: &mut Value, routes: &BTreeMap<String, String>) {
    match value {
        Value::String(text) => {
            if let Some((source, destination)) = routes
                .iter()
                .find(|(source, _)| text.starts_with(source.as_str()))
            {
                *text = format!("{destination}{}", &text[source.len()..]);
            }
        }
        Value::Array(values) => {
            for value in values {
                replace_strings(value, routes);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_strings(value, routes);
            }
        }
        _ => {}
    }
}

fn strip_content_suffix(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    if bytes.len() < HASH_SUFFIX_LEN {
        return None;
    }
    let suffix = &value[value.len() - HASH_SUFFIX_LEN..];
    if suffix.starts_with("--") && suffix[2..].bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(&value[..value.len() - HASH_SUFFIX_LEN])
    } else {
        None
    }
}

fn has_content_suffix(value: &str) -> bool {
    strip_content_suffix(value).is_some()
}

fn join_asset(asset_root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.starts_with('/') || relative.contains("..") || relative.contains('\\') {
        return Err(format!("unsafe runtime route {relative:?}"));
    }
    Ok(relative
        .split('/')
        .fold(asset_root.to_path_buf(), |path, part| path.join(part)))
}

fn collect_files(root: &Path, file_name: &str) -> Result<Vec<PathBuf>, String> {
    let mut result = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() && entry.file_name() == file_name {
                result.push(entry.path());
            }
        }
    }
    result.sort();
    Ok(result)
}

fn collect_all_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut result = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                result.push(entry.path());
            }
        }
    }
    result.sort();
    Ok(result)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn pretty(value: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let next = path.with_extension(format!(
        "{}.next",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("tmp")
    ));
    fs::write(&next, bytes).map_err(|error| format!("cannot write {}: {error}", next.display()))?;
    fs::rename(&next, path).map_err(|error| format!("cannot publish {}: {error}", path.display()))
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
