use super::*;

pub(super) fn exact_curve_count(animation: &JsonValue, field: &str, context: &str) -> Result<usize, String> {
    let value = animation
        .get("curveCounts")
        .and_then(|counts| counts.get(field))
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| format!("{context} has no integer curveCounts.{field}"))?;
    usize::try_from(value).map_err(|_| format!("{context} curveCounts.{field} does not fit usize"))
}

pub(super) fn exact_required_duration(
    animation: &JsonValue,
    field: &str,
    context: &str,
) -> Result<f64, String> {
    animation
        .get(field)
        .and_then(JsonValue::as_f64)
        .filter(|value| value.is_finite() && (*value as f32).is_finite() && *value >= 0.0)
        .ok_or_else(|| format!("{context} has invalid {field}"))
}

pub(super) fn exact_optional_duration(
    animation: &JsonValue,
    field: &str,
    context: &str,
) -> Result<Option<f64>, String> {
    let value = animation
        .get(field)
        .ok_or_else(|| format!("{context} has no typed {field}"))?;
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_f64()
        .filter(|value| value.is_finite() && (*value as f32).is_finite() && *value >= 0.0)
        .map(Some)
        .ok_or_else(|| format!("{context} has invalid {field}"))
}

pub(super) fn exact_duration_matches(
    source: Option<f64>,
    decoded: Option<f64>,
    field: &str,
    context: &str,
) -> Result<(), String> {
    if source
        .zip(decoded)
        .is_some_and(|(left, right)| durations_equal(left, right))
        || source.is_none() && decoded.is_none()
    {
        return Ok(());
    }
    Err(format!(
        "{context} typed {field} {source:?} != decoded maximum {decoded:?}"
    ))
}

pub(super) fn durations_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1.0e-12 * left.abs().max(right.abs()).max(1.0)
}

pub(super) fn raw_trs_source(
    kind: &str,
    encoding: &str,
    position: usize,
    rotation: usize,
    compressed_rotation: usize,
    scale: usize,
) -> Option<(u8, usize)> {
    match (kind, encoding) {
        ("translation", "plain") => Some((0, position)),
        ("rotation", "plain") => Some((1, rotation)),
        ("rotation", "compressed") => Some((2, compressed_rotation)),
        ("scale", "plain") => Some((3, scale)),
        _ => None,
    }
}

pub(super) fn curve_recovery_rejected_count(recoveries: &[JsonValue], kind: &str) -> usize {
    recoveries
        .iter()
        .filter(|recovery| recovery.get("kind").and_then(JsonValue::as_str) == Some(kind))
        .filter_map(|recovery| recovery.get("rejected").and_then(JsonValue::as_array))
        .map(Vec::len)
        .sum()
}

pub(super) fn curve_recovery_rejected_keyed_duration(recoveries: &[JsonValue]) -> Option<f64> {
    recoveries
        .iter()
        .filter_map(|recovery| recovery.get("rejected").and_then(JsonValue::as_array))
        .flatten()
        .filter_map(|track| track.get("keys").and_then(JsonValue::as_array))
        .flatten()
        .filter_map(|key| key.get("time").and_then(JsonValue::as_f64))
        .fold(None::<f64>, |maximum, time| {
            Some(maximum.map_or(time, |current| current.max(time)))
        })
}

pub(super) fn raw_target_binding_count(animation: &JsonValue, kind: &str, path: &str) -> usize {
    let data = animation.get("animationData");
    let sampleable = animation_tracks(animation, kind)
        .iter()
        .filter(|track| track.get("path").and_then(JsonValue::as_str) == Some(path))
        .count();
    let metadata_count = |field: &str| {
        data.and_then(|data| data.get(field))
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter(|binding| {
                binding.get("kind").and_then(JsonValue::as_str) == Some(kind)
                    && binding.get("path").and_then(JsonValue::as_str) == Some(path)
            })
            .count()
    };
    let recovered_rejected = data
        .and_then(|data| data.get("curveRecoveries"))
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter(|recovery| {
            recovery.get("kind").and_then(JsonValue::as_str) == Some(kind)
                && recovery.get("path").and_then(JsonValue::as_str) == Some(path)
        })
        .filter_map(|recovery| recovery.get("rejected").and_then(JsonValue::as_array))
        .map(Vec::len)
        .sum::<usize>();
    sampleable
        + metadata_count("emptyTrsBindings")
        + metadata_count("duplicateTrsBindings")
        + recovered_rejected
}

pub(super) fn exact_time_array(value: Option<&JsonValue>, context: &str) -> Result<Vec<f64>, String> {
    value
        .and_then(JsonValue::as_array)
        .ok_or_else(|| format!("{context} is not an array"))?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_f64()
                .filter(|value| value.is_finite() && *value >= 0.0)
                .ok_or_else(|| format!("{context}[{index}] is not finite/non-negative"))
        })
        .collect()
}

pub(super) fn exact_nullable_f64(value: Option<&JsonValue>, context: &str) -> Result<Option<f64>, String> {
    let value = value.ok_or_else(|| format!("{context} is absent"))?;
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_f64()
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(Some)
        .ok_or_else(|| format!("{context} is invalid"))
}

pub(super) fn track_payloads_equal_excluding_time(left: &JsonValue, right: &JsonValue) -> bool {
    let payloads = |track: &JsonValue| {
        track
            .get("keys")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .cloned()
            .map(|mut key| {
                if let Some(object) = key.as_object_mut() {
                    object.remove("time");
                    object.remove("sourceKeyIndex");
                }
                key
            })
            .collect::<Vec<_>>()
    };
    payloads(left) == payloads(right)
}

pub(super) fn preview_local_matrix(node: &TransformNode) -> Matrix4 {
    compose_matrix(
        [
            -node.translation[0],
            node.translation[1],
            node.translation[2],
        ],
        [
            node.rotation[0],
            -node.rotation[1],
            -node.rotation[2],
            node.rotation[3],
        ],
        node.scale,
    )
}

pub(super) fn character_root(
    key: ObjectKey,
    transforms: &BTreeMap<ObjectKey, TransformNode>,
) -> Option<ObjectKey> {
    let mut current = Some(key);
    let mut last = None;
    let mut visited = BTreeSet::new();
    while let Some(key) = current {
        if !visited.insert(key) {
            return None;
        }
        let node = transforms.get(&key)?;
        last = Some(key);
        current = node.parent;
    }
    last
}

pub(super) fn compact_character_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn character_root_preference_score(
    candidate: &CharacterRootCandidate,
    preferred_paths: &BTreeSet<String>,
) -> usize {
    let names = candidate
        .searchable_names
        .iter()
        .map(|name| compact_character_name(name))
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    let searchable = names.join(" ");
    preferred_paths
        .iter()
        .map(|path| {
            let (preferred, tokens) = preferred_path_parts(path);
            if preferred.is_empty() {
                return 0;
            }
            if names.iter().any(|name| name == &preferred) {
                return 10_000;
            }
            if preferred.len() >= 4
                && names
                    .iter()
                    .any(|name| name.len() >= 4 && name.contains(&preferred))
            {
                return 1_000;
            }
            let matched = tokens
                .iter()
                .filter(|token| searchable.contains(token.as_str()))
                .count();
            matched * 25 + usize::from(!tokens.is_empty() && matched == tokens.len()) * 250
        })
        .max()
        .unwrap_or(0)
}

pub(super) fn select_preferred_character_roots(
    env: &fusionforge::UnityEnvironment,
    selected_meshes: &BTreeSet<ObjectKey>,
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    preferred_paths: &BTreeSet<String>,
    selected_objects: Option<&BTreeSet<ObjectKey>>,
) -> Option<BTreeSet<ObjectKey>> {
    let candidates =
        collect_character_root_candidates(env, selected_meshes, transforms, selected_objects);
    if candidates.len() <= 1 || preferred_paths.is_empty() {
        return None;
    }
    let mut best = None::<(ObjectKey, usize, usize)>;
    for (root, candidate) in &candidates {
        let score = character_root_preference_score(candidate, preferred_paths);
        let mesh_count = candidate.meshes.len();
        if best.is_none_or(|(best_root, best_score, best_mesh_count)| {
            score > best_score
                || (score == best_score && mesh_count > best_mesh_count)
                || (score == best_score && mesh_count == best_mesh_count && *root < best_root)
        }) {
            best = Some((*root, score, mesh_count));
        }
    }
    best.and_then(|(root, score, _)| (score > 0).then(|| BTreeSet::from([root])))
}

pub(super) fn meshes_for_character_roots(
    env: &fusionforge::UnityEnvironment,
    selected_meshes: &BTreeSet<ObjectKey>,
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    preferred_roots: Option<&BTreeSet<ObjectKey>>,
    selected_objects: Option<&BTreeSet<ObjectKey>>,
) -> BTreeSet<ObjectKey> {
    let Some(preferred_roots) = preferred_roots else {
        return selected_meshes.clone();
    };
    collect_character_root_candidates(env, selected_meshes, transforms, selected_objects)
        .into_iter()
        .filter(|(root, _)| preferred_roots.contains(root))
        .flat_map(|(_, candidate)| candidate.meshes)
        .collect()
}

pub(super) fn recover_non_strict_curve_times(
    clip: &RawAnimationClip,
    catalog: &[CurveTimeReference],
) -> (fusionforge::UnityValue, Vec<JsonValue>) {
    let mut recovered_body = clip.body.clone();
    let source_sample_rate = clip_sample_rate(&clip.body);
    let mut recoveries = Vec::new();
    for (field, kind) in [
        ("m_PositionCurves", "translation"),
        ("m_RotationCurves", "rotation"),
        ("m_ScaleCurves", "scale"),
    ] {
        for (source_index, curve) in fusionforge::value_array(clip.body.get(field))
            .iter()
            .enumerate()
        {
            let Some((path, original_times, payload_without_time)) = raw_plain_curve_parts(curve)
            else {
                continue;
            };
            if original_times.is_empty() || strictly_increasing_finite_times(&original_times) {
                continue;
            }
            let matches = catalog
                .iter()
                .filter(|reference| {
                    (reference.asset_name != clip.asset_name || reference.path_id != clip.path_id)
                        && reference.field == field
                        && reference.kind == kind
                        && reference.path == path
                        && reference.sample_rate == source_sample_rate
                        && reference.payload_without_time == payload_without_time
                })
                .collect::<Vec<_>>();
            let distinct_time_vectors = matches
                .iter()
                .map(|reference| {
                    reference
                        .times
                        .iter()
                        .map(|value| value.to_bits())
                        .collect::<Vec<_>>()
                })
                .collect::<BTreeSet<_>>();
            if distinct_time_vectors.len() != 1 {
                continue;
            }
            let Some(reference) = matches.first().copied() else {
                continue;
            };
            if !replace_plain_curve_times(
                &mut recovered_body,
                field,
                source_index,
                &reference.times,
            ) {
                continue;
            }
            recoveries.push(json!({
                "kind": kind,
                "path": path,
                "sourceEncoding": "plain",
                "sourceIndex": source_index,
                "reason": "non-strict-source-times-recovered-from-exact-sibling-curve",
                "originalTimes": original_times,
                "recoveredTimes": reference.times,
                "sourceSampleRate": source_sample_rate,
                "reference": {
                    "asset": reference.asset_name,
                    "pathId": reference.path_id,
                    "clipName": reference.clip_name,
                    "kind": reference.kind,
                    "path": reference.path,
                    "sourceEncoding": "plain",
                    "sourceIndex": reference.source_index,
                    "sampleRate": reference.sample_rate,
                },
                "proof": {
                    "exactKeyPayloadExcludingTime": true,
                    "exactPath": true,
                    "exactSampleRate": true,
                    "uniqueRecoveredTimeVectorCount": 1,
                    "matchingReferenceCount": matches.len(),
                },
            }));
        }
    }
    (recovered_body, recoveries)
}

pub(super) fn recover_conflicting_constant_curves(
    clip: &RawAnimationClip,
    catalog: &[ConstantCurveReference],
) -> Vec<CurveRecoveryPlan> {
    let source_sample_rate = clip_sample_rate(&clip.body);
    let mut recoveries = Vec::new();
    for field in ["m_PositionCurves", "m_RotationCurves", "m_ScaleCurves"] {
        let Some(kind) = plain_trs_kind(field) else {
            continue;
        };
        let curves = fusionforge::value_array(clip.body.get(field));
        let mut raw_path_counts = BTreeMap::<String, usize>::new();
        let mut groups = BTreeMap::<String, Vec<(usize, DecodedCurveSamples, JsonValue)>>::new();
        for (source_index, curve) in curves.iter().enumerate() {
            let Some(raw_path) = plain_curve_path(curve) else {
                continue;
            };
            *raw_path_counts.entry(raw_path).or_default() += 1;
            let Ok((path, samples)) = decode_plain_trs_curve(field, curve) else {
                continue;
            };
            let Some(constant_payload) = constant_curve_payload(&samples) else {
                continue;
            };
            groups
                .entry(path)
                .or_default()
                .push((source_index, samples, constant_payload));
        }
        for (path, candidates) in groups {
            if candidates.len() < 2 || raw_path_counts.get(&path) != Some(&candidates.len()) {
                continue;
            }
            if candidates
                .windows(2)
                .all(|pair| decoded_samples_equal(&pair[0].1, &pair[1].1))
            {
                continue;
            }
            let mut matching_pairs = Vec::new();
            for candidate in &candidates {
                for reference in catalog {
                    if (reference.asset_name != clip.asset_name
                        || reference.path_id != clip.path_id)
                        && reference.field == field
                        && reference.kind == kind
                        && reference.path == path
                        && optional_f64_bits_equal(reference.sample_rate, source_sample_rate)
                        && reference.constant_payload == candidate.2
                    {
                        matching_pairs.push((candidate, reference));
                    }
                }
            }
            let [(canonical, reference)] = matching_pairs.as_slice() else {
                continue;
            };
            let canonical_source_index = canonical.0;
            let rejected = candidates
                .iter()
                .filter(|candidate| candidate.0 != canonical_source_index)
                .map(|candidate| (candidate.0, candidate.1.clone()))
                .collect::<Vec<_>>();
            if rejected.is_empty() {
                continue;
            }
            recoveries.push(CurveRecoveryPlan {
                field,
                kind,
                path,
                source_indices: candidates.iter().map(|candidate| candidate.0).collect(),
                canonical_source_index,
                canonical_samples: canonical.1.clone(),
                rejected,
                source_sample_rate,
                reference: (*reference).clone(),
            });
        }
    }
    recoveries.sort_by_key(|recovery| {
        (
            match recovery.kind {
                "translation" => 0_u8,
                "rotation" => 1_u8,
                "scale" => 3_u8,
                _ => u8::MAX,
            },
            recovery
                .source_indices
                .first()
                .copied()
                .unwrap_or(usize::MAX),
        )
    });
    recoveries
}

pub(super) fn plain_trs_kind(field: &str) -> Option<&'static str> {
    match field {
        "m_PositionCurves" => Some("translation"),
        "m_RotationCurves" => Some("rotation"),
        "m_ScaleCurves" => Some("scale"),
        _ => None,
    }
}

pub(super) fn constant_key_has_zero_tangents(key: &JsonValue) -> bool {
    ["inTangent", "outTangent"]
        .into_iter()
        .filter_map(|field| key.get(field))
        .all(json_value_is_exact_zero)
}

pub(super) fn json_value_is_exact_zero(value: &JsonValue) -> bool {
    if let Some(number) = value.as_f64() {
        return number == 0.0;
    }
    if let Some(values) = value.as_array() {
        return values.iter().all(json_value_is_exact_zero);
    }
    if let Some(values) = value.as_object() {
        return values.values().all(json_value_is_exact_zero);
    }
    false
}

pub(super) fn decoded_samples_equal(left: &DecodedCurveSamples, right: &DecodedCurveSamples) -> bool {
    left.keys == right.keys
        && left.duplicate_keys == right.duplicate_keys
        && left.source_key_count == right.source_key_count
}

pub(super) fn optional_f64_bits_equal(left: Option<f64>, right: Option<f64>) -> bool {
    left.map(f64::to_bits) == right.map(f64::to_bits)
}

pub(super) fn raw_plain_curve_parts(
    curve: &fusionforge::UnityValue,
) -> Option<(String, Vec<f64>, Vec<fusionforge::UnityValue>)> {
    let path = curve
        .get("path")
        .or_else(|| curve.get("m_Path"))
        .and_then(fusionforge::UnityValue::as_str)
        .map(normalized_path)
        .filter(|value| !value.is_empty())?;
    let keys = curve
        .get("curve")
        .and_then(|value| value.get("m_Curve"))
        .or_else(|| curve.get("m_Curve"))
        .and_then(fusionforge::UnityValue::as_array)?;
    let mut times = Vec::with_capacity(keys.len());
    let mut payload_without_time = Vec::with_capacity(keys.len());
    for key in keys {
        times.push(key.get("time")?.as_f64()?);
        let mut payload = key.clone();
        payload.as_object_mut()?.remove("time");
        payload_without_time.push(payload);
    }
    Some((path, times, payload_without_time))
}

pub(super) fn strictly_increasing_finite_times(times: &[f64]) -> bool {
    times.iter().all(|value| value.is_finite()) && times.windows(2).all(|pair| pair[0] < pair[1])
}

pub(super) fn replace_plain_curve_times(
    body: &mut fusionforge::UnityValue,
    field: &str,
    source_index: usize,
    times: &[f64],
) -> bool {
    let Some(curve) = body
        .get_mut(field)
        .and_then(fusionforge::UnityValue::as_array_mut)
        .and_then(|curves| curves.get_mut(source_index))
    else {
        return false;
    };
    let keys = if curve.get("curve").is_some() {
        curve
            .get_mut("curve")
            .and_then(|value| value.get_mut("m_Curve"))
            .and_then(fusionforge::UnityValue::as_array_mut)
    } else {
        curve
            .get_mut("m_Curve")
            .and_then(fusionforge::UnityValue::as_array_mut)
    };
    let Some(keys) = keys else {
        return false;
    };
    if keys.len() != times.len() {
        return false;
    }
    for (key, time) in keys.iter_mut().zip(times.iter().copied()) {
        let Some(value) = key.get_mut("time") else {
            return false;
        };
        *value = fusionforge::UnityValue::Float(time);
    }
    true
}

pub(super) fn keep_decoded_curve(
    output: &mut DecodedCurveTracks,
    empty_trs_bindings: &mut Vec<JsonValue>,
    duplicate_trs_bindings: &mut Vec<JsonValue>,
    curve_recovery_plans: &[CurveRecoveryPlan],
    errors: &mut Vec<String>,
    kind: &str,
    source_encoding: &str,
    source_index: usize,
    path: String,
    samples: DecodedCurveSamples,
) {
    if samples.keys.is_empty() {
        empty_trs_bindings.push(json!({
            "kind": kind,
            "path": path,
            "sourceIndex": source_index,
            "sourceEncoding": source_encoding,
        }));
        return;
    }
    if let Some(recovery) = curve_recovery_plans.iter().find(|recovery| {
        recovery.kind == kind
            && recovery.path == path
            && source_encoding == "plain"
            && recovery.source_indices.contains(&source_index)
    }) {
        let expected = if source_index == recovery.canonical_source_index {
            Some(&recovery.canonical_samples)
        } else {
            recovery
                .rejected
                .iter()
                .find(|(candidate, _)| *candidate == source_index)
                .map(|(_, samples)| samples)
        };
        if expected.is_none_or(|expected| !decoded_samples_equal(expected, &samples)) {
            errors.push(format!(
                "{kind} {source_encoding} curve[{source_index}] no longer matches its exact sibling recovery plan for {path:?}"
            ));
            return;
        }
        if source_index != recovery.canonical_source_index {
            return;
        }
    }
    if let Some((canonical_track_index, previous)) = output
        .iter()
        .enumerate()
        .find(|(_, candidate)| candidate.path == path)
    {
        if previous.keys == samples.keys
            && previous.duplicate_keys == samples.duplicate_keys
            && previous.source_key_count == samples.source_key_count
        {
            duplicate_trs_bindings.push(json!({
                "kind": kind,
                "path": path,
                "sourceIndex": source_index,
                "sourceEncoding": source_encoding,
                "relation": "identical",
                "canonicalTrackIndex": canonical_track_index,
                "canonicalSourceIndex": previous.source_index,
                "canonicalSourceEncoding": previous.source_encoding,
                "keys": samples.keys,
                "duplicateKeys": samples.duplicate_keys,
                "sourceKeyCount": samples.source_key_count,
            }));
            return;
        }

        let displaced = previous.clone();
        for binding in duplicate_trs_bindings.iter_mut().filter(|binding| {
            binding.get("kind").and_then(JsonValue::as_str) == Some(kind)
                && binding.get("path").and_then(JsonValue::as_str) == Some(path.as_str())
                && binding
                    .get("canonicalTrackIndex")
                    .and_then(JsonValue::as_u64)
                    == u64::try_from(canonical_track_index).ok()
        }) {
            let object = binding
                .as_object_mut()
                .expect("duplicate TRS binding object");
            object.insert("relation".to_string(), json!("serializedLastWriteWins"));
            object.insert("canonicalSourceIndex".to_string(), json!(source_index));
            object.insert(
                "canonicalSourceEncoding".to_string(),
                json!(source_encoding),
            );
        }
        duplicate_trs_bindings.push(json!({
            "kind": kind,
            "path": path,
            "sourceIndex": displaced.source_index,
            "sourceEncoding": displaced.source_encoding,
            "relation": "serializedLastWriteWins",
            "canonicalTrackIndex": canonical_track_index,
            "canonicalSourceIndex": source_index,
            "canonicalSourceEncoding": source_encoding,
            "keys": displaced.keys,
            "duplicateKeys": displaced.duplicate_keys,
            "sourceKeyCount": displaced.source_key_count,
        }));
        output[canonical_track_index] = DecodedCurveTrack {
            path,
            keys: samples.keys,
            duplicate_keys: samples.duplicate_keys,
            source_key_count: samples.source_key_count,
            source_encoding: source_encoding.to_owned(),
            source_index,
        };
        return;
    }
    output.push(DecodedCurveTrack {
        path,
        keys: samples.keys,
        duplicate_keys: samples.duplicate_keys,
        source_key_count: samples.source_key_count,
        source_encoding: source_encoding.to_owned(),
        source_index,
    });
}

pub(super) fn duplicate_trs_binding_source_order(binding: &JsonValue) -> (u8, u64) {
    let rank = match (
        binding.get("kind").and_then(JsonValue::as_str),
        binding.get("sourceEncoding").and_then(JsonValue::as_str),
    ) {
        (Some("translation"), Some("plain")) => 0,
        (Some("rotation"), Some("plain")) => 1,
        (Some("rotation"), Some("compressed")) => 2,
        (Some("scale"), Some("plain")) => 3,
        _ => u8::MAX,
    };
    let source_index = binding
        .get("sourceIndex")
        .and_then(JsonValue::as_u64)
        .unwrap_or(u64::MAX);
    (rank, source_index)
}

pub(super) fn decoded_curve_track_source_order(track: &DecodedCurveTrack) -> (u8, usize) {
    let encoding_rank = match track.source_encoding.as_str() {
        "plain" => 0,
        "compressed" => 1,
        _ => u8::MAX,
    };
    (encoding_rank, track.source_index)
}
