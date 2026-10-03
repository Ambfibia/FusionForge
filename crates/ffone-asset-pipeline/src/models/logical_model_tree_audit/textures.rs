use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn audit_exact_mip_pngs(
    output_root: &Path,
    glb_relative: &str,
    image_index: usize,
    image: &Value,
    base_uri: &str,
    pngs: &BTreeSet<String>,
    referenced: &mut BTreeSet<String>,
    violations: &mut Vec<LogicalModelTreeViolation>,
) -> Result<()> {
    let exact = image.pointer("/extras/ffone");
    let levels = exact
        .and_then(|value| value.get("mipLevels"))
        .and_then(Value::as_array);
    let Some(levels) = levels else {
        push_violation(
            violations,
            "missing_exact_mip_levels",
            glb_relative,
            format!("image {image_index} has no exact source mip-level contract"),
        );
        return Ok(());
    };
    let provenance = exact.and_then(|value| value.get("mipProvenance"));
    let source_mip_count = provenance
        .and_then(|value| value.get("sourceMipCount"))
        .and_then(Value::as_u64);
    let source_chain_length = provenance
        .and_then(|value| value.get("sourceChainByteLength"))
        .and_then(Value::as_u64);
    let source_chain_hash = provenance
        .and_then(|value| value.get("sourceChainSha256"))
        .and_then(Value::as_str);
    let format = provenance
        .and_then(|value| value.get("sourceTextureFormat"))
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok());
    let format_name = provenance
        .and_then(|value| value.get("sourceTextureFormatName"))
        .and_then(Value::as_str);
    let expected_policy = if levels.len() > 1 {
        "exactSourceLevels"
    } else {
        "baseLevelOnly"
    };
    if source_mip_count != Some(to_u64(levels.len()))
        || provenance
            .and_then(|value| value.get("sourceChainComplete"))
            .and_then(Value::as_bool)
            != Some(true)
        || provenance
            .and_then(|value| value.get("sourceLayout"))
            .and_then(Value::as_str)
            != Some("largestToSmallestContiguous")
        || provenance
            .and_then(|value| value.get("publishedPixelTransform"))
            .and_then(Value::as_str)
            != Some("vertical-flip-only-for-png-top-left-origin")
        || provenance
            .and_then(|value| value.get("publishedPolicy"))
            .and_then(Value::as_str)
            != Some(expected_policy)
        || source_chain_length.is_none_or(|value| value == 0)
        || source_chain_hash.is_none_or(|value| !is_sha256(value))
        || format.and_then(audited_texture_format_name) != format_name
    {
        push_violation(
            violations,
            "invalid_exact_mip_provenance",
            glb_relative,
            format!("image {image_index} mip provenance is incomplete or contradictory"),
        );
    }

    let expected_base_name = base_uri.rsplit('/').next();
    let mip_stem = expected_base_name.and_then(|name| name.strip_suffix(".png"));
    let base_parent = base_uri.rsplit_once('/').map(|(parent, _)| parent);
    let mut expected_width = image
        .pointer("/extras/ffone/width")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    let mut expected_height = image
        .pointer("/extras/ffone/height")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    let mut expected_source_offset = 0u64;
    for (level_index, level) in levels.iter().enumerate() {
        let expected_level = to_u64(level_index);
        let level_number = level.get("level").and_then(Value::as_u64);
        let width = level
            .get("width")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok());
        let height = level
            .get("height")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok());
        let source_offset = level.get("sourceByteOffset").and_then(Value::as_u64);
        let source_length = level.get("sourceByteLength").and_then(Value::as_u64);
        let source_hash = level.get("sourceByteSha256").and_then(Value::as_str);
        let decoded_length = level.get("decodedRgba8ByteLength").and_then(Value::as_u64);
        let decoded_hash = level.get("decodedRgba8Sha256").and_then(Value::as_str);
        let png_length = level.get("pngByteLength").and_then(Value::as_u64);
        let png_hash = level.get("pngSha256").and_then(Value::as_str);
        let expected_source_length = format.and_then(|format| {
            audited_texture_mip_byte_length(format, expected_width, expected_height)
        });
        let expected_rgba_length = u64::from(expected_width)
            .checked_mul(u64::from(expected_height))
            .and_then(|pixels| pixels.checked_mul(4));
        if level_number != Some(expected_level)
            || width != Some(expected_width)
            || height != Some(expected_height)
            || source_offset != Some(expected_source_offset)
            || source_length != expected_source_length
            || source_hash.is_none_or(|value| !is_sha256(value))
            || decoded_length != expected_rgba_length
            || decoded_hash.is_none_or(|value| !is_sha256(value))
            || png_length.is_none_or(|value| value == 0)
            || png_hash.is_none_or(|value| !is_sha256(value))
        {
            push_violation(
                violations,
                "invalid_exact_mip_level",
                glb_relative,
                format!("image {image_index} mip {level_index} metadata is invalid"),
            );
        }
        let expected_uri = if level_index == 0 {
            Some(base_uri.to_string())
        } else {
            mip_stem.map(|stem| match base_parent {
                Some(parent) => format!("{parent}/{stem}.mips/mip-{level_index:02}.png"),
                None => format!("{stem}.mips/mip-{level_index:02}.png"),
            })
        };
        let uri = level.get("uri").and_then(Value::as_str);
        if uri != expected_uri.as_deref() {
            push_violation(
                violations,
                "exact_mip_uri_mismatch",
                glb_relative,
                format!("image {image_index} mip {level_index} URI must be {expected_uri:?}"),
            );
        }
        if let Some(uri) = uri {
            let Some(relative) = safe_image_relative(glb_relative, uri) else {
                push_violation(
                    violations,
                    "unsafe_exact_mip_uri",
                    glb_relative,
                    format!("image {image_index} mip {level_index} URI is unsafe"),
                );
                continue;
            };
            referenced.insert(relative.clone());
            if !pngs.contains(&relative) {
                push_violation(
                    violations,
                    "missing_exact_mip_png",
                    &relative,
                    format!("referenced by {glb_relative} image {image_index} mip {level_index}"),
                );
            } else {
                let path = output_root.join(Path::new(&relative));
                let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
                let actual_hash = format!("{:x}", Sha256::digest(&bytes));
                if png_length != Some(to_u64(bytes.len())) || png_hash != Some(actual_hash.as_str())
                {
                    push_violation(
                        violations,
                        "exact_mip_png_hash_mismatch",
                        &relative,
                        "mip PNG byte length/SHA-256 differs from GLB extras",
                    );
                }
                if png_dimensions(&bytes) != Some((expected_width, expected_height)) {
                    push_violation(
                        violations,
                        "exact_mip_png_dimension_mismatch",
                        &relative,
                        format!(
                            "mip PNG dimensions differ from {expected_width}x{expected_height}"
                        ),
                    );
                }
            }
        }
        expected_source_offset = expected_source_offset
            .checked_add(source_length.unwrap_or(0))
            .unwrap_or(u64::MAX);
        expected_width = (expected_width / 2).max(1);
        expected_height = (expected_height / 2).max(1);
    }
    if source_chain_length != Some(expected_source_offset) {
        push_violation(
            violations,
            "exact_mip_chain_length_mismatch",
            glb_relative,
            format!("image {image_index} mip slices do not cover the source chain"),
        );
    }
    Ok(())
}

pub(super) fn audited_texture_mip_byte_length(format: i32, width: u32, height: u32) -> Option<u64> {
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    match format {
        1 => Some(pixels),
        3 => pixels.checked_mul(3),
        4 | 5 | 14 => pixels.checked_mul(4),
        2 | 7 | 13 => pixels.checked_mul(2),
        10 => u64::from(width.div_ceil(4))
            .checked_mul(u64::from(height.div_ceil(4)))?
            .checked_mul(8),
        11 | 12 => u64::from(width.div_ceil(4))
            .checked_mul(u64::from(height.div_ceil(4)))?
            .checked_mul(16),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn audit_report_mip_pngs(
    output_root: &Path,
    report_relative: &str,
    glb_relative: &str,
    texture_index: usize,
    texture: &Value,
    pngs: &BTreeSet<String>,
    violations: &mut Vec<LogicalModelTreeViolation>,
) -> Result<()> {
    let Some(levels) = texture.get("mipLevels").and_then(Value::as_array) else {
        push_violation(
            violations,
            "missing_report_mip_levels",
            report_relative,
            format!("texture report {texture_index} has no exact mip-level list"),
        );
        return Ok(());
    };
    let source_count = texture.get("sourceMipCount").and_then(Value::as_u64);
    let expected_policy = if levels.len() > 1 {
        "exactSourceLevels"
    } else {
        "baseLevelOnly"
    };
    if source_count != Some(to_u64(levels.len()))
        || texture.get("publishedPolicy").and_then(Value::as_str) != Some(expected_policy)
    {
        push_violation(
            violations,
            "invalid_report_mip_count",
            report_relative,
            format!("texture report {texture_index} mip count/policy is contradictory"),
        );
    }
    let base_uri = texture.get("uri").and_then(Value::as_str);
    let base_byte_length = texture.get("byteLength").and_then(Value::as_u64);
    let base_hash = texture.get("sha256").and_then(Value::as_str);
    let mut expected_width = texture
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    let mut expected_height = texture
        .get("height")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    let mut expected_source_offset = 0u64;
    for (level_index, level) in levels.iter().enumerate() {
        let uri = level.get("uri").and_then(Value::as_str);
        let png_length = level.get("pngByteLength").and_then(Value::as_u64);
        let png_hash = level.get("pngSha256").and_then(Value::as_str);
        let source_length = level.get("sourceByteLength").and_then(Value::as_u64);
        if level.get("level").and_then(Value::as_u64) != Some(to_u64(level_index))
            || level.get("width").and_then(Value::as_u64) != Some(u64::from(expected_width))
            || level.get("height").and_then(Value::as_u64) != Some(u64::from(expected_height))
            || level.get("sourceByteOffset").and_then(Value::as_u64) != Some(expected_source_offset)
            || source_length.is_none_or(|value| value == 0)
            || level
                .get("sourceByteSha256")
                .and_then(Value::as_str)
                .is_none_or(|value| !is_sha256(value))
            || level
                .get("decodedRgba8Sha256")
                .and_then(Value::as_str)
                .is_none_or(|value| !is_sha256(value))
            || png_length.is_none_or(|value| value == 0)
            || png_hash.is_none_or(|value| !is_sha256(value))
        {
            push_violation(
                violations,
                "invalid_report_mip_level",
                report_relative,
                format!("texture report {texture_index} mip {level_index} is invalid"),
            );
        }
        if level_index == 0
            && (uri != base_uri || png_length != base_byte_length || png_hash != base_hash)
        {
            push_violation(
                violations,
                "report_base_mip_mismatch",
                report_relative,
                format!("texture report {texture_index} base fields differ from mipLevels[0]"),
            );
        }
        if let Some(uri) = uri {
            let Some(relative) = safe_image_relative(glb_relative, uri) else {
                push_violation(
                    violations,
                    "unsafe_report_mip_uri",
                    report_relative,
                    format!("texture report {texture_index} mip {level_index} URI is unsafe"),
                );
                continue;
            };
            if !pngs.contains(&relative) {
                push_violation(
                    violations,
                    "missing_report_mip_png",
                    &relative,
                    format!("referenced by texture report {texture_index} mip {level_index}"),
                );
            } else {
                let path = output_root.join(Path::new(&relative));
                let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
                let actual_hash = format!("{:x}", Sha256::digest(&bytes));
                if png_length != Some(to_u64(bytes.len())) || png_hash != Some(actual_hash.as_str())
                {
                    push_violation(
                        violations,
                        "report_mip_png_hash_mismatch",
                        &relative,
                        "mip PNG byte length/SHA-256 differs from publish report",
                    );
                }
                if png_dimensions(&bytes) != Some((expected_width, expected_height)) {
                    push_violation(
                        violations,
                        "report_mip_png_dimension_mismatch",
                        &relative,
                        format!(
                            "mip PNG dimensions differ from report {expected_width}x{expected_height}"
                        ),
                    );
                }
            }
        }
        expected_source_offset = expected_source_offset
            .checked_add(source_length.unwrap_or(0))
            .unwrap_or(u64::MAX);
        expected_width = (expected_width / 2).max(1);
        expected_height = (expected_height / 2).max(1);
    }
    Ok(())
}

pub(super) fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24
        || &bytes[0..8] != SIGNATURE
        || read_be_u32(bytes, 8)? != 13
        || &bytes[12..16] != b"IHDR"
    {
        return None;
    }
    Some((read_be_u32(bytes, 16)?, read_be_u32(bytes, 20)?))
}
