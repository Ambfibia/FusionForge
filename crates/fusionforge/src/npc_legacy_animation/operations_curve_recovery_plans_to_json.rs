use super::*;

pub(super) fn canonicalize_curve_keys(
    path: &str,
    decoded: Vec<JsonValue>,
) -> Result<DecodedCurveSamples, String> {
    let source_key_count = decoded.len();
    let mut keys = Vec::<JsonValue>::with_capacity(source_key_count);
    let mut duplicate_keys = Vec::<JsonValue>::new();
    for (source_key_index, mut key) in decoded.into_iter().enumerate() {
        let time = key
            .get("time")
            .and_then(JsonValue::as_f64)
            .ok_or_else(|| format!("{path}: decoded key[{source_key_index}] has no time"))?;
        if let Some(canonical) = keys.last() {
            let previous_time = canonical
                .get("time")
                .and_then(JsonValue::as_f64)
                .expect("canonical decoded key has time");
            if time < previous_time {
                return Err(format!(
                    "{path}: serialized key[{source_key_index}] time {time} precedes previous time {previous_time}; refusing to reorder exact source data"
                ));
            }
            if time == previous_time {
                let mut canonical_payload = canonical.clone();
                canonical_payload
                    .as_object_mut()
                    .expect("decoded key object")
                    .remove("sourceKeyIndex");
                if canonical_payload != key {
                    return Err(format!(
                        "{path}: serialized key[{source_key_index}] conflicts with key[{}] at identical time {time}",
                        canonical
                            .get("sourceKeyIndex")
                            .and_then(JsonValue::as_u64)
                            .unwrap_or_default()
                    ));
                }
                duplicate_keys.push(json!({
                    "sourceKeyIndex": source_key_index,
                    "canonicalKeyIndex": keys.len() - 1,
                    "relation": "identical-same-time",
                    "key": key,
                }));
                continue;
            }
        }
        key.as_object_mut()
            .expect("decoded key object")
            .insert("sourceKeyIndex".to_string(), json!(source_key_index));
        keys.push(key);
    }
    Ok(DecodedCurveSamples {
        keys,
        duplicate_keys,
        source_key_count,
    })
}

/// Unity's legacy `PackedQuatVector` stores one quaternion in exactly 32 bits:
/// three flag bits followed by one 9-bit and two 10-bit components.  The
/// omitted largest component is reconstructed from unit length.
pub(super) fn unpack_legacy_compressed_quaternion(packed: u32) -> Quat {
    let flags = packed & 0x7;
    let omitted = (flags & 0x3) as usize;
    let negative = flags & 0x4 != 0;
    let mut cursor = 3_u32;
    let mut quaternion = [0.0_f64; 4];
    let mut squared_sum = 0.0_f64;
    for (component, slot) in quaternion.iter_mut().enumerate() {
        if component == omitted {
            continue;
        }
        let bits = if component == (omitted + 1) % 4 {
            9
        } else {
            10
        };
        let mask = (1_u32 << bits) - 1;
        let raw = (packed >> cursor) & mask;
        cursor += bits;
        *slot = raw as f64 / (0.5 * f64::from(mask)) - 1.0;
        squared_sum += *slot * *slot;
    }
    quaternion[omitted] = (1.0 - squared_sum).max(0.0).sqrt();
    if negative {
        quaternion[omitted] = -quaternion[omitted];
    }
    normalize_quat(quaternion)
}

pub(super) fn normalize_weights(weights: &mut [f64; 4]) {
    let total = weights.iter().sum::<f64>();
    if total > f64::EPSILON {
        for weight in weights {
            *weight /= total;
        }
    } else {
        weights[0] = 1.0;
    }
}

pub(super) fn reflect_matrix_x(mut matrix: Matrix4) -> Matrix4 {
    for (row, values) in matrix.iter_mut().enumerate() {
        for (column, value) in values.iter_mut().enumerate() {
            if (row == 0) != (column == 0) {
                *value = -*value;
            }
        }
    }
    matrix
}

pub(super) fn compose_matrix(translation: Vec3, quaternion: Quat, scale: Vec3) -> Matrix4 {
    let [x, y, z, w] = normalize_quat(quaternion);
    let [sx, sy, sz] = scale;
    let (xx, yy, zz) = (x * x, y * y, z * z);
    let (xy, xz, yz) = (x * y, x * z, y * z);
    let (wx, wy, wz) = (w * x, w * y, w * z);
    [
        [
            (1.0 - 2.0 * (yy + zz)) * sx,
            (2.0 * (xy - wz)) * sy,
            (2.0 * (xz + wy)) * sz,
            translation[0],
        ],
        [
            (2.0 * (xy + wz)) * sx,
            (1.0 - 2.0 * (xx + zz)) * sy,
            (2.0 * (yz - wx)) * sz,
            translation[1],
        ],
        [
            (2.0 * (xz - wy)) * sx,
            (2.0 * (yz + wx)) * sy,
            (1.0 - 2.0 * (xx + yy)) * sz,
            translation[2],
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub(super) fn mat_mul(left: Matrix4, right: Matrix4) -> Matrix4 {
    let mut result = [[0.0; 4]; 4];
    for row in 0..4 {
        for column in 0..4 {
            result[row][column] = (0..4)
                .map(|index| left[row][index] * right[index][column])
                .sum();
        }
    }
    result
}

pub(super) fn common_ancestor(
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    values: &[ObjectKey],
) -> Option<ObjectKey> {
    let first = *values.first()?;
    let first_chain = ancestor_chain(transforms, first);
    first_chain.into_iter().find(|candidate| {
        values
            .iter()
            .skip(1)
            .all(|value| ancestor_chain(transforms, *value).contains(candidate))
    })
}

pub(super) fn ancestor_chain(
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    key: ObjectKey,
) -> Vec<ObjectKey> {
    let mut result = Vec::new();
    let mut current = Some(key);
    let mut guard = BTreeSet::new();
    while let Some(value) = current {
        if !guard.insert(value) {
            break;
        }
        result.push(value);
        current = transforms.get(&value).and_then(|node| node.parent);
    }
    result
}

pub(super) fn ancestor_in_set(
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    key: ObjectKey,
    roots: &BTreeSet<ObjectKey>,
) -> bool {
    ancestor_chain(transforms, key)
        .into_iter()
        .any(|candidate| roots.contains(&candidate))
}

pub(super) fn depth_within(
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    key: ObjectKey,
    roots: &BTreeSet<ObjectKey>,
) -> usize {
    ancestor_chain(transforms, key)
        .into_iter()
        .position(|candidate| roots.contains(&candidate))
        .unwrap_or(usize::MAX)
}

pub(super) fn resolved_key(
    env: &fusionforge::UnityEnvironment,
    value: Option<&fusionforge::UnityValue>,
) -> Option<ObjectKey> {
    let pointer = value?.as_pointer()?;
    let key = env.resolve_pointer(pointer).ok()?;
    Some((key.asset, key.path_id))
}

pub(super) fn curve_count(body: &fusionforge::UnityValue, field: &str) -> usize {
    fusionforge::value_array(body.get(field)).len()
}

pub(super) fn curve_tracks_to_json(values: DecodedCurveTracks) -> Vec<JsonValue> {
    values
        .into_iter()
        .map(|track| {
            let interpolation = if track
                .keys
                .iter()
                .all(|key| key.get("inTangent").is_some() && key.get("outTangent").is_some())
            {
                "CUBICSPLINE"
            } else {
                "LINEAR"
            };
            json!({
                "path": track.path,
                "interpolation": interpolation,
                "keys": track.keys,
                "duplicateKeys": track.duplicate_keys,
                "sourceKeyCount": track.source_key_count,
                "sourceIndex": track.source_index,
                "sourceEncoding": track.source_encoding,
            })
        })
        .collect()
}

pub(super) fn curve_recovery_plans_to_json(
    plans: &[CurveRecoveryPlan],
    translations: &[DecodedCurveTrack],
    rotations: &[DecodedCurveTrack],
    scales: &[DecodedCurveTrack],
) -> Result<Vec<JsonValue>, String> {
    plans
        .iter()
        .map(|plan| {
            let tracks = match plan.kind {
                "translation" => translations,
                "rotation" => rotations,
                "scale" => scales,
                _ => return Err(format!("invalid curve recovery kind {:?}", plan.kind)),
            };
            let (canonical_track_index, canonical_track) = tracks
                .iter()
                .enumerate()
                .find(|(_, track)| {
                    track.path == plan.path
                        && track.source_encoding == "plain"
                        && track.source_index == plan.canonical_source_index
                })
                .ok_or_else(|| {
                    format!(
                        "exact sibling recovery has no canonical {} plain curve[{}] for {:?}",
                        plan.kind, plan.canonical_source_index, plan.path
                    )
                })?;
            let mut canonical = curve_track_to_json(canonical_track);
            canonical
                .as_object_mut()
                .expect("curve track JSON object")
                .insert("trackIndex".to_string(), json!(canonical_track_index));
            let rejected = plan
                .rejected
                .iter()
                .map(|(source_index, samples)| {
                    decoded_samples_to_track_json(&plan.path, "plain", *source_index, samples)
                })
                .collect::<Vec<_>>();
            Ok(json!({
                "kind": plan.kind,
                "path": plan.path,
                "sourceEncoding": "plain",
                "reason": "conflicting-duplicate-constant-curve-resolved-from-exact-sibling-curve",
                "sourceSampleRate": plan.source_sample_rate,
                "source": {
                    "field": plan.field,
                    "sourceEncoding": "plain",
                    "sourceIndices": plan.source_indices,
                    "sourceTargetCurveCount": plan.source_indices.len(),
                },
                "canonical": canonical,
                "rejected": rejected,
                "reference": {
                    "asset": plan.reference.asset_name,
                    "pathId": plan.reference.path_id,
                    "clipName": plan.reference.clip_name,
                    "field": plan.reference.field,
                    "kind": plan.reference.kind,
                    "path": plan.reference.path,
                    "sourceEncoding": "plain",
                    "sourceIndex": plan.reference.source_index,
                    "sampleRate": plan.reference.sample_rate,
                },
                "proof": {
                    "exactPath": true,
                    "exactSampleRate": true,
                    "allSourceCurvesConstant": true,
                    "canonicalMatchesReference": true,
                    "uniqueCanonicalCandidateCount": 1,
                    "matchingReferenceCount": 1,
                    "sourceTargetCurveCount": plan.source_indices.len(),
                },
            }))
        })
        .collect()
}

pub(super) fn curve_track_to_json(track: &DecodedCurveTrack) -> JsonValue {
    decoded_samples_to_track_json(
        &track.path,
        &track.source_encoding,
        track.source_index,
        &DecodedCurveSamples {
            keys: track.keys.clone(),
            duplicate_keys: track.duplicate_keys.clone(),
            source_key_count: track.source_key_count,
        },
    )
}

pub(super) fn decoded_samples_to_track_json(
    path: &str,
    source_encoding: &str,
    source_index: usize,
    samples: &DecodedCurveSamples,
) -> JsonValue {
    let interpolation = if samples
        .keys
        .iter()
        .all(|key| key.get("inTangent").is_some() && key.get("outTangent").is_some())
    {
        "CUBICSPLINE"
    } else {
        "LINEAR"
    };
    json!({
        "path": path,
        "interpolation": interpolation,
        "keys": samples.keys,
        "duplicateKeys": samples.duplicate_keys,
        "sourceKeyCount": samples.source_key_count,
        "sourceIndex": source_index,
        "sourceEncoding": source_encoding,
    })
}

pub(super) fn normalize_quat(value: Quat) -> Quat {
    let length = value
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    if !length.is_finite() || length <= f64::EPSILON {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [
            value[0] / length,
            value[1] / length,
            value[2] / length,
            value[3] / length,
        ]
    }
}
