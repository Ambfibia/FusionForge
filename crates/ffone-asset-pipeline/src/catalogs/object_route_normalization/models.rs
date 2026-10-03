use super::*;

pub(super) fn rewrite_glb_uris(
    source: &Path,
    destination: &str,
    routes: &BTreeMap<String, String>,
    asset_root: &Path,
) -> Result<Vec<u8>> {
    let bytes = read_file(source, "map-object GLB")?;
    if bytes.len() < 20 || &bytes[..4] != b"glTF" || read_u32(&bytes, 4)? != 2 {
        return invalid(format!("map model is not GLB 2.0: {}", source.display()));
    }
    if read_u32(&bytes, 8)? as usize != bytes.len() {
        return invalid(format!("GLB length mismatch: {}", source.display()));
    }
    let mut cursor = 12usize;
    let mut chunks = Vec::<(u32, Vec<u8>)>::new();
    while cursor < bytes.len() {
        let length = read_u32(&bytes, cursor)? as usize;
        let kind = read_u32(&bytes, cursor + 4)?;
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .ok_or_else(|| invalid_error("GLB chunk length overflow"))?;
        if end > bytes.len() {
            return invalid(format!("truncated GLB chunk: {}", source.display()));
        }
        chunks.push((kind, bytes[start..end].to_vec()));
        cursor = end;
    }
    let Some((_, json_bytes)) = chunks.first_mut().filter(|(kind, _)| *kind == 0x4e4f_534a) else {
        return invalid(format!(
            "GLB has no leading JSON chunk: {}",
            source.display()
        ));
    };
    while json_bytes
        .last()
        .is_some_and(|byte| *byte == b' ' || *byte == 0)
    {
        json_bytes.pop();
    }
    let mut document: JsonValue =
        serde_json::from_slice(json_bytes).map_err(|source_error| PipelineError::Json {
            path: source.display().to_string(),
            source: source_error,
        })?;
    let old_parent = source
        .parent()
        .ok_or_else(|| invalid_error("GLB source has no parent"))?;
    let new_parent = Path::new(destination)
        .parent()
        .ok_or_else(|| invalid_error("GLB destination has no parent"))?;
    rewrite_glb_json_paths(&mut document, old_parent, new_parent, routes, asset_root)?;
    *json_bytes = serde_json::to_vec(&document).map_err(generated_json_error)?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12usize
        + chunks
            .iter()
            .map(|(_, chunk)| 8usize + chunk.len())
            .sum::<usize>();
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(b"glTF");
    output.extend_from_slice(&2u32.to_le_bytes());
    output.extend_from_slice(&(total as u32).to_le_bytes());
    for (kind, chunk) in chunks {
        output.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        output.extend_from_slice(&kind.to_le_bytes());
        output.extend_from_slice(&chunk);
    }
    Ok(output)
}

pub(super) fn rewrite_glb_json_paths(
    value: &mut JsonValue,
    old_parent: &Path,
    new_parent: &Path,
    routes: &BTreeMap<String, String>,
    asset_root: &Path,
) -> Result<()> {
    match value {
        JsonValue::String(text) => {
            if text.starts_with("data:") || !text.to_ascii_lowercase().contains(".png") {
                return Ok(());
            }
            let candidate = old_parent.join(Path::new(text.as_str()));
            let Ok(absolute) = fs::canonicalize(&candidate) else {
                return Ok(());
            };
            let old_route = asset_relative(asset_root, &absolute)?;
            if let Some(new_route) = routes.get(&old_route) {
                *text = relative_path(new_parent, Path::new(new_route))?;
            }
        }
        JsonValue::Array(values) => {
            for value in values {
                rewrite_glb_json_paths(value, old_parent, new_parent, routes, asset_root)?;
            }
        }
        JsonValue::Object(values) => {
            for value in values.values_mut() {
                rewrite_glb_json_paths(value, old_parent, new_parent, routes, asset_root)?;
            }
        }
        _ => {}
    }
    Ok(())
}
