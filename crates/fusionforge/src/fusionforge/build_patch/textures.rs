use super::*;

pub(super) const TEXTURE_MANIFEST_FORMAT: &str = "fftools.texture-patch.v1";

pub fn patch_texture_pngs(args: &[String]) -> Result<(), String> {
    let usage = "patch-texture-pngs <extracted-dir> <texture-dir> --asset name [--container name] [--allow-resize] [--status-file path]";
    let extracted_dir = required_path(args, 0, usage)?;
    let texture_dir = required_path(args, 1, usage)?;
    let asset_name = option_value(args, "--asset").ok_or_else(|| usage.to_string())?;
    let container = option_value(args, "--container").unwrap_or_default();
    let allow_resize = has_flag(args, "--allow-resize");
    let status_file = option_value(args, "--status-file").map(PathBuf::from);
    let manifest_path = texture_dir.join("manifest.json");
    let manifest = read_json(&manifest_path)?;
    if manifest.get("format").and_then(JsonValue::as_str) != Some(TEXTURE_MANIFEST_FORMAT) {
        return Err(format!(
            "Unsupported texture manifest format in {}",
            manifest_path.display()
        ));
    }
    let asset = Asset::from_path(&extracted_dir.join(&asset_name))?;
    let asset_index = 0usize;
    let mut replacements = BTreeMap::new();
    let mut patched = 0usize;
    let mut missing = 0usize;
    let unchanged = 0usize;
    for entry in manifest
        .get("entries")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        if entry.get("asset").and_then(JsonValue::as_str) != Some(asset_name.as_str()) {
            continue;
        }
        let entry_container = entry
            .get("container")
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        if !container.is_empty() && !entry_container.is_empty() && entry_container != container {
            continue;
        }
        let Some(replacement) = texture_replacement_path(entry, &texture_dir) else {
            missing += 1;
            continue;
        };
        if !replacement.exists() {
            missing += 1;
            continue;
        }
        let path_id = entry_i64(entry, "path_id").or_else(|_| entry_i64(entry, "pathId"))?;
        let Some(info) = asset.objects.get(&path_id) else {
            missing += 1;
            continue;
        };
        let mut value = asset.read_object(asset_index, info)?;
        if !allow_resize {
            let image = image::open(&replacement)
                .map_err(|err| format!("{}: {err}", replacement.display()))?;
            let (width, height) = image.dimensions();
            if value.get("m_Width").and_then(UnityValue::as_i64) != Some(width as i64)
                || value.get("m_Height").and_then(UnityValue::as_i64) != Some(height as i64)
            {
                return Err(format!(
                    "{} size differs; pass --allow-resize",
                    replacement.display()
                ));
            }
        }
        let imported = ImportedTexture::from_png_path(&replacement, None, false)?;
        apply_texture_import(&mut value, imported)?;
        replacements.insert(path_id, value);
        patched += 1;
    }
    if patched > 0 {
        let bytes = asset.rebuild_with_object_values(asset_index, &replacements)?;
        fs::write(extracted_dir.join(&asset_name), bytes)
            .map_err(|err| format!("{}: {err}", extracted_dir.join(&asset_name).display()))?;
    }
    println!(
        "patched {patched} texture(s) in {}",
        extracted_dir.join(&asset_name).display()
    );
    if unchanged > 0 {
        println!("unchanged PNG(s) skipped: {unchanged}");
    }
    if missing > 0 {
        println!("missing manifest target(s): {missing}");
    }
    write_status(
        status_file.as_deref(),
        json!({ "asset": asset_name, "container": container, "patched": patched, "unchanged": unchanged, "missing": missing }),
    )?;
    Ok(())
}

pub(super) fn texture_replacement_path(entry: &JsonValue, texture_dir: &Path) -> Option<PathBuf> {
    let rel = entry
        .get("file")
        .or_else(|| entry.get("png"))
        .or_else(|| entry.get("path"))
        .and_then(JsonValue::as_str)?;
    Some(texture_dir.join(rel))
}

pub(super) fn texture_to_logical_alpha(texture: &UnityValue) -> Result<Vec<u8>, String> {
    let width = texture
        .get("m_Width")
        .and_then(UnityValue::as_i64)
        .unwrap_or(0) as usize;
    let height = texture
        .get("m_Height")
        .and_then(UnityValue::as_i64)
        .unwrap_or(0) as usize;
    if texture.get("m_TextureFormat").and_then(UnityValue::as_i64) != Some(1) {
        return Err("Only Alpha8 font textures can be patched from TTF".to_string());
    }
    let data = texture
        .get("image data")
        .and_then(UnityValue::as_bytes)
        .ok_or_else(|| "Font texture has no image data".to_string())?;
    if data.len() != width * height {
        return Err(format!(
            "Unexpected Alpha8 texture data length {} for {width}x{height}",
            data.len()
        ));
    }
    let mut logical = vec![0u8; width * height];
    for y in 0..height {
        let source = (height - 1 - y) * width;
        let target = y * width;
        logical[target..target + width].copy_from_slice(&data[source..source + width]);
    }
    Ok(logical)
}

pub(super) fn logical_alpha_to_texture_data(logical: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut raw = vec![0u8; width * height];
    for y in 0..height {
        let source = y * width;
        let target = (height - 1 - y) * width;
        raw[target..target + width].copy_from_slice(&logical[source..source + width]);
    }
    raw
}
