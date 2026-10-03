use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceDynamicTexturePayload {
    pub(super) encoding: String,
    pub(super) byte_length: u64,
    pub(super) sha256: String,
    pub(super) data_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceTextureChainPayload {
    pub(super) byte_length: usize,
    pub(super) sha256: String,
    pub(super) embedded_in_exact_source: bool,
    pub(super) layout: String,
    pub(super) level_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceTexturePayload {
    pub(super) kind: String,
    pub(super) mime_type: String,
    pub(super) data_url: String,
    pub(super) byte_length: usize,
    pub(super) sha256: String,
    pub(super) decoded_rgba_sha256: String,
    pub(super) pixel_transform: PublishedPixelTransform,
    pub(super) resized: bool,
    pub(super) rgb_repair: bool,
    pub(super) alpha_mask_applied: bool,
    pub(super) tint_applied: bool,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub(super) enum SourceExactTrsKeyPayload {
    Vec3(SourceVec3KeyPayload),
    Quaternion(SourceQuatKeyPayload),
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceVec3KeyPayload {
    pub(super) time: f64,
    pub(super) value: [f64; 3],
    pub(super) in_tangent: Option<[f64; 3]>,
    pub(super) out_tangent: Option<[f64; 3]>,
    pub(super) tangent_mode: Option<i32>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceQuatKeyPayload {
    pub(super) time: f64,
    pub(super) value: [f64; 4],
    pub(super) in_tangent: Option<[f64; 4]>,
    pub(super) out_tangent: Option<[f64; 4]>,
    pub(super) tangent_mode: Option<i32>,
}

pub(super) fn decode_exact_png(texture: &SourceTexture) -> Result<Vec<u8>> {
    decode_exact_png_payload(
        &texture.name,
        texture.width,
        texture.height,
        &texture.payload,
    )
}

pub(super) fn decode_exact_png_payload(
    texture_name: &str,
    expected_width: u32,
    expected_height: u32,
    payload: &SourceTexturePayload,
) -> Result<Vec<u8>> {
    if payload.kind != "decoded-rgba8-png"
        || payload.mime_type != "image/png"
        || payload.pixel_transform != PublishedPixelTransform::VerticalFlipOnlyForPngTopLeftOrigin
        || payload.resized
        || payload.rgb_repair
        || payload.alpha_mask_applied
        || payload.tint_applied
    {
        return invalid(format!(
            "texture {:?} payload is not an untouched full-resolution decoded PNG",
            texture_name
        ));
    }
    const PREFIX: &str = "data:image/png;base64,";
    let encoded = payload.data_url.strip_prefix(PREFIX).ok_or_else(|| {
        invalid_error(format!(
            "texture {:?} payload is not strict data:image/png;base64",
            texture_name
        ))
    })?;
    if encoded.is_empty() || encoded.chars().any(char::is_whitespace) {
        return invalid(format!(
            "texture {:?} PNG base64 is empty or contains whitespace",
            texture_name
        ));
    }
    let bytes = BASE64_STANDARD.decode(encoded).map_err(|error| {
        invalid_error(format!(
            "texture {:?} PNG base64 is invalid: {error}",
            texture_name
        ))
    })?;
    if bytes.len() != payload.byte_length || sha256_hex(&bytes) != payload.sha256 {
        return invalid(format!(
            "texture {:?} PNG byteLength/SHA-256 evidence does not match payload",
            texture_name
        ));
    }
    if bytes.len() < 24
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || u32::from_be_bytes(bytes[8..12].try_into().expect("four-byte slice")) != 13
        || &bytes[12..16] != b"IHDR"
    {
        return invalid(format!("texture {:?} payload is not a PNG", texture_name));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("four-byte slice"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("four-byte slice"));
    if width != expected_width || height != expected_height {
        return invalid(format!(
            "texture {:?} PNG IHDR is {width}x{height}, source is {}x{}",
            texture_name, expected_width, expected_height
        ));
    }
    Ok(bytes)
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SourceConstantCurvePayload {
    Vec3 {
        value: [f64; 3],
        in_tangent: Option<[f64; 3]>,
        out_tangent: Option<[f64; 3]>,
        tangent_mode: Option<i32>,
    },
    Quaternion {
        value: [f64; 4],
        in_tangent: Option<[f64; 4]>,
        out_tangent: Option<[f64; 4]>,
        tangent_mode: Option<i32>,
    },
}

pub(super) fn source_track_constant_payload(
    track: SourceRecoveryTrackRef<'_>,
    duration: f64,
    context: &str,
) -> Result<Option<SourceConstantCurvePayload>> {
    let result = match track {
        SourceRecoveryTrackRef::Vec3(track) => source_constant_curve_payload(
            &SourceDuplicateTrsKeys::Vec3(track.keys.clone()),
            &track.duplicate_keys,
            track.source_key_count,
            &track.interpolation,
            duration,
            context,
        ),
        SourceRecoveryTrackRef::Quaternion(track) => source_constant_curve_payload(
            &SourceDuplicateTrsKeys::Quaternion(track.keys.clone()),
            &track.duplicate_keys,
            track.source_key_count,
            &track.interpolation,
            duration,
            context,
        ),
    };
    match result {
        Ok(payload) => Ok(Some(payload)),
        Err(_) => Ok(None),
    }
}

pub(super) fn source_constant_curve_payload(
    keys: &SourceDuplicateTrsKeys,
    duplicate_keys: &[SourceDuplicateAnimationKey],
    source_key_count: usize,
    interpolation: &str,
    duration: f64,
    context: &str,
) -> Result<SourceConstantCurvePayload> {
    if !duplicate_keys.is_empty()
        || source_keys_len(keys) == 0
        || source_key_count != source_keys_len(keys)
    {
        return invalid(format!(
            "{context} is not a complete non-duplicate constant curve"
        ));
    }
    match keys {
        SourceDuplicateTrsKeys::Vec3(keys) => {
            if keys
                .iter()
                .enumerate()
                .any(|(index, key)| key.source_key_index != index)
            {
                return invalid(format!("{context} key provenance is not source ordered"));
            }
            validate_source_recovery_times(keys.iter().map(|key| key.time), duration, context)?;
            validate_source_recovery_interpolation_vec3(keys, interpolation, context)?;
            let mut payload = None;
            for key in keys {
                validate_finite_source_values(&key.value, context)?;
                if let Some(values) = &key.in_tangent {
                    validate_finite_source_values(values, context)?;
                }
                if let Some(values) = &key.out_tangent {
                    validate_finite_source_values(values, context)?;
                }
                let current = SourceConstantCurvePayload::Vec3 {
                    value: key.value,
                    in_tangent: key.in_tangent,
                    out_tangent: key.out_tangent,
                    tangent_mode: key.tangent_mode,
                };
                if payload
                    .as_ref()
                    .is_some_and(|previous| previous != &current)
                {
                    return invalid(format!("{context} is not constant"));
                }
                payload.get_or_insert(current);
            }
            Ok(payload.expect("non-empty keys"))
        }
        SourceDuplicateTrsKeys::Quaternion(keys) => {
            if keys
                .iter()
                .enumerate()
                .any(|(index, key)| key.source_key_index != index)
            {
                return invalid(format!("{context} key provenance is not source ordered"));
            }
            validate_source_recovery_times(keys.iter().map(|key| key.time), duration, context)?;
            validate_source_recovery_interpolation_quaternion(keys, interpolation, context)?;
            let mut payload = None;
            for key in keys {
                validate_finite_source_values(&key.value, context)?;
                if let Some(values) = &key.in_tangent {
                    validate_finite_source_values(values, context)?;
                }
                if let Some(values) = &key.out_tangent {
                    validate_finite_source_values(values, context)?;
                }
                let current = SourceConstantCurvePayload::Quaternion {
                    value: key.value,
                    in_tangent: key.in_tangent,
                    out_tangent: key.out_tangent,
                    tangent_mode: key.tangent_mode,
                };
                if payload
                    .as_ref()
                    .is_some_and(|previous| previous != &current)
                {
                    return invalid(format!("{context} is not constant"));
                }
                payload.get_or_insert(current);
            }
            Ok(payload.expect("non-empty keys"))
        }
    }
}

pub(super) fn source_animation_has_model_payload(animation: &SourceAnimation) -> bool {
    animation
        .animation_data
        .translations
        .iter()
        .any(|track| !track.unbound_model_target)
        || animation
            .animation_data
            .rotations
            .iter()
            .any(|track| !track.unbound_model_target)
        || animation
            .animation_data
            .scales
            .iter()
            .any(|track| !track.unbound_model_target)
        || !animation.animation_data.float_curves.is_empty()
        || animation
            .animation_data
            .empty_trs_bindings
            .iter()
            .any(|binding| !binding.unbound_model_target)
        || !animation.animation_data.duplicate_trs_bindings.is_empty()
        || !animation.animation_data.time_recoveries.is_empty()
        || !animation.animation_data.curve_recoveries.is_empty()
        || !animation.events.is_empty()
}
