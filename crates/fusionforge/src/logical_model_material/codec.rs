use super::*;

pub(super) fn decode_exact_mip_level(
    env: &UnityEnvironment,
    texture: &UnityValue,
    texture_format: i64,
    width: u32,
    height: u32,
    source_level: &[u8],
) -> Result<Vec<u8>, String> {
    if matches!(texture_format, 28 | 29) {
        return Err("Crunch-compressed textures are not raw DXT mip slices".to_string());
    }
    let expected = mip_level_byte_length(width, height, texture_format)
        .ok_or_else(|| format!("unsupported TextureFormat {texture_format}"))?;
    if source_level.len() != expected {
        return Err(format!(
            "source level has {} bytes, expected exactly {}",
            source_level.len(),
            expected
        ));
    }
    let mut level_texture = texture.clone();
    let fields = match &mut level_texture {
        UnityValue::Object(fields) => fields,
        _ => return Err("Texture2D body is not an object".to_string()),
    };
    fields.insert("m_Width".to_string(), UnityValue::Int(i64::from(width)));
    fields.insert("m_Height".to_string(), UnityValue::Int(i64::from(height)));
    fields.insert(
        "m_TextureFormat".to_string(),
        UnityValue::Int(texture_format),
    );
    fields.insert(
        "image data".to_string(),
        UnityValue::Bytes(source_level.to_vec()),
    );
    // `source_level` is one isolated mip, not the original complete mip
    // chain. Clear the cloned Texture2D's mip flags before routing it through
    // the normal decoder; otherwise a mipmapped DXT texture makes the decoder
    // request all smaller levels from this one-level slice and rejects it.
    fields.insert("m_MipMap".to_string(), UnityValue::Bool(false));
    fields.insert("m_MipCount".to_string(), UnityValue::Int(1));
    fields.insert(
        "m_CompleteImageSize".to_string(),
        UnityValue::Int(
            i64::try_from(expected)
                .map_err(|_| "isolated mip byte length does not fit i64".to_string())?,
        ),
    );
    fields.remove("m_ImageData");
    fields.remove("m_StreamData");
    let decoded = decode_texture(env, &level_texture)
        .ok_or_else(|| format!("TextureFormat {texture_format} decoder rejected the level"))?;
    if decoded.width != width || decoded.height != height {
        return Err(format!(
            "decoder returned {}x{} instead of {}x{}",
            decoded.width, decoded.height, width, height
        ));
    }
    let expected_rgba = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "decoded RGBA byte count overflow".to_string())?;
    if decoded.rgba.len() != expected_rgba {
        return Err(format!(
            "decoder returned {} RGBA bytes, expected {}",
            decoded.rgba.len(),
            expected_rgba
        ));
    }
    Ok(decoded.rgba)
}

pub(super) fn exact_png_payload(png: &[u8], decoded_rgba: &[u8]) -> JsonValue {
    json!({
        "kind": "decoded-rgba8-png",
        "mimeType": "image/png",
        "dataUrl": format!("data:image/png;base64,{}", STANDARD.encode(png)),
        "byteLength": png.len(),
        "sha256": sha256_hex(png),
        "decodedRgbaSha256": sha256_hex(decoded_rgba),
        "pixelTransform": "vertical-flip-only-for-png-top-left-origin",
        "resized": false,
        "rgbRepair": false,
        "alphaMaskApplied": false,
        "tintApplied": false,
    })
}
