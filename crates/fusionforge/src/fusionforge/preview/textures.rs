use super::*;

pub(super) const MATERIAL_TEXTURE_PREVIEW_MAX_SIZE: u32 = 256;

#[derive(Debug, Clone)]
pub struct DecodedTexture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DecodedTextureMip {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub source_encoded: Vec<u8>,
}

impl DecodedTexture {
    pub fn image(&self) -> Option<RgbaImage> {
        RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
    }

    pub fn has_alpha(&self) -> bool {
        self.rgba
            .chunks_exact(4)
            .any(|pixel| pixel.get(3).copied().unwrap_or(255) < 255)
    }

    pub fn has_partial_alpha(&self) -> bool {
        self.rgba.chunks_exact(4).any(|pixel| {
            let alpha = pixel.get(3).copied().unwrap_or(255);
            alpha > 0 && alpha < 255
        })
    }
}

pub fn texture_to_data_url(
    env: &UnityEnvironment,
    texture: &UnityValue,
    max_size: u32,
    flip_y: bool,
) -> Option<String> {
    let decoded = decode_texture(env, texture)?;
    image_to_data_url(decoded.image()?, max_size, flip_y)
}

pub(super) fn texture_mip_encoded_len(format: i32, width: u32, height: u32) -> Option<usize> {
    let pixels = (width as usize).checked_mul(height as usize)?;
    match format {
        1 => Some(pixels),
        3 => pixels.checked_mul(3),
        4 | 5 | 14 => pixels.checked_mul(4),
        7 | 13 | 2 => pixels.checked_mul(2),
        10 | 28 => (width.div_ceil(4) as usize)
            .checked_mul(height.div_ceil(4) as usize)?
            .checked_mul(8),
        11 | 12 | 29 => (width.div_ceil(4) as usize)
            .checked_mul(height.div_ceil(4) as usize)?
            .checked_mul(16),
        _ => None,
    }
}

pub(super) fn texture_image_data(env: &UnityEnvironment, texture: &UnityValue) -> Option<Vec<u8>> {
    if let Some(data) = texture.get("image data").and_then(UnityValue::as_bytes) {
        if !data.is_empty() {
            return Some(data.to_vec());
        }
    }
    texture
        .get("m_StreamData")
        .and_then(|value| env.read_streaming_data(value))
}

pub(super) fn rgb565_to_rgba(value: u16) -> [u8; 4] {
    let r = ((value >> 11) & 0x1f) as u8;
    let g = ((value >> 5) & 0x3f) as u8;
    let b = (value & 0x1f) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
        255,
    ]
}

pub(super) fn dxt_color_block(block: &[u8], allow_transparent: bool) -> Option<[[u8; 4]; 4]> {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let mut colors = [[0; 4]; 4];
    colors[0] = rgb565_to_rgba(c0);
    colors[1] = rgb565_to_rgba(c1);
    if c0 > c1 || !allow_transparent {
        colors[2] = lerp_color(colors[0], colors[1], 2, 1, 3);
        colors[3] = lerp_color(colors[0], colors[1], 1, 2, 3);
    } else {
        colors[2] = lerp_color(colors[0], colors[1], 1, 1, 2);
        colors[3] = [0, 0, 0, 0];
    }
    Some(colors)
}

pub(super) fn texture_slot_priority(name: &str) -> i32 {
    let lower = name.to_lowercase();
    if matches!(name, "_MainTex" | "_BaseMap" | "_Diffuse" | "_MainTexture") {
        0
    } else if lower.contains("diffuse") || lower.contains("albedo") || lower.contains("base") {
        1
    } else if lower.contains("alpha")
        || lower.contains("opacity")
        || lower.contains("mask")
        || lower.contains("trans")
    {
        4
    } else if lower.contains("bump")
        || lower.contains("normal")
        || lower.contains("spec")
        || lower.contains("light")
        || lower.contains("shader")
    {
        5
    } else {
        2
    }
}

pub(super) fn is_alpha_texture_slot(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("alpha")
        || lower.contains("opacity")
        || lower.contains("mask")
        || lower.contains("trans")
}
