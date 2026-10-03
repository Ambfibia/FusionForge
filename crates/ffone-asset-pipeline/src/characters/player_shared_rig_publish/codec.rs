use super::*;

pub(super) fn decode_retargeted_custom_clip(
    catalog: &DumpCatalog,
    nodes: &[PlayerRigNode],
    root_name: &str,
    semantic_name: &str,
    source_name: &str,
    source_path_id: i64,
    minimum_mapped_paths: usize,
) -> Result<AnimationClip> {
    let source = catalog.get(source_path_id)?;
    if source.object_type != "AnimationClip" || source.name != source_name {
        return rig_error(format!(
            "custom clip {source_name}#{source_path_id} identity is {} {:?}",
            source.object_type, source.name
        ));
    }
    let is_extension = semantic_name.starts_with("ffr_") && semantic_name != "ffr_run";
    if !is_extension {
        return rig_error(format!(
            "custom clip semantic name {semantic_name:?} is not an FFR extension"
        ));
    }

    let paths = nodes
        .iter()
        .map(|node| {
            let relative = node
                .full_path
                .strip_prefix(root_name)
                .and_then(|path| path.strip_prefix('/'))
                .unwrap_or("");
            relative.to_owned()
        })
        .collect::<BTreeSet<_>>();
    let mut body = catalog.value(source_path_id)?;
    let mut mapped_paths = BTreeSet::new();
    let mut dropped_paths = BTreeSet::new();
    for field in [
        "m_PositionCurves",
        "m_RotationCurves",
        "m_CompressedRotationCurves",
        "m_ScaleCurves",
    ] {
        let Some(value) = body.get_mut(field) else {
            continue;
        };
        let Some(curves) = value.as_array_mut() else {
            return rig_error(format!(
                "custom clip {source_name}#{source_path_id} {field} is not an array"
            ));
        };
        let mut retained = Vec::with_capacity(curves.len());
        for curve in std::mem::take(curves) {
            let path = curve_path(&curve)?;
            if paths.contains(&path) {
                mapped_paths.insert(path);
                retained.push(curve);
            } else {
                dropped_paths.insert(path);
            }
        }
        *curves = retained;
    }
    if mapped_paths.len() < minimum_mapped_paths {
        return rig_error(format!(
            "custom clip {source_name}#{source_path_id} maps only {} unique paths to {root_name}; expected at least {minimum_mapped_paths}; dropped={:?}",
            mapped_paths.len(),
            dropped_paths
        ));
    }
    if dropped_paths
        .iter()
        .any(|path| path.is_empty() || path == "Bip01")
    {
        return rig_error(format!(
            "custom clip {source_name}#{source_path_id} attempted to drop a required root path"
        ));
    }

    let value = serde_json::value::to_raw_value(&body).map_err(|source| PipelineError::Json {
        path: format!("custom-animation#{source_path_id}"),
        source,
    })?;
    let temporary = DumpCatalog {
        objects: vec![DumpObject {
            name: semantic_name.to_owned(),
            path_id: source_path_id,
            object_type: "AnimationClip".to_owned(),
            value,
        }],
        by_path_id: HashMap::from([(source_path_id, 0)]),
    };
    decode_clip(&temporary, nodes, root_name, semantic_name, source_path_id)
}

pub(super) fn decode_clip(
    catalog: &DumpCatalog,
    nodes: &[PlayerRigNode],
    root_name: &str,
    expected_name: &str,
    path_id: i64,
) -> Result<AnimationClip> {
    let object = catalog.get(path_id)?;
    if object.object_type != "AnimationClip" || object.name != expected_name {
        return rig_error(format!(
            "clip {expected_name}#{path_id} identity is {} {:?}",
            object.object_type, object.name
        ));
    }
    let body = catalog.value(path_id)?;
    let paths = nodes
        .iter()
        .map(|node| {
            let relative = node
                .full_path
                .strip_prefix(root_name)
                .and_then(|path| path.strip_prefix('/'))
                .unwrap_or("");
            (relative.to_owned(), node.actor_bone_index)
        })
        .collect::<BTreeMap<_, _>>();

    let mut channels = Vec::new();
    decode_vec3_curves(
        &body,
        "m_PositionCurves",
        Vec3Track::Translation,
        &paths,
        &mut channels,
    )?;
    decode_plain_rotation_curves(&body, &paths, &mut channels)?;
    decode_compressed_rotation_curves(&body, &paths, &mut channels)?;
    decode_vec3_curves(
        &body,
        "m_ScaleCurves",
        Vec3Track::Scale,
        &paths,
        &mut channels,
    )?;
    let keyed_duration = channels
        .iter()
        .filter_map(|channel| channel.times.last().copied())
        .reduce(f64::max);
    let events = decode_events(&body)?;
    let event_duration = events.iter().map(|event| event.time).reduce(f64::max);
    let duration = keyed_duration
        .into_iter()
        .chain(event_duration)
        .reduce(f64::max)
        .unwrap_or(0.0);
    let sample_rate = body
        .get("m_SampleRate")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0);
    if sample_rate.is_none() {
        return rig_error(format!(
            "{expected_name}#{path_id} has no valid sample rate"
        ));
    }
    Ok(AnimationClip {
        name: expected_name.to_owned(),
        duration,
        declared_duration: None,
        keyed_duration,
        event_duration,
        sample_rate,
        wrap_mode: None,
        looped: runtime_clip_loops(expected_name),
        channels,
        metadata: AnimationMetadata {
            events,
            ..AnimationMetadata::default()
        },
    })
}

pub(super) fn decode_vec3_curves(
    body: &Value,
    field: &str,
    kind: Vec3Track,
    paths: &BTreeMap<String, u32>,
    output: &mut Vec<AnimationChannel>,
) -> Result<()> {
    for (source_index, binding) in optional_array(body, field).iter().enumerate() {
        let path = curve_path(binding)?;
        let target_node = unique_target(paths, &path)?;
        let keys = binding
            .pointer("/curve/m_Curve")
            .and_then(Value::as_array)
            .ok_or_else(|| rig_message(format!("{field}[{source_index}] has no key array")))?;
        if keys.is_empty() {
            return rig_error(format!("{field}[{source_index}] is empty"));
        }
        let mut times = Vec::with_capacity(keys.len());
        let mut values = Vec::with_capacity(keys.len());
        let mut inputs = Vec::with_capacity(keys.len());
        let mut outputs = Vec::with_capacity(keys.len());
        for key in keys {
            let time = finite(key.get("time"), "key time")?;
            if times.last().is_some_and(|previous| *previous >= time) {
                return rig_error(format!("{field}[{source_index}] key times are not strict"));
            }
            let value = vec3(key.get("value"), "key value")?;
            let input = vec3(key.get("inSlope"), "key inSlope")?;
            let output = vec3(key.get("outSlope"), "key outSlope")?;
            times.push(time);
            match kind {
                Vec3Track::Translation => {
                    values.push(native_translation(value));
                    inputs.push(native_translation(input));
                    outputs.push(native_translation(output));
                }
                Vec3Track::Scale => {
                    values.push(value);
                    inputs.push(input);
                    outputs.push(output);
                }
            }
        }
        output.push(AnimationChannel {
            target_node,
            source_index: source_index as u32,
            source_encoding: EmptyTrsSourceEncoding::Plain,
            source_key_count: keys.len() as u32,
            source_key_indices: (0..keys.len() as u32).collect(),
            duplicate_keys: Vec::new(),
            interpolation: Interpolation::CubicSpline,
            times,
            values: match kind {
                Vec3Track::Translation => TrackValues::Translation(values),
                Vec3Track::Scale => TrackValues::Scale(values),
            },
            in_tangents: Some(match kind {
                Vec3Track::Translation => TrackValues::Translation(inputs),
                Vec3Track::Scale => TrackValues::Scale(inputs),
            }),
            out_tangents: Some(match kind {
                Vec3Track::Translation => TrackValues::Translation(outputs),
                Vec3Track::Scale => TrackValues::Scale(outputs),
            }),
            tangent_modes: Vec::new(),
        });
    }
    Ok(())
}

pub(super) fn decode_plain_rotation_curves(
    body: &Value,
    paths: &BTreeMap<String, u32>,
    output: &mut Vec<AnimationChannel>,
) -> Result<()> {
    for (source_index, binding) in optional_array(body, "m_RotationCurves").iter().enumerate() {
        let path = curve_path(binding)?;
        let target_node = unique_target(paths, &path)?;
        let keys = binding
            .pointer("/curve/m_Curve")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                rig_message(format!("m_RotationCurves[{source_index}] has no key array"))
            })?;
        if keys.is_empty() {
            return rig_error(format!("m_RotationCurves[{source_index}] is empty"));
        }
        let mut times = Vec::with_capacity(keys.len());
        let mut values = Vec::with_capacity(keys.len());
        let mut inputs = Vec::with_capacity(keys.len());
        let mut outputs = Vec::with_capacity(keys.len());
        for key in keys {
            let time = finite(key.get("time"), "rotation key time")?;
            if times.last().is_some_and(|previous| *previous >= time) {
                return rig_error(format!(
                    "m_RotationCurves[{source_index}] key times are not strict"
                ));
            }
            times.push(time);
            values.push(native_rotation(quat(
                key.get("value"),
                "rotation key value",
            )?)?);
            inputs.push(native_quat_tangent(quat(
                key.get("inSlope"),
                "rotation key inSlope",
            )?));
            outputs.push(native_quat_tangent(quat(
                key.get("outSlope"),
                "rotation key outSlope",
            )?));
        }
        output.push(AnimationChannel {
            target_node,
            source_index: source_index as u32,
            source_encoding: EmptyTrsSourceEncoding::Plain,
            source_key_count: keys.len() as u32,
            source_key_indices: (0..keys.len() as u32).collect(),
            duplicate_keys: Vec::new(),
            interpolation: Interpolation::CubicSpline,
            times,
            values: TrackValues::Rotation(values),
            in_tangents: Some(TrackValues::Rotation(inputs)),
            out_tangents: Some(TrackValues::Rotation(outputs)),
            tangent_modes: Vec::new(),
        });
    }
    Ok(())
}

pub(super) fn decode_compressed_rotation_curves(
    body: &Value,
    paths: &BTreeMap<String, u32>,
    output: &mut Vec<AnimationChannel>,
) -> Result<()> {
    for (source_index, binding) in optional_array(body, "m_CompressedRotationCurves")
        .iter()
        .enumerate()
    {
        let path = binding
            .get("m_Path")
            .or_else(|| binding.get("path"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                rig_message(format!(
                    "m_CompressedRotationCurves[{source_index}] has no path"
                ))
            })?;
        let target_node = unique_target(paths, path)?;
        let deltas = read_packed_bits(binding.get("m_Times").ok_or_else(|| {
            rig_message(format!(
                "m_CompressedRotationCurves[{source_index}] has no times"
            ))
        })?)?;
        let slopes = read_packed_floats(binding.get("m_Slopes").ok_or_else(|| {
            rig_message(format!(
                "m_CompressedRotationCurves[{source_index}] has no slopes"
            ))
        })?)?;
        let packed_values = binding.get("m_Values").ok_or_else(|| {
            rig_message(format!(
                "m_CompressedRotationCurves[{source_index}] has no values"
            ))
        })?;
        let count = usize_number(
            packed_values
                .get("m_NumItems")
                .ok_or_else(|| rig_message("packed quaternion count is absent"))?,
            "packed quaternion count",
        )?;
        let bytes = byte_payload(
            packed_values
                .get("m_Data")
                .ok_or_else(|| rig_message("packed quaternion data is absent"))?,
        )?;
        if count == 0
            || deltas.len() != count
            || bytes.len() != count * 4
            || slopes.len() != count * 4
        {
            return rig_error(format!(
                "m_CompressedRotationCurves[{source_index}] cardinality mismatch: count={count}, times={}, bytes={}, slopes={}",
                deltas.len(),
                bytes.len(),
                slopes.len()
            ));
        }
        let mut elapsed = 0_u64;
        let mut times = Vec::with_capacity(count);
        let mut values = Vec::with_capacity(count);
        let mut tangents = Vec::with_capacity(count);
        for (index, delta) in deltas.into_iter().enumerate() {
            elapsed = elapsed
                .checked_add(u64::from(delta))
                .ok_or_else(|| rig_message("packed animation time overflow"))?;
            let time = elapsed as f64 * 0.01;
            if times.last().is_some_and(|previous| *previous >= time) {
                return rig_error(format!(
                    "m_CompressedRotationCurves[{source_index}] decoded times are not strict"
                ));
            }
            times.push(time);
            let offset = index * 4;
            let packed = u32::from_le_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .expect("exact four-byte quaternion"),
            );
            values.push(native_rotation(unpack_legacy_compressed_quaternion(
                packed,
            ))?);
            let offset = index * 4;
            tangents.push(native_quat_tangent([
                slopes[offset],
                slopes[offset + 1],
                slopes[offset + 2],
                slopes[offset + 3],
            ]));
        }
        output.push(AnimationChannel {
            target_node,
            source_index: source_index as u32,
            source_encoding: EmptyTrsSourceEncoding::Compressed,
            source_key_count: count as u32,
            source_key_indices: (0..count as u32).collect(),
            duplicate_keys: Vec::new(),
            interpolation: Interpolation::CubicSpline,
            times,
            values: TrackValues::Rotation(values),
            // Unity's legacy compressed quaternion format stores one smooth
            // quaternion slope per key. The uncompressed rotation curves in
            // this exact client generation serialize that same slope as both
            // Keyframe.inSlope and Keyframe.outSlope. glTF CUBICSPLINE carries
            // those two arrays explicitly, so preserve the source derivative
            // on both sides rather than degrading the curve to LINEAR.
            in_tangents: Some(TrackValues::Rotation(tangents.clone())),
            out_tangents: Some(TrackValues::Rotation(tangents)),
            tangent_modes: Vec::new(),
        });
    }
    Ok(())
}

pub(super) fn decode_events(body: &Value) -> Result<Vec<AnimationEvent>> {
    optional_array(body, "m_Events")
        .iter()
        .map(|event| {
            let time = finite(
                event.get("time").or_else(|| event.get("m_Time")),
                "event time",
            )?;
            let function_name = event
                .get("functionName")
                .or_else(|| event.get("m_FunctionName"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            if function_name.is_empty() {
                return rig_error("animation event has no functionName");
            }
            let string_parameter = event
                .get("data")
                .or_else(|| event.get("stringParameter"))
                .or_else(|| event.get("m_StringParameter"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let float_parameter = event
                .get("floatParameter")
                .or_else(|| event.get("m_FloatParameter"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let int_parameter = event
                .get("intParameter")
                .or_else(|| event.get("m_IntParameter"))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let message_options = event
                .get("messageOptions")
                .or_else(|| event.get("m_MessageOptions"))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            Ok(AnimationEvent {
                time,
                function_name,
                string_parameter,
                float_parameter,
                float_parameter_provenance: None,
                int_parameter: i32::try_from(int_parameter)
                    .map_err(|_| rig_message("animation event intParameter exceeds i32"))?,
                object_parameter: None,
                object_parameter_provenance: AnimationEventObjectParameterProvenance::Missing {
                    interpretation: MissingEventObjectParameterInterpretation::Missing,
                },
                message_options: i32::try_from(message_options)
                    .map_err(|_| rig_message("animation event messageOptions exceeds i32"))?,
            })
        })
        .collect()
}

pub(super) fn byte_payload(value: &Value) -> Result<Vec<u8>> {
    let encoded = value
        .get("base64")
        .and_then(Value::as_str)
        .ok_or_else(|| rig_message("byte payload has no base64"))?;
    let decoded = BASE64
        .decode(encoded)
        .map_err(|error| rig_message(format!("invalid base64 byte payload: {error}")))?;
    let declared = usize_number(
        value
            .get("bytes")
            .ok_or_else(|| rig_message("byte payload has no byte count"))?,
        "byte payload count",
    )?;
    if decoded.len() != declared {
        return rig_error(format!(
            "byte payload decoded {} bytes, declared {declared}",
            decoded.len()
        ));
    }
    Ok(decoded)
}
