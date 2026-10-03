use super::*;

pub(super) fn dump_terrain(args: &[String]) -> Result<(), String> {
    let path = required_path(args, 0, "dump-terrain <asset-or-bundle> <out.png>")?;
    let out = required_path(args, 1, "dump-terrain <asset-or-bundle> <out.png>")?;
    let loaded = load_input(&path)?;
    let Some((_asset_index, _asset, _info, body)) = find_first_type(&loaded.env, "TerrainData")?
    else {
        return Err("TerrainData not found".to_string());
    };
    let heightmap = body
        .get("m_Heightmap")
        .ok_or_else(|| "TerrainData has no m_Heightmap".to_string())?;
    let width = heightmap
        .get("m_Width")
        .and_then(UnityValue::as_i64)
        .ok_or_else(|| "TerrainData has no m_Width".to_string())? as u32;
    let height = heightmap
        .get("m_Height")
        .and_then(UnityValue::as_i64)
        .ok_or_else(|| "TerrainData has no m_Height".to_string())? as u32;
    let heights = value_array(heightmap.get("m_Heights"));
    let mut image = RgbaImage::new(width, height);
    for (index, height_value) in heights.iter().enumerate() {
        let value = height_value.as_f64().unwrap_or_default();
        let pixel = ((value * 255.0) / 20000.0).round().clamp(0.0, 255.0) as u8;
        let x = index as u32 % width;
        let y = index as u32 / width;
        if x < width && y < height {
            image.put_pixel(x, y, image::Rgba([pixel, pixel, pixel, 255]));
        }
    }
    write_png(&out, &image)?;
    println!("Wrote terrain PNG to {}", out.display());
    Ok(())
}

pub(super) fn export_native_terrain(args: &[String]) -> Result<(), String> {
    let mut selection = super::super::native_terrain::TerrainSelection::First;
    let mut positional = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--first" => selection = super::super::native_terrain::TerrainSelection::First,
            "--all" => selection = super::super::native_terrain::TerrainSelection::All,
            value if value.starts_with("--") => {
                return Err(format!("unknown export-native-terrain option: {value}"));
            }
            _ => positional.push(arg),
        }
    }
    if positional.len() != 2 {
        return Err(
            "export-native-terrain <asset-or-bundle> <fresh-out-dir> [--first|--all]".to_string(),
        );
    }
    let input = PathBuf::from(positional[0]);
    let output = PathBuf::from(positional[1]);
    let loaded = load_input_with_sibling_dependencies(&input)?;
    let count = super::super::native_terrain::export_native_terrain(
        &loaded.env,
        &loaded.source_asset_names,
        &input,
        &output,
        selection,
    )?;
    println!(
        "Exported {count} native TerrainData object(s) to {}",
        output.display()
    );
    Ok(())
}

pub(super) fn replace_terrain(args: &[String]) -> Result<(), String> {
    let img = required_path(
        args,
        0,
        "replace-terrain <input.png> <offset> <output-asset>",
    )?;
    let offset = args
        .get(1)
        .ok_or_else(|| "replace-terrain requires an offset".to_string())
        .and_then(|value| parse_i64(value))? as u64
        + 4;
    let output = required_path(
        args,
        2,
        "replace-terrain <input.png> <offset> <output-asset>",
    )?;
    let image = image::open(&img).map_err(|err| err.to_string())?.to_rgb8();
    let mut bytes = Vec::with_capacity((image.width() * image.height() * 2) as usize);
    for pixel in image.pixels() {
        let height = ((pixel[0] as u32 * 20000) / 255) as u16;
        bytes.extend_from_slice(&height.to_le_bytes());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .open(&output)
        .map_err(|err| err.to_string())?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|err| err.to_string())?;
    file.write_all(&bytes).map_err(|err| err.to_string())?;
    println!("Patched {} bytes at {:#x}", bytes.len(), offset);
    Ok(())
}
