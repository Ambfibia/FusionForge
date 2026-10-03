use super::*;

pub(super) fn refresh_map_json_references(
    asset_root: &Path,
    map_root: &Path,
    path_map: &BTreeMap<String, String>,
) -> Result<()> {
    let tiles = map_root.join("tiles");
    if !tiles.is_dir() {
        return invalid("map tiles directory is absent");
    }
    let mut json_files = collect_files(&tiles)?
        .into_iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    json_files.sort_by_key(|path| {
        (
            path.file_name().and_then(|value| value.to_str()) == Some("tile.json"),
            slash_path(path),
        )
    });
    // Scene hashes flow into tile.json. Two passes also refresh nested artifact
    // proofs if a readable scene refers to another readable tile-owned file.
    for _ in 0..2 {
        for path in &json_files {
            let mut value = read_json(path, "map tile JSON")?;
            replace_paths(&mut value, path_map);
            refresh_artifact_objects(&mut value, asset_root)?;
            let bytes = pretty_json(&value)?;
            let current = fs::read(path).map_err(|error| io_at(path, error))?;
            if current != bytes {
                write_replace(path, &bytes)?;
            }
        }
    }
    Ok(())
}

pub(super) fn refresh_artifact_objects(value: &mut JsonValue, asset_root: &Path) -> Result<()> {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                refresh_artifact_objects(value, asset_root)?;
            }
        }
        JsonValue::Object(values) => {
            if let Some(path) = values
                .get("path")
                .and_then(JsonValue::as_str)
                .map(str::to_owned)
            {
                let absolute = asset_root.join(Path::new(&path));
                if absolute.is_file() {
                    let bytes = fs::read(&absolute).map_err(|error| io_at(&absolute, error))?;
                    if values.contains_key("bytes") {
                        values.insert("bytes".to_owned(), JsonValue::from(bytes.len() as u64));
                    }
                    if values.contains_key("blake3") {
                        values.insert("blake3".to_owned(), JsonValue::String(hash_bytes(&bytes)));
                    }
                }
            }
            for child in values.values_mut() {
                refresh_artifact_objects(child, asset_root)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn refresh_staged_artifact_objects(
    value: &mut JsonValue,
    asset_root: &Path,
    stage: &Path,
) -> Result<()> {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                refresh_staged_artifact_objects(value, asset_root, stage)?;
            }
        }
        JsonValue::Object(values) => {
            if let Some(path) = values
                .get("path")
                .and_then(JsonValue::as_str)
                .map(str::to_owned)
            {
                let absolute = if path.starts_with("map/objects/") {
                    stage_path_for_rooted(stage, &path)?
                } else {
                    asset_root.join(Path::new(&path))
                };
                if absolute.is_file() {
                    let bytes = fs::read(&absolute).map_err(|error| io_at(&absolute, error))?;
                    if values.contains_key("bytes") {
                        values.insert("bytes".to_owned(), JsonValue::from(bytes.len() as u64));
                    }
                    if values.contains_key("blake3") {
                        values.insert("blake3".to_owned(), JsonValue::String(hash_bytes(&bytes)));
                    }
                }
            }
            for child in values.values_mut() {
                refresh_staged_artifact_objects(child, asset_root, stage)?;
            }
        }
        _ => {}
    }
    Ok(())
}
