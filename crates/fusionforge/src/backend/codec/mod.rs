use super::super::*;

pub(in super::super) fn has_cached_payload(cache_dir: &Path) -> bool {
    fs::read_dir(cache_dir)
        .map(|entries| {
            entries.flatten().any(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                entry.path().is_file() && name != "cache-meta.json"
            })
        })
        .unwrap_or(false)
}

pub(in super::super) fn object_payload_bytes(body: &fusionforge::UnityValue) -> Option<Vec<u8>> {
    for key in ["m_Script", "m_Data", "m_RawData", "bytes", "data"] {
        if let Some(bytes) = body.get(key).and_then(unity_value_owned_bytes) {
            return Some(bytes);
        }
    }
    unity_value_owned_bytes(body)
}

pub(in super::super) fn exact_kfm_text_asset_payload(
    object_type: &str,
    body: &fusionforge::UnityValue,
) -> Option<Vec<u8>> {
    if object_type != "TextAsset" {
        return None;
    }
    let bytes = object_payload_bytes(body)?;
    kfm_reference_paths_exact(&bytes)
        .iter()
        .any(|reference| normalized_asset_path(reference).ends_with(".nif"))
        .then_some(bytes)
}

pub(in super::super) fn decode_png_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    let Some(encoded) = data_url.strip_prefix("data:image/png;base64,") else {
        return Err("ICON capture must be a PNG data URL.".to_string());
    };
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| format!("could not decode ICON PNG data: {err}"))
}

pub(in super::super) fn encode_dxt3_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() < expected {
        return Err(format!(
            "texture RGBA data is {} bytes, expected at least {expected}",
            rgba.len()
        ));
    }
    if width == 0 || height == 0 {
        return Err("texture has zero size".to_string());
    }

    let blocks_x = (width + 3) / 4;
    let blocks_y = (height + 3) / 4;
    let mut output = Vec::with_capacity((blocks_x * blocks_y * 16) as usize);
    for block_y in 0..blocks_y {
        for block_x in 0..blocks_x {
            let mut pixels = [[0_u8; 4]; 16];
            for y in 0..4 {
                for x in 0..4 {
                    let src_x = (block_x * 4 + x).min(width - 1);
                    let src_y = (block_y * 4 + y).min(height - 1);
                    let src = ((src_y * width + src_x) * 4) as usize;
                    pixels[(y * 4 + x) as usize].copy_from_slice(&rgba[src..src + 4]);
                }
            }

            let mut alpha_bits = 0_u64;
            for (index, pixel) in pixels.iter().enumerate() {
                alpha_bits |= ((pixel[3] as u64 + 8) / 17).min(15) << (index * 4);
            }
            output.extend_from_slice(&alpha_bits.to_le_bytes());

            let mut min_rgb = [255_u8; 3];
            let mut max_rgb = [0_u8; 3];
            for pixel in &pixels {
                for channel in 0..3 {
                    min_rgb[channel] = min_rgb[channel].min(pixel[channel]);
                    max_rgb[channel] = max_rgb[channel].max(pixel[channel]);
                }
            }
            let mut color0 = rgb_to_565(max_rgb[0], max_rgb[1], max_rgb[2]);
            let mut color1 = rgb_to_565(min_rgb[0], min_rgb[1], min_rgb[2]);
            if color0 < color1 {
                std::mem::swap(&mut color0, &mut color1);
            }
            let c0 = rgb_from_565(color0);
            let c1 = rgb_from_565(color1);
            let palette = [
                c0,
                c1,
                [
                    ((2 * c0[0] as u16 + c1[0] as u16) / 3) as u8,
                    ((2 * c0[1] as u16 + c1[1] as u16) / 3) as u8,
                    ((2 * c0[2] as u16 + c1[2] as u16) / 3) as u8,
                ],
                [
                    ((c0[0] as u16 + 2 * c1[0] as u16) / 3) as u8,
                    ((c0[1] as u16 + 2 * c1[1] as u16) / 3) as u8,
                    ((c0[2] as u16 + 2 * c1[2] as u16) / 3) as u8,
                ],
            ];
            let mut color_indices = 0_u32;
            for (index, pixel) in pixels.iter().enumerate() {
                let rgb = [pixel[0], pixel[1], pixel[2]];
                let best = palette
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, candidate)| color_distance_sq(rgb, **candidate))
                    .map(|(palette_index, _)| palette_index as u32)
                    .unwrap_or(0);
                color_indices |= best << (index * 2);
            }
            output.extend_from_slice(&color0.to_le_bytes());
            output.extend_from_slice(&color1.to_le_bytes());
            output.extend_from_slice(&color_indices.to_le_bytes());
        }
    }
    Ok(output)
}

pub(in super::super) fn encode_dxt3_rgba_mip_chain(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(Vec<u8>, u32), String> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() < expected {
        return Err(format!(
            "texture RGBA data is {} bytes, expected at least {expected}",
            rgba.len()
        ));
    }
    if width == 0 || height == 0 {
        return Err("texture has zero size".to_string());
    }

    let mut full = Vec::new();
    let mut mip_count = 0_u32;
    let mut level_width = width;
    let mut level_height = height;
    let mut level_image = image::RgbaImage::from_raw(width, height, rgba[..expected].to_vec())
        .ok_or_else(|| "texture RGBA data could not be converted into an image".to_string())?;

    loop {
        full.extend_from_slice(&encode_dxt3_rgba(
            level_width,
            level_height,
            level_image.as_raw(),
        )?);
        mip_count += 1;
        if level_width == 1 && level_height == 1 {
            break;
        }
        level_width = (level_width / 2).max(1);
        level_height = (level_height / 2).max(1);
        level_image = image::imageops::resize(
            &level_image,
            level_width,
            level_height,
            image::imageops::FilterType::Triangle,
        );
    }

    Ok((full, mip_count))
}

pub(in super::super) fn npc_payload_fingerprint(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}
