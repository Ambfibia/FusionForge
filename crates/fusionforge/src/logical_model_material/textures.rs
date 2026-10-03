use super::*;

pub(super) fn exact_movie_texture(env: &UnityEnvironment, key: ObjectKey) -> Result<JsonValue, String> {
    ensure_object_type(env, key, "MovieTexture", "dynamic material texture")?;
    let movie = env.read_object(key)?;
    let bytes = movie
        .get("m_MovieData")
        .and_then(UnityValue::as_bytes)
        .ok_or_else(|| format!("{} has no byte m_MovieData", object_id(env, key)))?;
    if bytes.is_empty() {
        return Err(format!("{} has empty m_MovieData", object_id(env, key)));
    }
    let audio_clip_pointer = match movie.get("m_AudioClip") {
        Some(UnityValue::Pointer(pointer)) => pointer_json(pointer),
        Some(_) => {
            return Err(format!("{}.m_AudioClip is not a PPtr", object_id(env, key)));
        }
        None => JsonValue::Null,
    };
    let looped = movie
        .get("m_Loop")
        .and_then(|value| match value {
            UnityValue::Bool(value) => Some(*value),
            _ => value.as_i64().map(|value| value != 0),
        })
        .unwrap_or(false);
    Ok(json!({
        "id": object_id(env, key),
        "source": source_object_json(env, key),
        "name": object_name(&movie),
        "objectType": "MovieTexture",
        "container": "ogg-theora",
        "mimeType": "video/ogg",
        "looped": looped,
        "audioClipPointer": audio_clip_pointer,
        "payload": {
            "encoding": "base64-data-url",
            "byteLength": bytes.len(),
            "sha256": sha256_hex(bytes),
            "dataUrl": format!("data:video/ogg;base64,{}", STANDARD.encode(bytes)),
        },
    }))
}

pub(crate) fn exact_texture(env: &UnityEnvironment, key: ObjectKey) -> Result<JsonValue, String> {
    let tuple_key = (key.asset, key.path_id);
    if let Some(texture) = env.exact_texture_cache.borrow().get(&tuple_key) {
        return texture.clone();
    }
    let texture = exact_texture_uncached(env, key);
    env.exact_texture_cache
        .borrow_mut()
        .insert(tuple_key, texture.clone());
    texture
}

pub(super) fn exact_texture_uncached(env: &UnityEnvironment, key: ObjectKey) -> Result<JsonValue, String> {
    ensure_object_type(env, key, "Texture2D", "texture")?;
    let texture = env.read_object(key)?;
    let width = required_u32(&texture, "m_Width", key, env)?;
    let height = required_u32(&texture, "m_Height", key, env)?;
    let texture_format = texture
        .get("m_TextureFormat")
        .and_then(UnityValue::as_i64)
        .ok_or_else(|| format!("{} has no m_TextureFormat", object_id(env, key)))?;
    let source_bytes = texture_source_bytes(env, &texture).ok_or_else(|| {
        format!(
            "{} has no embedded/streamed source bytes from which exact mip levels can be preserved",
            object_id(env, key)
        )
    })?;
    let serialized_mip_count = texture
        .get("m_MipCount")
        .and_then(UnityValue::as_i64)
        .and_then(|value| u32::try_from(value).ok());
    let mip_map = texture
        .get("m_MipMap")
        .and_then(UnityValue::as_i64)
        .map(|value| value != 0)
        .unwrap_or(false);
    let (mip_count, mut mip_count_evidence) = if let Some(count) = serialized_mip_count {
        (
            count,
            json!({ "source": "m_MipCount", "serialized": count }),
        )
    } else {
        let byte_length = source_bytes.len();
        let count = derive_mip_count(width, height, texture_format, byte_length, mip_map)?;
        let serialized_mip_map_contradiction = !mip_map && count > 1;
        (
            count,
            json!({
                "source": if serialized_mip_map_contradiction {
                    "derived-from-exact-complete-mip-prefix-despite-serialized-m_MipMap-false"
                } else {
                    "derived-from-dimensions-format-source-byte-length-and-m_MipMap"
                },
                "width": width,
                "height": height,
                "textureFormat": texture_format,
                "sourceByteLength": byte_length,
                "mMipMap": mip_map,
                "serializedMipMapContradiction": serialized_mip_map_contradiction,
                "intentionalDivergence": if serialized_mip_map_contradiction {
                    Some("publish every byte of the exact complete source mip prefix; preserve m_MipMap=false in importSettings")
                } else {
                    None
                },
            }),
        )
    };
    let layout = exact_mip_layout(width, height, texture_format, mip_count, source_bytes.len())?;
    if let Some(complete_size) = texture
        .get("m_CompleteImageSize")
        .and_then(UnityValue::as_i64)
        .and_then(|value| usize::try_from(value).ok())
    {
        // Legacy icon replacements can leave this redundant field stale
        // after resizing or changing compression. The serialized byte vector
        // and the exact layout above are authoritative: every byte must be
        // consumed by complete levels, with no inferred or padded payload.
        if complete_size != source_bytes.len() {
            mip_count_evidence["serializedCompleteImageSize"] = json!(complete_size);
            mip_count_evidence["resolvedSourceByteLength"] = json!(source_bytes.len());
            mip_count_evidence["completeImageSizeContradiction"] =
                json!("stale-header-with-exact-complete-mip-layout");
        }
    }
    let mut mip_levels = Vec::with_capacity(layout.len());
    for level in &layout {
        let end = level
            .source_byte_offset
            .checked_add(level.source_byte_length)
            .ok_or_else(|| "mip source range overflow".to_string())?;
        let source_level = source_bytes
            .get(level.source_byte_offset..end)
            .ok_or_else(|| {
                format!(
                    "{} mip {} source slice is out of bounds",
                    object_id(env, key),
                    level.level
                )
            })?;
        let decoded = decode_exact_mip_level(
            env,
            &texture,
            texture_format,
            level.width,
            level.height,
            source_level,
        )
        .map_err(|error| {
            format!(
                "{} mip {} ({}x{}) cannot be decoded exactly: {error}",
                object_id(env, key),
                level.level,
                level.width,
                level.height
            )
        })?;
        let png = exact_png_bytes(level.width, level.height, &decoded, true)?;
        mip_levels.push(json!({
            "level": level.level,
            "width": level.width,
            "height": level.height,
            "sourceByteOffset": level.source_byte_offset,
            "sourceByteLength": level.source_byte_length,
            "sourceByteSha256": sha256_hex(source_level),
            "decodedRgbaByteLength": decoded.len(),
            "decodedRgbaSha256": sha256_hex(&decoded),
            "payload": exact_png_payload(&png, &decoded),
        }));
    }
    let payload = mip_levels
        .first()
        .and_then(|level| level.get("payload"))
        .cloned()
        .ok_or_else(|| format!("{} has no base mip payload", object_id(env, key)))?;

    let mut import_settings = JsonMap::new();
    for field in [
        "m_MipCount",
        "m_MipMap",
        "m_IsReadable",
        "m_ReadAllowed",
        "m_CompleteImageSize",
        "m_Dimension",
        "m_TextureDimension",
        "m_ImageCount",
        "m_Limit",
        "m_Touchable",
        "m_FilterMode",
        "m_Aniso",
        "m_MipBias",
        "m_WrapMode",
        "m_LightmapFormat",
        "m_ColorSpace",
    ] {
        if let Some(value) = texture.get(field) {
            import_settings.insert(field.to_string(), unity_to_lossless_json(value));
        }
    }
    if let Some(settings) = texture.get("m_TextureSettings") {
        import_settings.insert(
            "m_TextureSettings".to_string(),
            unity_to_lossless_json(settings),
        );
    }
    let sampler = json!({
        "filterMode": exact_texture_setting(&texture, "m_FilterMode", 1),
        "wrapMode": exact_texture_setting(&texture, "m_WrapMode", 0),
        "aniso": exact_texture_setting(&texture, "m_Aniso", 1),
        "mipBias": exact_texture_setting(&texture, "m_MipBias", 0),
    });
    let raw_source = json!({
        "byteLength": source_bytes.len(),
        "sha256": sha256_hex(&source_bytes),
        "embeddedInExactSource": false,
        "layout": "largest-to-smallest-contiguous",
        "levelCount": mip_levels.len(),
    });

    Ok(json!({
        "id": object_id(env, key),
        "source": source_object_json(env, key),
        "name": object_name(&texture),
        "width": width,
        "height": height,
        "textureFormat": texture_format,
        "textureFormatName": texture_format_name(texture_format),
        "mipCount": mip_count,
        "mipCountEvidence": mip_count_evidence,
        "sampler": sampler,
        "importSettings": import_settings,
        "sourcePayload": raw_source,
        "mipLevels": mip_levels,
        "payload": payload,
    }))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExactMipLevelLayout {
    pub(super) level: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) source_byte_offset: usize,
    pub(super) source_byte_length: usize,
}

pub(super) fn exact_mip_layout(
    width: u32,
    height: u32,
    texture_format: i64,
    mip_count: u32,
    source_byte_length: usize,
) -> Result<Vec<ExactMipLevelLayout>, String> {
    if width == 0 || height == 0 || mip_count == 0 {
        return Err("exact mip layout requires non-zero dimensions and mip count".to_string());
    }
    if matches!(texture_format, 28 | 29) {
        return Err(format!(
            "TextureFormat {} ({}) is Crunch-compressed; its payload is not a contiguous raw DXT mip chain and requires a dedicated lossless Crunch decoder",
            texture_format,
            texture_format_name(texture_format).unwrap_or("unknown")
        ));
    }
    let max_mips = 32 - width.max(height).leading_zeros();
    if mip_count > max_mips {
        return Err(format!(
            "{}x{} texture declares {} mip levels, maximum is {}",
            width, height, mip_count, max_mips
        ));
    }

    let mut levels = Vec::with_capacity(mip_count as usize);
    let mut level_width = width;
    let mut level_height = height;
    let mut offset = 0usize;
    for level in 0..mip_count {
        let byte_length = mip_level_byte_length(level_width, level_height, texture_format)
            .ok_or_else(|| {
                format!(
                    "TextureFormat {} ({}) has no exact native mip decoder",
                    texture_format,
                    texture_format_name(texture_format).unwrap_or("unknown")
                )
            })?;
        levels.push(ExactMipLevelLayout {
            level,
            width: level_width,
            height: level_height,
            source_byte_offset: offset,
            source_byte_length: byte_length,
        });
        offset = offset
            .checked_add(byte_length)
            .ok_or_else(|| "exact mip chain byte count overflow".to_string())?;
        level_width = (level_width / 2).max(1);
        level_height = (level_height / 2).max(1);
    }
    if offset != source_byte_length {
        return Err(format!(
            "{}x{} TextureFormat {} declares {} mip levels occupying {} bytes, but source chain contains {} bytes",
            width, height, texture_format, mip_count, offset, source_byte_length
        ));
    }
    Ok(levels)
}

pub(super) fn texture_source_bytes(env: &UnityEnvironment, texture: &UnityValue) -> Option<Vec<u8>> {
    for field in ["image data", "m_ImageData"] {
        if let Some(bytes) = texture.get(field).and_then(UnityValue::as_bytes) {
            if !bytes.is_empty() {
                return Some(bytes.to_vec());
            }
        }
    }
    texture
        .get("m_StreamData")
        .and_then(|stream| env.read_streaming_data(stream))
}

pub(super) fn exact_texture_setting(texture: &UnityValue, field: &str, unity_default: i64) -> JsonValue {
    if let Some(value) = texture
        .get("m_TextureSettings")
        .and_then(|settings| settings.get(field))
    {
        return json!({
            "value": unity_to_lossless_json(value),
            "source": format!("m_TextureSettings.{field}"),
            "serialized": true,
        });
    }
    if let Some(value) = texture.get(field) {
        return json!({
            "value": unity_to_lossless_json(value),
            "source": field,
            "serialized": true,
        });
    }
    json!({
        "value": unity_default,
        "source": "unity-texture-serialized-default",
        "serialized": false,
    })
}

pub(super) fn derive_mip_count(
    width: u32,
    height: u32,
    texture_format: i64,
    source_byte_length: usize,
    mip_map: bool,
) -> Result<u32, String> {
    let base = mip_level_byte_length(width, height, texture_format)
        .ok_or_else(|| format!("cannot derive mip count for TextureFormat {texture_format}"))?;
    if !mip_map && source_byte_length == base {
        return Ok(1);
    }

    let mut level_width = width;
    let mut level_height = height;
    let mut total = 0usize;
    let mut count = 0u32;
    loop {
        total = total
            .checked_add(
                mip_level_byte_length(level_width, level_height, texture_format).ok_or_else(
                    || format!("cannot derive mip count for TextureFormat {texture_format}"),
                )?,
            )
            .ok_or_else(|| "mip byte count overflow".to_string())?;
        count += 1;
        if total == source_byte_length {
            return Ok(count);
        }
        if total > source_byte_length || (level_width == 1 && level_height == 1) {
            break;
        }
        level_width = (level_width / 2).max(1);
        level_height = (level_height / 2).max(1);
    }
    Err(format!(
        "{} {}x{} TextureFormat {} source byte length {} does not match a complete mip prefix (calculated {})",
        if mip_map {
            "mipmapped"
        } else {
            "serialized non-mipmapped"
        },
        width,
        height,
        texture_format,
        source_byte_length,
        total
    ))
}

pub(super) fn mip_level_byte_length(width: u32, height: u32, texture_format: i64) -> Option<usize> {
    let pixels = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?;
    match texture_format {
        1 => Some(pixels),
        3 => pixels.checked_mul(3),
        4 | 5 | 14 => pixels.checked_mul(4),
        2 | 7 | 13 => pixels.checked_mul(2),
        10 | 28 => {
            let blocks = usize::try_from(width.div_ceil(4))
                .ok()?
                .checked_mul(usize::try_from(height.div_ceil(4)).ok()?)?;
            blocks.checked_mul(8)
        }
        11 | 12 | 29 => {
            let blocks = usize::try_from(width.div_ceil(4))
                .ok()?
                .checked_mul(usize::try_from(height.div_ceil(4)).ok()?)?;
            blocks.checked_mul(16)
        }
        _ => None,
    }
}

pub(super) fn exact_png_bytes(width: u32, height: u32, rgba: &[u8], flip_y: bool) -> Result<Vec<u8>, String> {
    let mut image = RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| format!("invalid RGBA payload for {width}x{height}"))?;
    if flip_y {
        image::imageops::flip_vertical_in_place(&mut image);
    }
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|error| format!("PNG encode failed: {error}"))?;
    Ok(png)
}

pub(super) fn texture_format_name(format: i64) -> Option<&'static str> {
    match format {
        1 => Some("Alpha8"),
        2 => Some("ARGB4444"),
        3 => Some("RGB24"),
        4 => Some("RGBA32"),
        5 => Some("ARGB32"),
        7 => Some("RGB565"),
        10 => Some("DXT1"),
        11 => Some("DXT3"),
        12 => Some("DXT5"),
        13 => Some("RGBA4444"),
        14 => Some("BGRA32"),
        28 => Some("DXT1Crunched"),
        29 => Some("DXT5Crunched"),
        _ => None,
    }
}
