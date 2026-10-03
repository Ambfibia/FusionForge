use super::*;

pub(super) fn decode_world_animation_clip(clip: &ExportAnimationClip) -> Result<JsonValue> {
    let mut channels = Vec::new();
    decode_vec3_animation_curves(clip, "m_PositionCurves", "translation", true, &mut channels)?;
    decode_plain_rotation_animation_curves(clip, &mut channels)?;
    decode_compressed_rotation_animation_curves(clip, &mut channels)?;
    decode_vec3_animation_curves(clip, "m_ScaleCurves", "scale", false, &mut channels)?;

    let mut float_curves = Vec::new();
    for (source_index, binding) in optional_json_array(
        clip.fields.get("m_FloatCurves"),
        &format!("AnimationClip {} m_FloatCurves", clip.id),
    )?
    .iter()
    .enumerate()
    {
        let path = animation_curve_path(binding, &clip.id, "float")?;
        let attribute = binding
            .get("attribute")
            .or_else(|| binding.get("m_Attribute"))
            .and_then(JsonValue::as_str)
            .ok_or_else(|| {
                invalid_error(format!(
                    "AnimationClip {} float curve has no attribute",
                    clip.id
                ))
            })?;
        let curve = binding
            .get("curve")
            .or_else(|| binding.get("m_Curve"))
            .ok_or_else(|| {
                invalid_error(format!(
                    "AnimationClip {} float curve has no curve",
                    clip.id
                ))
            })?;
        let (times, values, in_tangents, out_tangents) =
            decode_scalar_keys(curve, &clip.id, "float")?;
        float_curves.push(serde_json::json!({
            "sourceIndex": source_index,
            "targetPath": path,
            "classId": binding.get("classID").or_else(|| binding.get("m_ClassID")).and_then(JsonValue::as_i64).unwrap_or(0),
            "attribute": attribute,
            "preInfinity": infinity_mode(curve, "m_PreInfinity"),
            "postInfinity": infinity_mode(curve, "m_PostInfinity"),
            "times": times,
            "values": values,
            "inTangents": in_tangents,
            "outTangents": out_tangents,
        }));
    }

    let duration = channels
        .iter()
        .chain(float_curves.iter())
        .filter_map(|curve| {
            curve
                .get("times")
                .and_then(JsonValue::as_array)
                .and_then(|times| times.last())
                .and_then(JsonValue::as_f64)
        })
        .chain(
            optional_json_array(
                clip.fields.get("m_Events"),
                &format!("AnimationClip {} m_Events", clip.id),
            )?
            .iter()
            .filter_map(|event| {
                event
                    .get("time")
                    .or_else(|| event.get("m_Time"))
                    .and_then(JsonValue::as_f64)
            }),
        )
        .fold(0.0_f64, f64::max);
    if !duration.is_finite() || duration < 0.0 {
        return invalid(format!("AnimationClip {} has invalid duration", clip.id));
    }
    let sample_rate = finite_json(
        clip.fields.get("m_SampleRate"),
        &format!("AnimationClip {} sample rate", clip.id),
    )?;
    if sample_rate <= 0.0 {
        return invalid(format!("AnimationClip {} has invalid sample rate", clip.id));
    }
    let looped = channels.iter().chain(float_curves.iter()).any(|curve| {
        curve.get("preInfinity").and_then(JsonValue::as_i64) == Some(2)
            || curve.get("postInfinity").and_then(JsonValue::as_i64) == Some(2)
    });

    Ok(serde_json::json!({
        "id": clip.id,
        "name": clip.name,
        "duration": duration,
        "sampleRate": sample_rate,
        "looped": looped,
        "channels": channels,
        "floatCurves": float_curves,
        "events": optional_json_array(
            clip.fields.get("m_Events"),
            &format!("AnimationClip {} m_Events", clip.id),
        )?,
    }))
}

pub(super) fn decode_vec3_animation_curves(
    clip: &ExportAnimationClip,
    field: &str,
    property: &str,
    mirror_translation: bool,
    output: &mut Vec<JsonValue>,
) -> Result<()> {
    for (source_index, binding) in optional_json_array(
        clip.fields.get(field),
        &format!("AnimationClip {} {field}", clip.id),
    )?
    .iter()
    .enumerate()
    {
        let path = animation_curve_path(binding, &clip.id, property)?;
        let curve = binding
            .get("curve")
            .or_else(|| binding.get("m_Curve"))
            .ok_or_else(|| {
                invalid_error(format!("AnimationClip {} {field} has no curve", clip.id))
            })?;
        let keys = required_curve_keys(curve, &clip.id, field)?;
        if keys.is_empty() {
            continue;
        }
        let mut times = Vec::with_capacity(keys.len());
        let mut values = Vec::with_capacity(keys.len());
        let mut inputs = Vec::with_capacity(keys.len());
        let mut outputs = Vec::with_capacity(keys.len());
        for key in keys {
            push_strict_time(
                &mut times,
                finite_json(key.get("time"), &format!("{} {field} key time", clip.id))?,
                &clip.id,
                field,
            )?;
            let convert = |value: Option<&JsonValue>, label: &str| -> Result<[f64; 3]> {
                let mut value = json_vec3(value, &format!("{} {field} {label}", clip.id))?;
                if mirror_translation {
                    value[0] = -value[0];
                }
                Ok(value)
            };
            values.push(convert(key.get("value"), "value")?);
            inputs.push(convert(key.get("inSlope"), "inSlope")?);
            outputs.push(convert(key.get("outSlope"), "outSlope")?);
        }
        output.push(serde_json::json!({
            "sourceIndex": source_index,
            "sourceEncoding": "plain",
            "targetPath": path,
            "property": property,
            "preInfinity": infinity_mode(curve, "m_PreInfinity"),
            "postInfinity": infinity_mode(curve, "m_PostInfinity"),
            "times": times,
            "values": values,
            "inTangents": inputs,
            "outTangents": outputs,
        }));
    }
    Ok(())
}

pub(super) fn decode_plain_rotation_animation_curves(
    clip: &ExportAnimationClip,
    output: &mut Vec<JsonValue>,
) -> Result<()> {
    for (source_index, binding) in optional_json_array(
        clip.fields.get("m_RotationCurves"),
        &format!("AnimationClip {} m_RotationCurves", clip.id),
    )?
    .iter()
    .enumerate()
    {
        let path = animation_curve_path(binding, &clip.id, "rotation")?;
        let curve = binding
            .get("curve")
            .or_else(|| binding.get("m_Curve"))
            .ok_or_else(|| {
                invalid_error(format!("AnimationClip {} rotation has no curve", clip.id))
            })?;
        let keys = required_curve_keys(curve, &clip.id, "m_RotationCurves")?;
        if keys.is_empty() {
            continue;
        }
        let mut times = Vec::with_capacity(keys.len());
        let mut values = Vec::with_capacity(keys.len());
        let mut inputs = Vec::with_capacity(keys.len());
        let mut outputs = Vec::with_capacity(keys.len());
        for key in keys {
            push_strict_time(
                &mut times,
                finite_json(key.get("time"), &format!("{} rotation key time", clip.id))?,
                &clip.id,
                "m_RotationCurves",
            )?;
            values.push(native_animation_quaternion(
                json_quaternion(key.get("value"), &format!("{} rotation value", clip.id))?,
                true,
                &clip.id,
            )?);
            inputs.push(native_animation_quaternion(
                json_quaternion(key.get("inSlope"), &format!("{} rotation inSlope", clip.id))?,
                false,
                &clip.id,
            )?);
            outputs.push(native_animation_quaternion(
                json_quaternion(
                    key.get("outSlope"),
                    &format!("{} rotation outSlope", clip.id),
                )?,
                false,
                &clip.id,
            )?);
        }
        output.push(serde_json::json!({
            "sourceIndex": source_index,
            "sourceEncoding": "plain",
            "targetPath": path,
            "property": "rotation",
            "preInfinity": infinity_mode(curve, "m_PreInfinity"),
            "postInfinity": infinity_mode(curve, "m_PostInfinity"),
            "times": times,
            "values": values,
            "inTangents": inputs,
            "outTangents": outputs,
        }));
    }
    Ok(())
}

pub(super) fn decode_compressed_rotation_animation_curves(
    clip: &ExportAnimationClip,
    output: &mut Vec<JsonValue>,
) -> Result<()> {
    for (source_index, binding) in optional_json_array(
        clip.fields.get("m_CompressedRotationCurves"),
        &format!("AnimationClip {} m_CompressedRotationCurves", clip.id),
    )?
    .iter()
    .enumerate()
    {
        let path = animation_curve_path(binding, &clip.id, "compressed rotation")?;
        let deltas = read_packed_animation_bits(
            binding.get("m_Times").ok_or_else(|| {
                invalid_error(format!(
                    "AnimationClip {} compressed rotation has no times",
                    clip.id
                ))
            })?,
            &clip.id,
        )?;
        let slopes = read_packed_animation_floats(
            binding.get("m_Slopes").ok_or_else(|| {
                invalid_error(format!(
                    "AnimationClip {} compressed rotation has no slopes",
                    clip.id
                ))
            })?,
            &clip.id,
        )?;
        let packed_values = binding.get("m_Values").ok_or_else(|| {
            invalid_error(format!(
                "AnimationClip {} compressed rotation has no values",
                clip.id
            ))
        })?;
        let count = usize_json(packed_values.get("m_NumItems"), "packed quaternion count")?;
        let bytes = animation_byte_payload(packed_values.get("m_Data"), &clip.id)?;
        if count == 0
            || deltas.len() != count
            || slopes.len() != count * 4
            || bytes.len() != count * 4
        {
            return invalid(format!(
                "AnimationClip {} compressed rotation cardinality mismatch",
                clip.id
            ));
        }
        let mut elapsed = 0_u64;
        let mut times = Vec::with_capacity(count);
        let mut values = Vec::with_capacity(count);
        let mut tangents = Vec::with_capacity(count);
        for (index, delta) in deltas.into_iter().enumerate() {
            elapsed = elapsed
                .checked_add(u64::from(delta))
                .ok_or_else(|| invalid_error("packed animation time overflow"))?;
            push_strict_time(
                &mut times,
                elapsed as f64 * 0.01,
                &clip.id,
                "m_CompressedRotationCurves",
            )?;
            let offset = index * 4;
            let packed = u32::from_le_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .expect("exact four-byte quaternion"),
            );
            values.push(native_animation_quaternion(
                unpack_legacy_animation_quaternion(packed),
                true,
                &clip.id,
            )?);
            tangents.push(native_animation_quaternion(
                [
                    slopes[offset],
                    slopes[offset + 1],
                    slopes[offset + 2],
                    slopes[offset + 3],
                ],
                false,
                &clip.id,
            )?);
        }
        let out_tangents = tangents.clone();
        output.push(serde_json::json!({
            "sourceIndex": source_index,
            "sourceEncoding": "compressed",
            "targetPath": path,
            "property": "rotation",
            "preInfinity": infinity_mode(binding, "m_PreInfinity"),
            "postInfinity": infinity_mode(binding, "m_PostInfinity"),
            "times": times,
            "values": values,
            "inTangents": tangents,
            "outTangents": out_tangents,
        }));
    }
    Ok(())
}

pub(super) fn decode_scalar_keys(
    curve: &JsonValue,
    clip_id: &str,
    label: &str,
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>)> {
    let keys = required_curve_keys(curve, clip_id, label)?;
    let mut times = Vec::with_capacity(keys.len());
    let mut values = Vec::with_capacity(keys.len());
    let mut inputs = Vec::with_capacity(keys.len());
    let mut outputs = Vec::with_capacity(keys.len());
    for key in keys {
        push_strict_time(
            &mut times,
            finite_json(key.get("time"), &format!("{clip_id} {label} key time"))?,
            clip_id,
            label,
        )?;
        values.push(finite_json(
            key.get("value"),
            &format!("{clip_id} {label} value"),
        )?);
        inputs.push(finite_json(
            key.get("inSlope"),
            &format!("{clip_id} {label} inSlope"),
        )?);
        outputs.push(finite_json(
            key.get("outSlope"),
            &format!("{clip_id} {label} outSlope"),
        )?);
    }
    Ok((times, values, inputs, outputs))
}

pub(super) fn animation_byte_payload(value: Option<&JsonValue>, clip_id: &str) -> Result<Vec<u8>> {
    let value = value
        .ok_or_else(|| invalid_error(format!("AnimationClip {clip_id} byte payload is absent")))?;
    let encoded = value
        .get("base64")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            invalid_error(format!(
                "AnimationClip {clip_id} byte payload has no base64"
            ))
        })?;
    let decoded = STANDARD.decode(encoded).map_err(|error| {
        invalid_error(format!(
            "AnimationClip {clip_id} byte payload is invalid: {error}"
        ))
    })?;
    if let Some(declared) = value.get("bytes").and_then(JsonValue::as_u64) {
        if decoded.len() as u64 != declared {
            return invalid(format!(
                "AnimationClip {clip_id} byte payload decoded {} bytes, declared {declared}",
                decoded.len()
            ));
        }
    }
    Ok(decoded)
}
