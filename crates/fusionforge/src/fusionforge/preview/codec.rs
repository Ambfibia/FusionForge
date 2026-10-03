use super::*;

pub fn decode_texture(env: &UnityEnvironment, texture: &UnityValue) -> Option<DecodedTexture> {
    let mip = decode_texture_mips(env, texture)?.into_iter().next()?;
    Some(DecodedTexture {
        width: mip.width,
        height: mip.height,
        rgba: mip.rgba,
    })
}

pub fn decode_texture_mips(
    env: &UnityEnvironment,
    texture: &UnityValue,
) -> Option<Vec<DecodedTextureMip>> {
    let width = texture.get("m_Width")?.as_i64()? as u32;
    let height = texture.get("m_Height")?.as_i64()? as u32;
    let format = texture.get("m_TextureFormat")?.as_i64()? as i32;
    if width == 0 || height == 0 {
        return None;
    }
    let data = texture_image_data(env, texture)?;
    let has_mips = texture
        .get("m_MipMap")
        .and_then(UnityValue::as_i64)
        .is_some_and(|value| value != 0);
    let mip_count = if has_mips {
        u32::BITS - width.max(height).leading_zeros()
    } else {
        1
    };
    let mut offset = 0_usize;
    let mut mips = Vec::with_capacity(mip_count as usize);
    for level in 0..mip_count {
        let mip_width = (width >> level).max(1);
        let mip_height = (height >> level).max(1);
        let byte_len = texture_mip_encoded_len(format, mip_width, mip_height)?;
        let encoded = data.get(offset..offset.checked_add(byte_len)?)?.to_vec();
        offset += byte_len;
        let rgba = decode_texture_level(format, mip_width, mip_height, &encoded)?;
        mips.push(DecodedTextureMip {
            level,
            width: mip_width,
            height: mip_height,
            rgba,
            source_encoded: encoded,
        });
    }
    Some(mips)
}

/// Terrain publication uses this strict variant because its mip sidecars claim
/// an exact, complete serialized Texture2D payload. The general preview path
/// intentionally remains tolerant of legacy files with trailing bytes.
pub fn decode_texture_mips_exact(
    env: &UnityEnvironment,
    texture: &UnityValue,
) -> Result<Vec<DecodedTextureMip>, String> {
    let mips = decode_texture_mips(env, texture)
        .ok_or_else(|| "could not decode serialized Texture2D mip chain".to_string())?;
    let consumed = mips.iter().try_fold(0_usize, |total, mip| {
        total
            .checked_add(mip.source_encoded.len())
            .ok_or_else(|| "Texture2D mip-chain byte length overflows usize".to_string())
    })?;
    let payload = texture_image_data(env, texture)
        .ok_or_else(|| "Texture2D has no readable image payload".to_string())?;
    if consumed != payload.len() {
        return Err(format!(
            "Texture2D mip chain consumes {consumed} bytes but serialized image payload contains {} bytes",
            payload.len()
        ));
    }
    let complete_image_size =
        match texture.get("m_CompleteImageSize") {
            Some(UnityValue::Int(value)) if *value >= 0 => usize::try_from(*value)
                .map_err(|_| format!("Texture2D m_CompleteImageSize {value} does not fit usize"))?,
            Some(UnityValue::UInt(value)) => usize::try_from(*value)
                .map_err(|_| format!("Texture2D m_CompleteImageSize {value} does not fit usize"))?,
            Some(_) => return Err(
                "Texture2D m_CompleteImageSize is present but not an exact non-negative integer"
                    .to_string(),
            ),
            None => return Err("Texture2D m_CompleteImageSize is missing".to_string()),
        };
    if complete_image_size != payload.len() {
        return Err(format!(
            "Texture2D m_CompleteImageSize is {complete_image_size} but serialized image payload contains {} bytes",
            payload.len()
        ));
    }
    Ok(mips)
}

pub(super) fn decode_texture_level(format: i32, width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    match format {
        1 => decode_alpha8(width, height, &data),
        3 => decode_rgb24(width, height, &data),
        4 => decode_rgba32(width, height, &data),
        5 => decode_argb32(width, height, &data),
        7 => decode_rgb565(width, height, &data),
        10 | 28 => decode_dxt1(width, height, &data),
        11 => decode_dxt3(width, height, &data),
        12 | 29 => decode_dxt5(width, height, &data),
        2 => decode_argb4444(width, height, &data),
        13 => decode_rgba4444(width, height, &data),
        14 => decode_bgra32(width, height, &data),
        _ => None,
    }
}

pub(super) fn decode_alpha8(width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let pixels = (width * height) as usize;
    if data.len() < pixels {
        return None;
    }
    let mut rgba = Vec::with_capacity(pixels * 4);
    for alpha in data.iter().take(pixels) {
        rgba.extend_from_slice(&[255, 255, 255, *alpha]);
    }
    Some(rgba)
}

pub(super) fn decode_rgb24(width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let pixels = (width * height) as usize;
    if data.len() < pixels * 3 {
        return None;
    }
    let mut rgba = Vec::with_capacity(pixels * 4);
    for chunk in data.chunks_exact(3).take(pixels) {
        rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
    }
    Some(rgba)
}

pub(super) fn decode_rgba32(width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let bytes = (width * height * 4) as usize;
    data.get(..bytes).map(Vec::from)
}

pub(super) fn decode_argb32(width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let pixels = (width * height) as usize;
    if data.len() < pixels * 4 {
        return None;
    }
    let mut rgba = Vec::with_capacity(pixels * 4);
    for chunk in data.chunks_exact(4).take(pixels) {
        rgba.extend_from_slice(&[chunk[1], chunk[2], chunk[3], chunk[0]]);
    }
    Some(rgba)
}

pub(super) fn decode_bgra32(width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let pixels = (width * height) as usize;
    if data.len() < pixels * 4 {
        return None;
    }
    let mut rgba = Vec::with_capacity(pixels * 4);
    for chunk in data.chunks_exact(4).take(pixels) {
        rgba.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
    }
    Some(rgba)
}

pub(super) fn decode_rgb565(width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let pixels = (width * height) as usize;
    if data.len() < pixels * 2 {
        return None;
    }
    let mut rgba = Vec::with_capacity(pixels * 4);
    for chunk in data.chunks_exact(2).take(pixels) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        let r = ((value >> 11) & 0x1f) as u8;
        let g = ((value >> 5) & 0x3f) as u8;
        let b = (value & 0x1f) as u8;
        rgba.extend_from_slice(&[
            (r << 3) | (r >> 2),
            (g << 2) | (g >> 4),
            (b << 3) | (b >> 2),
            255,
        ]);
    }
    Some(rgba)
}

pub(super) fn decode_rgba4444(width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let pixels = (width * height) as usize;
    if data.len() < pixels * 2 {
        return None;
    }
    let mut rgba = Vec::with_capacity(pixels * 4);
    for chunk in data.chunks_exact(2).take(pixels) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        let r = ((value >> 12) & 0xf) as u8;
        let g = ((value >> 8) & 0xf) as u8;
        let b = ((value >> 4) & 0xf) as u8;
        let a = (value & 0xf) as u8;
        rgba.extend_from_slice(&[r * 17, g * 17, b * 17, a * 17]);
    }
    Some(rgba)
}

pub(super) fn decode_argb4444(width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let pixels = (width * height) as usize;
    if data.len() < pixels * 2 {
        return None;
    }
    let mut rgba = Vec::with_capacity(pixels * 4);
    for chunk in data.chunks_exact(2).take(pixels) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        let a = ((value >> 12) & 0xf) as u8;
        let r = ((value >> 8) & 0xf) as u8;
        let g = ((value >> 4) & 0xf) as u8;
        let b = (value & 0xf) as u8;
        rgba.extend_from_slice(&[r * 17, g * 17, b * 17, a * 17]);
    }
    Some(rgba)
}

pub(super) fn decode_dxt1(width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let mut rgba = vec![0; (width * height * 4) as usize];
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let mut offset = 0;
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block = data.get(offset..offset + 8)?;
            offset += 8;
            let c0 = u16::from_le_bytes([block[0], block[1]]);
            let c1 = u16::from_le_bytes([block[2], block[3]]);
            let mut colors = [[0; 4]; 4];
            colors[0] = rgb565_to_rgba(c0);
            colors[1] = rgb565_to_rgba(c1);
            if c0 > c1 {
                colors[2] = lerp_color(colors[0], colors[1], 2, 1, 3);
                colors[3] = lerp_color(colors[0], colors[1], 1, 2, 3);
            } else {
                colors[2] = lerp_color(colors[0], colors[1], 1, 1, 2);
                colors[3] = [0, 0, 0, 0];
            }
            let bits = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
            for py in 0..4 {
                for px in 0..4 {
                    let idx = ((bits >> (2 * (py * 4 + px))) & 0x3) as usize;
                    put_pixel(
                        &mut rgba,
                        width,
                        height,
                        bx * 4 + px,
                        by * 4 + py,
                        colors[idx],
                    );
                }
            }
        }
    }
    Some(rgba)
}

pub(super) fn decode_dxt3(width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let mut rgba = vec![0; (width * height * 4) as usize];
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let mut offset = 0;
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let alpha = data.get(offset..offset + 8)?;
            let block = data.get(offset + 8..offset + 16)?;
            offset += 16;
            let colors = dxt_color_block(block, false)?;
            let bits = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
            for py in 0..4 {
                for px in 0..4 {
                    let color_idx = ((bits >> (2 * (py * 4 + px))) & 0x3) as usize;
                    let alpha_byte = alpha[(py * 4 + px) as usize / 2];
                    let nibble = if px % 2 == 0 {
                        alpha_byte & 0x0f
                    } else {
                        alpha_byte >> 4
                    };
                    let mut color = colors[color_idx];
                    color[3] = nibble * 17;
                    put_pixel(&mut rgba, width, height, bx * 4 + px, by * 4 + py, color);
                }
            }
        }
    }
    Some(rgba)
}

pub(super) fn decode_dxt5(width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let mut rgba = vec![0; (width * height * 4) as usize];
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let mut offset = 0;
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block = data.get(offset..offset + 16)?;
            offset += 16;
            let a0 = block[0];
            let a1 = block[1];
            let mut alphas = [0_u8; 8];
            alphas[0] = a0;
            alphas[1] = a1;
            if a0 > a1 {
                for i in 1..7 {
                    alphas[i + 1] = (((7 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 7) as u8;
                }
            } else {
                for i in 1..5 {
                    alphas[i + 1] = (((5 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 5) as u8;
                }
                alphas[6] = 0;
                alphas[7] = 255;
            }
            let mut alpha_bits = 0_u64;
            for i in 0..6 {
                alpha_bits |= (block[2 + i] as u64) << (8 * i);
            }
            let colors = dxt_color_block(&block[8..16], false)?;
            let bits = u32::from_le_bytes([block[12], block[13], block[14], block[15]]);
            for py in 0..4 {
                for px in 0..4 {
                    let i = py * 4 + px;
                    let color_idx = ((bits >> (2 * i)) & 0x3) as usize;
                    let alpha_idx = ((alpha_bits >> (3 * i)) & 0x7) as usize;
                    let mut color = colors[color_idx];
                    color[3] = alphas[alpha_idx];
                    put_pixel(&mut rgba, width, height, bx * 4 + px, by * 4 + py, color);
                }
            }
        }
    }
    Some(rgba)
}
