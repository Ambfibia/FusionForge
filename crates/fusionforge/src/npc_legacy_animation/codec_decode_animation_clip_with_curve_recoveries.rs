use super::*;

pub(super) fn decode_plain_trs_curve(
    field: &str,
    curve: &fusionforge::UnityValue,
) -> Result<(String, DecodedCurveSamples), String> {
    match field {
        "m_PositionCurves" => decode_plain_curve(
            curve,
            |value| {
                let value = unity_vec3(Some(value), [f64::NAN; 3]);
                value
                    .iter()
                    .all(|component| component.is_finite())
                    .then_some([-value[0], value[1], value[2]])
            },
            |value| {
                let value = unity_vec3(Some(value), [f64::NAN; 3]);
                value
                    .iter()
                    .all(|component| component.is_finite())
                    .then_some([-value[0], value[1], value[2]])
            },
        ),
        "m_RotationCurves" => decode_plain_curve(
            curve,
            |value| {
                let value = unity_quat(Some(value), [f64::NAN; 4]);
                value
                    .iter()
                    .all(|component| component.is_finite())
                    .then(|| normalize_quat([value[0], -value[1], -value[2], value[3]]))
            },
            |value| {
                let value = unity_quat(Some(value), [f64::NAN; 4]);
                value
                    .iter()
                    .all(|component| component.is_finite())
                    .then_some([value[0], -value[1], -value[2], value[3]])
            },
        ),
        "m_ScaleCurves" => decode_plain_curve(
            curve,
            |value| {
                let value = unity_vec3(Some(value), [f64::NAN; 3]);
                value
                    .iter()
                    .all(|component| component.is_finite())
                    .then_some(value)
            },
            |value| {
                let value = unity_vec3(Some(value), [f64::NAN; 3]);
                value
                    .iter()
                    .all(|component| component.is_finite())
                    .then_some(value)
            },
        ),
        _ => Err(format!("unsupported plain TRS field {field:?}")),
    }
}

pub(super) fn constant_curve_payload(samples: &DecodedCurveSamples) -> Option<JsonValue> {
    if samples.keys.is_empty() || !samples.duplicate_keys.is_empty() {
        return None;
    }
    let payloads = samples
        .keys
        .iter()
        .cloned()
        .map(|mut key| {
            if !constant_key_has_zero_tangents(&key) {
                return None;
            }
            let object = key.as_object_mut()?;
            object.remove("time");
            object.remove("sourceKeyIndex");
            Some(key)
        })
        .collect::<Option<Vec<_>>>()?;
    let first = payloads.first()?.clone();
    payloads
        .iter()
        .all(|payload| payload == &first)
        .then_some(first)
}

pub(super) fn decode_animation_clip(
    asset_name: &str,
    path_id: i64,
    body: &fusionforge::UnityValue,
    time_recoveries: Vec<JsonValue>,
) -> DecodedClip {
    decode_animation_clip_with_curve_recoveries(asset_name, path_id, body, time_recoveries, &[])
}

pub(super) fn decode_animation_clip_with_curve_recoveries(
    asset_name: &str,
    path_id: i64,
    body: &fusionforge::UnityValue,
    time_recoveries: Vec<JsonValue>,
    curve_recovery_plans: &[CurveRecoveryPlan],
) -> DecodedClip {
    let rotation_curves = curve_count(body, "m_RotationCurves");
    let compressed_rotation_curves = curve_count(body, "m_CompressedRotationCurves");
    let euler_curves = curve_count(body, "m_EulerCurves");
    let position_curves = curve_count(body, "m_PositionCurves");
    let scale_curves = curve_count(body, "m_ScaleCurves");
    let float_curves = curve_count(body, "m_FloatCurves");
    let pptr_curves = curve_count(body, "m_PPtrCurves");
    let event_count = curve_count(body, "m_Events");
    let clip_name = fusionforge::object_name(body);

    // Preserve serialized source order. Unity may keep legitimate zero-key
    // bindings; they are source metadata, not fake glTF animation channels.
    let mut translations = DecodedCurveTracks::new();
    let mut rotations = DecodedCurveTracks::new();
    let mut scales = DecodedCurveTracks::new();
    let mut empty_trs_bindings = Vec::<JsonValue>::new();
    let mut duplicate_trs_bindings = Vec::<JsonValue>::new();
    let mut decode_errors = Vec::new();
    let events =
        decode_animation_events_for_clip(asset_name, path_id, &clip_name, body, &mut decode_errors);
    let decoded_float_curves = decode_float_curves(body, &mut decode_errors);

    decode_vec3_curves(
        body,
        "m_PositionCurves",
        Vec3CurveKind::Translation,
        &mut translations,
        &mut empty_trs_bindings,
        &mut duplicate_trs_bindings,
        curve_recovery_plans,
        &mut decode_errors,
    );
    decode_quaternion_curves(
        body,
        "m_RotationCurves",
        &mut rotations,
        &mut empty_trs_bindings,
        &mut duplicate_trs_bindings,
        curve_recovery_plans,
        &mut decode_errors,
    );
    decode_compressed_quaternion_curves(
        body,
        &mut rotations,
        &mut empty_trs_bindings,
        &mut duplicate_trs_bindings,
        curve_recovery_plans,
        &mut decode_errors,
    );
    decode_vec3_curves(
        body,
        "m_ScaleCurves",
        Vec3CurveKind::Scale,
        &mut scales,
        &mut empty_trs_bindings,
        &mut duplicate_trs_bindings,
        curve_recovery_plans,
        &mut decode_errors,
    );

    // Pre-Mecanim AnimationClip curves are serialized in application order.
    // When the same path/property occurs more than once, the later binding is
    // the effective legacy value. Preserve every displaced complete curve as
    // typed metadata, then bind that evidence to this exact source clip.
    translations.sort_by_key(decoded_curve_track_source_order);
    rotations.sort_by_key(decoded_curve_track_source_order);
    scales.sort_by_key(decoded_curve_track_source_order);
    for binding in &mut duplicate_trs_bindings {
        let kind = binding.get("kind").and_then(JsonValue::as_str);
        let path = binding.get("path").and_then(JsonValue::as_str);
        let canonical_tracks = match kind {
            Some("translation") => Some(&translations),
            Some("rotation") => Some(&rotations),
            Some("scale") => Some(&scales),
            _ => None,
        };
        if let (Some(path), Some(canonical_tracks)) = (path, canonical_tracks) {
            if let Some(canonical_track_index) =
                canonical_tracks.iter().position(|track| track.path == path)
            {
                binding
                    .as_object_mut()
                    .expect("duplicate TRS binding object")
                    .insert(
                        "canonicalTrackIndex".to_string(),
                        json!(canonical_track_index),
                    );
            }
        }
    }
    duplicate_trs_bindings.sort_by_key(duplicate_trs_binding_source_order);
    for binding in &mut duplicate_trs_bindings {
        if binding.get("relation").and_then(JsonValue::as_str) == Some("serializedLastWriteWins") {
            binding
                .as_object_mut()
                .expect("duplicate TRS binding object")
                .insert(
                    "resolutionProof".to_string(),
                    json!({
                        "rule": "later-serialized-binding-overwrites-earlier",
                        "asset": asset_name,
                        "pathId": path_id,
                        "clipName": clip_name,
                        "exactTargetPath": true,
                        "exactSourceArrayOrder": true,
                    }),
                );
        }
    }

    let keyed_duration = translations
        .iter()
        .chain(rotations.iter())
        .chain(scales.iter())
        .flat_map(|track| &track.keys)
        .chain(
            decoded_float_curves
                .iter()
                .flat_map(|track| track.get("keys").and_then(JsonValue::as_array))
                .flatten(),
        )
        .chain(
            curve_recovery_plans
                .iter()
                .flat_map(|recovery| &recovery.rejected)
                .flat_map(|(_, samples)| &samples.keys),
        )
        .filter_map(|key| key.get("time").and_then(JsonValue::as_f64))
        .fold(None::<f64>, |maximum, time| {
            Some(maximum.map_or(time, |current| current.max(time)))
        });
    let event_duration = events
        .iter()
        .filter_map(|event| event.get("time").and_then(JsonValue::as_f64))
        .fold(None::<f64>, |maximum, time| {
            Some(maximum.map_or(time, |current| current.max(time)))
        });
    let declared_duration = declared_clip_duration(body);
    let duration = [declared_duration, keyed_duration, event_duration]
        .into_iter()
        .flatten()
        .fold(0.0_f64, f64::max);
    let has_track_data = !(translations.is_empty() && rotations.is_empty() && scales.is_empty());
    let has_pose_curves = rotation_curves
        + compressed_rotation_curves
        + euler_curves
        + position_curves
        + scale_curves
        > 0;
    let sample_rate = clip_sample_rate(body);
    let wrap_mode = body
        .get("m_WrapMode")
        .and_then(fusionforge::UnityValue::as_i64);
    let looped = wrap_mode.map(|mode| mode == 2 || mode == 4).unwrap_or(true);
    let curve_recoveries = match curve_recovery_plans_to_json(
        curve_recovery_plans,
        &translations,
        &rotations,
        &scales,
    ) {
        Ok(recoveries) => recoveries,
        Err(error) => {
            decode_errors.push(error);
            Vec::new()
        }
    };

    let mut preview = json!({
        "source": "unity-animation-clip",
        "asset": asset_name,
        "pathId": path_id,
        "name": clip_name,
        "duration": duration,
        "declaredDuration": declared_duration,
        "keyedDuration": keyed_duration,
        "eventDuration": event_duration,
        "sampleRate": sample_rate,
        "wrapMode": wrap_mode,
        "loop": looped,
        "curveCounts": {
            "rotation": rotation_curves,
            "compressedRotation": compressed_rotation_curves,
            "euler": euler_curves,
            "position": position_curves,
            "scale": scale_curves,
            "float": float_curves,
            "pptr": pptr_curves,
            "events": event_count,
        },
        "hasPoseCurves": has_pose_curves,
        "canPreviewPose": has_track_data,
        "previewSupport": if has_track_data { "unity-legacy-trs" } else { "metadata-only" },
        "events": events,
    });
    if has_track_data
        || !empty_trs_bindings.is_empty()
        || !duplicate_trs_bindings.is_empty()
        || !time_recoveries.is_empty()
        || !curve_recoveries.is_empty()
        || !decoded_float_curves.is_empty()
    {
        preview
            .as_object_mut()
            .expect("clip preview object")
            .insert(
                "animationData".to_string(),
                json!({
                    "translations": curve_tracks_to_json(translations),
                    "rotations": curve_tracks_to_json(rotations),
                    "scales": curve_tracks_to_json(scales),
                    "floatCurves": decoded_float_curves,
                    "emptyTrsBindings": empty_trs_bindings,
                    "duplicateTrsBindings": duplicate_trs_bindings,
                    "timeRecoveries": time_recoveries,
                    "curveRecoveries": curve_recoveries,
                }),
            );
    } else {
        let reason = if !has_pose_curves {
            "AnimationClip has no legacy TRS pose curves.".to_string()
        } else if decode_errors.is_empty() {
            "AnimationClip pose curves contain no finite keys.".to_string()
        } else {
            format!(
                "Legacy pose curves could not be decoded: {}",
                decode_errors.join("; ")
            )
        };
        preview
            .as_object_mut()
            .expect("clip preview object")
            .insert("previewUnsupportedReason".to_string(), json!(reason));
    }
    if !decode_errors.is_empty() {
        preview
            .as_object_mut()
            .expect("clip preview object")
            .insert("decodeWarnings".to_string(), json!(decode_errors));
    }

    let track_roots = preview
        .pointer("/animationData/translations")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .chain(
            preview
                .pointer("/animationData/rotations")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten(),
        )
        .chain(
            preview
                .pointer("/animationData/scales")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten(),
        )
        .chain(
            preview
                .pointer("/animationData/floatCurves")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten(),
        )
        .filter_map(|curve| curve.get("path").and_then(JsonValue::as_str))
        .filter_map(|path| normalized_path(path).split('/').next().map(str::to_string))
        .filter(|path| !path.is_empty())
        .collect::<BTreeSet<_>>();
    let warnings = preview
        .get("decodeWarnings")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .map(|warning| format!("{asset_name}#{path_id} ({clip_name}): {warning}"))
        .collect();

    DecodedClip {
        preview,
        track_roots,
        warnings,
    }
}

pub(super) fn decode_vec3_curves(
    body: &fusionforge::UnityValue,
    field: &str,
    kind: Vec3CurveKind,
    output: &mut DecodedCurveTracks,
    empty_trs_bindings: &mut Vec<JsonValue>,
    duplicate_trs_bindings: &mut Vec<JsonValue>,
    curve_recovery_plans: &[CurveRecoveryPlan],
    errors: &mut Vec<String>,
) {
    for (source_index, curve) in fusionforge::value_array(body.get(field)).iter().enumerate() {
        match decode_plain_curve(
            curve,
            |value| {
                let value = unity_vec3(Some(value), [f64::NAN; 3]);
                value
                    .iter()
                    .all(|component| component.is_finite())
                    .then(|| match kind {
                        Vec3CurveKind::Translation => [-value[0], value[1], value[2]],
                        Vec3CurveKind::Scale => value,
                    })
            },
            |value| {
                let value = unity_vec3(Some(value), [f64::NAN; 3]);
                value
                    .iter()
                    .all(|component| component.is_finite())
                    .then(|| match kind {
                        Vec3CurveKind::Translation => [-value[0], value[1], value[2]],
                        Vec3CurveKind::Scale => value,
                    })
            },
        ) {
            Ok((path, keys)) => keep_decoded_curve(
                output,
                empty_trs_bindings,
                duplicate_trs_bindings,
                curve_recovery_plans,
                errors,
                kind.source_kind(),
                "plain",
                source_index,
                path,
                keys,
            ),
            Err(error) => errors.push(format!("{field}[{source_index}]: {error}")),
        }
    }
}

pub(super) fn decode_quaternion_curves(
    body: &fusionforge::UnityValue,
    field: &str,
    output: &mut DecodedCurveTracks,
    empty_trs_bindings: &mut Vec<JsonValue>,
    duplicate_trs_bindings: &mut Vec<JsonValue>,
    curve_recovery_plans: &[CurveRecoveryPlan],
    errors: &mut Vec<String>,
) {
    for (source_index, curve) in fusionforge::value_array(body.get(field)).iter().enumerate() {
        match decode_plain_curve(
            curve,
            |value| {
                let value = unity_quat(Some(value), [f64::NAN; 4]);
                value
                    .iter()
                    .all(|component| component.is_finite())
                    .then(|| normalize_quat([value[0], -value[1], -value[2], value[3]]))
            },
            |value| {
                let value = unity_quat(Some(value), [f64::NAN; 4]);
                value
                    .iter()
                    .all(|component| component.is_finite())
                    .then(|| [value[0], -value[1], -value[2], value[3]])
            },
        ) {
            Ok((path, keys)) => keep_decoded_curve(
                output,
                empty_trs_bindings,
                duplicate_trs_bindings,
                curve_recovery_plans,
                errors,
                "rotation",
                "plain",
                source_index,
                path,
                keys,
            ),
            Err(error) => errors.push(format!("{field}[{source_index}]: {error}")),
        }
    }
}

pub(super) fn decode_float_curves(body: &fusionforge::UnityValue, errors: &mut Vec<String>) -> Vec<JsonValue> {
    fusionforge::value_array(body.get("m_FloatCurves"))
        .iter()
        .enumerate()
        .filter_map(|(source_index, curve)| {
            let decoded = (|| -> Result<JsonValue, String> {
                let (path, samples) = decode_plain_curve(
                    curve,
                    |value| {
                        value
                            .as_f64()
                            .filter(|value| value.is_finite() && (*value as f32).is_finite())
                    },
                    |value| {
                        value
                            .as_f64()
                            .filter(|value| value.is_finite() && (*value as f32).is_finite())
                    },
                )?;
                let property = curve
                    .get("attribute")
                    .or_else(|| curve.get("m_Attribute"))
                    .and_then(fusionforge::UnityValue::as_str)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| format!("{path}: float curve attribute is empty"))?;
                let class_id = curve
                    .get("classID")
                    .or_else(|| curve.get("m_ClassID"))
                    .and_then(fusionforge::UnityValue::as_i64)
                    .ok_or_else(|| format!("{path}: float curve classID is absent"))?;
                let script = curve
                    .get("script")
                    .or_else(|| curve.get("m_Script"))
                    .ok_or_else(|| format!("{path}: float curve script PPtr is absent"))?;
                let (script_file_id, script_path_id) = match script {
                    // A null script binding is meaningful source metadata for
                    // material curves. `UnityValue::as_pointer()` intentionally
                    // hides null PPtrs, so inspect this variant directly.
                    fusionforge::UnityValue::Pointer(pointer) => {
                        (i64::from(pointer.file_id), pointer.path_id)
                    }
                    _ => {
                        let file_id = script
                            .get("fileId")
                            .or_else(|| script.get("m_FileID"))
                            .and_then(fusionforge::UnityValue::as_i64)
                            .ok_or_else(|| {
                                format!("{path}: float curve script fileId is absent")
                            })?;
                        let path_id = script
                            .get("pathId")
                            .or_else(|| script.get("m_PathID"))
                            .and_then(fusionforge::UnityValue::as_i64)
                            .ok_or_else(|| {
                                format!("{path}: float curve script pathId is absent")
                            })?;
                        (file_id, path_id)
                    }
                };
                let curve_data = curve.get("curve").unwrap_or(curve);
                let pre_infinity = curve_data
                    .get("m_PreInfinity")
                    .and_then(fusionforge::UnityValue::as_i64)
                    .ok_or_else(|| format!("{path}: float curve m_PreInfinity is absent"))?;
                let post_infinity = curve_data
                    .get("m_PostInfinity")
                    .and_then(fusionforge::UnityValue::as_i64)
                    .ok_or_else(|| format!("{path}: float curve m_PostInfinity is absent"))?;
                let interpolation =
                    if samples.keys.iter().all(|key| {
                        key.get("inTangent").is_some() && key.get("outTangent").is_some()
                    }) {
                        "CUBICSPLINE"
                    } else {
                        "LINEAR"
                    };
                Ok(json!({
                    "path": path,
                    "property": property,
                    "classId": class_id,
                    "script": {
                        "fileId": script_file_id,
                        "pathId": script_path_id,
                    },
                    "preInfinity": pre_infinity,
                    "postInfinity": post_infinity,
                    "interpolation": interpolation,
                    "keys": samples.keys,
                    "duplicateKeys": samples.duplicate_keys,
                    "sourceKeyCount": samples.source_key_count,
                    "sourceIndex": source_index,
                    "sourceEncoding": "plain",
                }))
            })();
            match decoded {
                Ok(value) => Some(value),
                Err(error) => {
                    errors.push(format!("m_FloatCurves[{source_index}]: {error}"));
                    None
                }
            }
        })
        .collect()
}

pub(super) fn decode_plain_curve<T, F, G>(
    curve: &fusionforge::UnityValue,
    convert_value: F,
    convert_tangent: G,
) -> Result<(String, DecodedCurveSamples), String>
where
    T: serde::Serialize,
    F: Fn(&fusionforge::UnityValue) -> Option<T>,
    G: Fn(&fusionforge::UnityValue) -> Option<T>,
{
    let path = curve
        .get("path")
        .or_else(|| curve.get("m_Path"))
        .and_then(fusionforge::UnityValue::as_str)
        .map(normalized_path)
        .ok_or_else(|| "curve path is absent or invalid".to_string())?;
    let keys = curve
        .get("curve")
        .and_then(|value| value.get("m_Curve"))
        .or_else(|| curve.get("m_Curve"))
        .and_then(fusionforge::UnityValue::as_array)
        .ok_or_else(|| format!("{path}: m_Curve is absent"))?;
    let decoded = keys
        .iter()
        .enumerate()
        .map(|(key_index, key)| {
            let time = key
                .get("time")
                .and_then(fusionforge::UnityValue::as_f64)
                .filter(|time| time.is_finite())
                .ok_or_else(|| format!("{path}: key[{key_index}] has no finite time"))?;
            let value = key
                .get("value")
                .and_then(&convert_value)
                .ok_or_else(|| format!("{path}: key[{key_index}] has no finite value"))?;
            let mut decoded = json!({ "time": time, "value": value });
            if let Some(object) = decoded.as_object_mut() {
                if let Some(raw) = key.get("inSlope") {
                    let tangent = convert_tangent(raw)
                        .ok_or_else(|| format!("{path}: key[{key_index}] has no finite inSlope"))?;
                    object.insert("inTangent".to_string(), json!(tangent));
                }
                if let Some(raw) = key.get("outSlope") {
                    let tangent = convert_tangent(raw).ok_or_else(|| {
                        format!("{path}: key[{key_index}] has no finite outSlope")
                    })?;
                    object.insert("outTangent".to_string(), json!(tangent));
                }
                if let Some(raw) = key.get("tangentMode") {
                    let mode = raw.as_i64().ok_or_else(|| {
                        format!("{path}: key[{key_index}] has invalid tangentMode")
                    })?;
                    object.insert("tangentMode".to_string(), json!(mode));
                }
            }
            Ok(decoded)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((path.clone(), canonicalize_curve_keys(&path, decoded)?))
}

pub(super) fn decode_animation_events(
    body: &fusionforge::UnityValue,
    errors: &mut Vec<String>,
) -> Vec<JsonValue> {
    decode_animation_events_for_clip("", 0, "", body, errors)
}

pub(super) fn decode_animation_events_for_clip(
    asset_name: &str,
    path_id: i64,
    clip_name: &str,
    body: &fusionforge::UnityValue,
    errors: &mut Vec<String>,
) -> Vec<JsonValue> {
    fusionforge::value_array(body.get("m_Events"))
        .iter()
        .enumerate()
        .filter_map(|(event_index, event)| {
            let decoded = (|| -> Result<JsonValue, String> {
                let time = event_field(event, &["time", "m_Time"])
                    .and_then(fusionforge::UnityValue::as_f64)
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .ok_or_else(|| "has no finite non-negative time".to_string())?;
                let function_name = optional_event_string(
                    event,
                    &["functionName", "m_FunctionName"],
                    "functionName",
                )?;
                let string_parameter = optional_event_string(
                    event,
                    &["data", "stringParameter", "m_StringParameter"],
                    "stringParameter",
                )?;
                let (float_parameter, float_parameter_provenance) = match event_field(
                    event,
                    &["floatParameter", "m_FloatParameter"],
                ) {
                    None => (0.0, None),
                    Some(value) => {
                        let value = value
                            .as_f64()
                            .ok_or_else(|| "has invalid floatParameter".to_string())?;
                        if value.is_finite() {
                            (value, None)
                        } else if is_exact_larry_walk_end_event_with_stale_float(
                            asset_name,
                            path_id,
                            clip_name,
                            event_index,
                            event,
                            time,
                            &function_name,
                            &string_parameter,
                            value,
                        ) {
                            (
                                0.0,
                                Some(json!({
                                    "sourceAssetFormat": 6,
                                    "rawFloat32Bits": "0xffffff00",
                                    "interpretation": "legacy-unused-nonfinite-normalized-to-zero",
                                })),
                            )
                        } else {
                            return Err("has invalid floatParameter".to_string());
                        }
                    }
                };
                let int_parameter =
                    optional_event_i64(event, &["intParameter", "m_IntParameter"], "intParameter")?;
                let message_options = optional_event_i64(
                    event,
                    &["messageOptions", "m_MessageOptions"],
                    "messageOptions",
                )?;
                let (object_parameter, object_parameter_provenance) =
                    decode_event_object_parameter(event_field(
                        event,
                        &["objectReferenceParameter", "m_ObjectReferenceParameter"],
                    ))?;
                let mut decoded = json!({
                    "time": time,
                    "functionName": function_name,
                    "stringParameter": string_parameter,
                    "floatParameter": float_parameter,
                    "intParameter": int_parameter,
                    "objectParameter": object_parameter,
                    "objectParameterProvenance": object_parameter_provenance,
                    "messageOptions": message_options,
                });
                if let Some(provenance) = float_parameter_provenance {
                    decoded
                        .as_object_mut()
                        .expect("decoded event object")
                        .insert("floatParameterProvenance".to_string(), provenance);
                }
                Ok(decoded)
            })();
            match decoded {
                Ok(value) => Some(value),
                Err(error) => {
                    errors.push(format!("m_Events[{event_index}] {error}"));
                    None
                }
            }
        })
        .collect()
}
