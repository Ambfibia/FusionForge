use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_time_recoveries(
    animation: &JsonValue,
    recoveries: &[JsonValue],
    translations: &[JsonValue],
    rotations: &[JsonValue],
    scales: &[JsonValue],
    animations: &[JsonValue],
    hierarchy_paths: &[String],
    context: &str,
) -> Result<(), String> {
    let mut previous_order = None;
    let mut identities = BTreeSet::<(String, String, usize)>::new();
    for (recovery_index, recovery) in recoveries.iter().enumerate() {
        let kind = recovery
            .get("kind")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("{context} timeRecoveries[{recovery_index}] has no kind"))?;
        let path = recovery
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("{context} timeRecoveries[{recovery_index}] has no path"))?;
        let source_encoding = recovery
            .get("sourceEncoding")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| {
                format!("{context} timeRecoveries[{recovery_index}] has no sourceEncoding")
            })?;
        let source_index = recovery
            .get("sourceIndex")
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                format!("{context} timeRecoveries[{recovery_index}] has invalid sourceIndex")
            })?;
        let rank = match (kind, source_encoding) {
            ("translation", "plain") => 0_u8,
            ("rotation", "plain") => 1_u8,
            ("scale", "plain") => 3_u8,
            _ => {
                return Err(format!(
                    "{context} timeRecoveries[{recovery_index}] has invalid kind/encoding {kind:?}/{source_encoding:?}"
                ));
            }
        };
        let order = (rank, source_index);
        if previous_order.is_some_and(|previous| previous >= order) {
            return Err(format!(
                "{context} timeRecoveries are not in canonical source order"
            ));
        }
        previous_order = Some(order);
        if !identities.insert((kind.to_string(), source_encoding.to_string(), source_index)) {
            return Err(format!(
                "{context} repeats time recovery for {kind} {source_encoding} curve[{source_index}]"
            ));
        }
        if recovery.get("reason").and_then(JsonValue::as_str)
            != Some("non-strict-source-times-recovered-from-exact-sibling-curve")
        {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] has an unsupported reason"
            ));
        }
        let original_times = exact_time_array(
            recovery.get("originalTimes"),
            &format!("{context} timeRecoveries[{recovery_index}].originalTimes"),
        )?;
        let recovered_times = exact_time_array(
            recovery.get("recoveredTimes"),
            &format!("{context} timeRecoveries[{recovery_index}].recoveredTimes"),
        )?;
        if original_times.len() != recovered_times.len() || original_times.is_empty() {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] original/recovered key counts differ"
            ));
        }
        if strictly_increasing_finite_times(&original_times) {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] original times were already strict"
            ));
        }
        if !strictly_increasing_finite_times(&recovered_times) {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] recovered times are not strict"
            ));
        }
        let source_sample_rate = exact_nullable_f64(
            recovery.get("sourceSampleRate"),
            &format!("{context} timeRecoveries[{recovery_index}].sourceSampleRate"),
        )?;
        let animation_sample_rate = exact_nullable_f64(
            animation.get("sampleRate"),
            &format!("{context}.sampleRate"),
        )?;
        if source_sample_rate.map(f64::to_bits) != animation_sample_rate.map(f64::to_bits) {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] source sample rate does not match its clip"
            ));
        }
        let source_track = find_source_track(
            kind,
            source_encoding,
            source_index,
            translations,
            rotations,
            scales,
        )
        .ok_or_else(|| {
            format!("{context} timeRecoveries[{recovery_index}] has no canonical source track")
        })?;
        if source_track.get("path").and_then(JsonValue::as_str) != Some(path) {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] path does not match its source track"
            ));
        }
        validate_recovered_track_times(source_track, &recovered_times, context, recovery_index)?;
        resolve_exact_animation_path(hierarchy_paths, path, context)?;

        let reference = recovery.get("reference").ok_or_else(|| {
            format!("{context} timeRecoveries[{recovery_index}] has no reference")
        })?;
        let reference_asset = reference
            .get("asset")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| {
                format!("{context} timeRecoveries[{recovery_index}] reference has no asset")
            })?;
        let reference_path_id = reference
            .get("pathId")
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| {
                format!("{context} timeRecoveries[{recovery_index}] reference has no pathId")
            })?;
        let reference_clip_name = reference
            .get("clipName")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| {
                format!("{context} timeRecoveries[{recovery_index}] reference has no clipName")
            })?;
        if reference.get("kind").and_then(JsonValue::as_str) != Some(kind)
            || reference.get("path").and_then(JsonValue::as_str) != Some(path)
            || reference.get("sourceEncoding").and_then(JsonValue::as_str) != Some(source_encoding)
        {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] reference identity does not match the source curve"
            ));
        }
        let reference_source_index = reference
            .get("sourceIndex")
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                format!(
                    "{context} timeRecoveries[{recovery_index}] reference has invalid sourceIndex"
                )
            })?;
        let reference_sample_rate = exact_nullable_f64(
            reference.get("sampleRate"),
            &format!("{context} timeRecoveries[{recovery_index}].reference.sampleRate"),
        )?;
        if reference_sample_rate.map(f64::to_bits) != source_sample_rate.map(f64::to_bits) {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] reference sample rate differs"
            ));
        }
        let reference_animation = animations
            .iter()
            .find(|candidate| {
                candidate.get("asset").and_then(JsonValue::as_str) == Some(reference_asset)
                    && candidate.get("pathId").and_then(JsonValue::as_i64)
                        == Some(reference_path_id)
                    && candidate.get("name").and_then(JsonValue::as_str)
                        == Some(reference_clip_name)
            })
            .ok_or_else(|| {
                format!(
                    "{context} timeRecoveries[{recovery_index}] exact sibling reference clip is absent"
                )
            })?;
        let reference_data = reference_animation.get("animationData").ok_or_else(|| {
            format!(
                "{context} timeRecoveries[{recovery_index}] sibling reference has no animationData"
            )
        })?;
        let reference_tracks = match kind {
            "translation" => reference_data.get("translations"),
            "rotation" => reference_data.get("rotations"),
            "scale" => reference_data.get("scales"),
            _ => None,
        }
        .and_then(JsonValue::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
        let reference_track = reference_tracks
            .iter()
            .find(|track| {
                track.get("sourceEncoding").and_then(JsonValue::as_str) == Some(source_encoding)
                    && track.get("sourceIndex").and_then(JsonValue::as_u64)
                        == u64::try_from(reference_source_index).ok()
            })
            .ok_or_else(|| {
                format!(
                    "{context} timeRecoveries[{recovery_index}] sibling reference track is absent"
                )
            })?;
        if reference_track.get("path").and_then(JsonValue::as_str) != Some(path) {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] sibling reference path differs"
            ));
        }
        validate_recovered_track_times(reference_track, &recovered_times, context, recovery_index)?;
        if !track_payloads_equal_excluding_time(source_track, reference_track) {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] sibling key payload differs"
            ));
        }
        let proof = recovery
            .get("proof")
            .ok_or_else(|| format!("{context} timeRecoveries[{recovery_index}] has no proof"))?;
        if proof
            .get("exactKeyPayloadExcludingTime")
            .and_then(JsonValue::as_bool)
            != Some(true)
            || proof.get("exactPath").and_then(JsonValue::as_bool) != Some(true)
            || proof.get("exactSampleRate").and_then(JsonValue::as_bool) != Some(true)
            || proof
                .get("uniqueRecoveredTimeVectorCount")
                .and_then(JsonValue::as_u64)
                != Some(1)
            || proof
                .get("matchingReferenceCount")
                .and_then(JsonValue::as_u64)
                .is_none_or(|count| count == 0)
        {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] proof is incomplete"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_recovered_track_times(
    track: &JsonValue,
    expected: &[f64],
    context: &str,
    recovery_index: usize,
) -> Result<(), String> {
    if track
        .get("duplicateKeys")
        .and_then(JsonValue::as_array)
        .is_none_or(|values| !values.is_empty())
    {
        return Err(format!(
            "{context} timeRecoveries[{recovery_index}] track still has duplicate keys"
        ));
    }
    let source_key_count = track
        .get("sourceKeyCount")
        .and_then(JsonValue::as_u64)
        .and_then(|value| usize::try_from(value).ok());
    let keys = track
        .get("keys")
        .and_then(JsonValue::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if source_key_count != Some(expected.len()) || keys.len() != expected.len() {
        return Err(format!(
            "{context} timeRecoveries[{recovery_index}] recovered track key count differs"
        ));
    }
    for key in keys {
        let source_key_index = key
            .get("sourceKeyIndex")
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value < expected.len())
            .ok_or_else(|| {
                format!(
                    "{context} timeRecoveries[{recovery_index}] recovered key has invalid sourceKeyIndex"
                )
            })?;
        let time = key.get("time").and_then(JsonValue::as_f64).ok_or_else(|| {
            format!("{context} timeRecoveries[{recovery_index}] recovered key has no time")
        })?;
        if time.to_bits() != expected[source_key_index].to_bits() {
            return Err(format!(
                "{context} timeRecoveries[{recovery_index}] recovered key time differs"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_sampleable_tracks(
    tracks: &[JsonValue],
    kind: &str,
    source_encodings: &[(&str, usize)],
    hierarchy_paths: &[String],
    context: &str,
    duration: f64,
    targets: &mut BTreeSet<(String, String)>,
    source_identities: &mut BTreeSet<(String, String, usize)>,
) -> Result<Option<f64>, String> {
    let mut maximum_time = None::<f64>;
    let mut previous_source_order = None;
    for (index, track) in tracks.iter().enumerate() {
        let path = track
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("{context} {kind} track[{index}] has no path"))?;
        let source_encoding = track
            .get("sourceEncoding")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("{context} {kind} track[{index}] has no sourceEncoding"))?;
        let source_index = track
            .get("sourceIndex")
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("{context} {kind} track[{index}] has invalid sourceIndex"))?;
        let (encoding_rank, source_count) = source_encodings
            .iter()
            .enumerate()
            .find(|(_, (candidate, _))| candidate == &source_encoding)
            .map(|(rank, (_, count))| (rank, *count))
            .ok_or_else(|| {
                format!(
                    "{context} {kind} track[{index}] has invalid sourceEncoding {source_encoding:?}"
                )
            })?;
        if source_index >= source_count {
            return Err(format!(
                "{context} {kind} track[{index}] sourceIndex {source_index} is outside raw count {source_count}"
            ));
        }
        let source_order = (encoding_rank, source_index);
        if previous_source_order.is_some_and(|previous| previous >= source_order) {
            return Err(format!(
                "{context} sampleable {kind} tracks are not in canonical source order"
            ));
        }
        previous_source_order = Some(source_order);
        if !source_identities.insert((kind.to_string(), source_encoding.to_string(), source_index))
        {
            return Err(format!(
                "{context} repeats {kind} {source_encoding} sourceIndex {source_index}"
            ));
        }
        let keys = track
            .get("keys")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| format!("{context} {kind} track[{index}] has no keys array"))?;
        let duplicate_keys = track
            .get("duplicateKeys")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| {
                format!("{context} {kind} track[{index}] has no typed duplicateKeys array")
            })?;
        let source_key_count = track
            .get("sourceKeyCount")
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("{context} {kind} track[{index}] has invalid sourceKeyCount"))?;
        if keys.is_empty() {
            return Err(format!(
                "{context} {kind} track[{index}] {path:?} is empty instead of using emptyTrsBindings"
            ));
        }
        let mut previous_time = None::<f64>;
        let mut previous_source_key_index = None::<usize>;
        let mut source_key_identities = BTreeSet::<usize>::new();
        for (key_index, key) in keys.iter().enumerate() {
            let source_key_index = key
                .get("sourceKeyIndex")
                .and_then(JsonValue::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value < source_key_count)
                .ok_or_else(|| {
                    format!(
                        "{context} {kind} track[{index}] key[{key_index}] has invalid sourceKeyIndex"
                    )
                })?;
            if previous_source_key_index.is_some_and(|previous| previous >= source_key_index) {
                return Err(format!(
                    "{context} {kind} track[{index}] canonical keys are not in source order"
                ));
            }
            previous_source_key_index = Some(source_key_index);
            if !source_key_identities.insert(source_key_index) {
                return Err(format!(
                    "{context} {kind} track[{index}] repeats source key[{source_key_index}]"
                ));
            }
            let time = key
                .get("time")
                .and_then(JsonValue::as_f64)
                .filter(|time| {
                    time.is_finite()
                        && (*time as f32).is_finite()
                        && *time >= 0.0
                        && *time <= duration + 1.0e-6
                })
                .ok_or_else(|| {
                    format!("{context} {kind} track[{index}] key[{key_index}] has invalid time")
                })?;
            if previous_time.is_some_and(|previous| previous >= time) {
                return Err(format!(
                    "{context} {kind} track[{index}] keys are not strictly ordered by time"
                ));
            }
            previous_time = Some(time);
            maximum_time = Some(maximum_time.map_or(time, |current| current.max(time)));
        }
        let mut previous_duplicate_source_index = None::<usize>;
        for (duplicate_index, duplicate) in duplicate_keys.iter().enumerate() {
            let source_key_index = duplicate
                .get("sourceKeyIndex")
                .and_then(JsonValue::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value < source_key_count)
                .ok_or_else(|| {
                    format!(
                        "{context} {kind} track[{index}] duplicateKeys[{duplicate_index}] has invalid sourceKeyIndex"
                    )
                })?;
            if previous_duplicate_source_index.is_some_and(|previous| previous >= source_key_index)
            {
                return Err(format!(
                    "{context} {kind} track[{index}] duplicateKeys are not in source order"
                ));
            }
            previous_duplicate_source_index = Some(source_key_index);
            if !source_key_identities.insert(source_key_index) {
                return Err(format!(
                    "{context} {kind} track[{index}] repeats source key[{source_key_index}]"
                ));
            }
            if duplicate.get("relation").and_then(JsonValue::as_str) != Some("identical-same-time")
            {
                return Err(format!(
                    "{context} {kind} track[{index}] duplicateKeys[{duplicate_index}] is not lossless-identical"
                ));
            }
            let canonical_key_index = duplicate
                .get("canonicalKeyIndex")
                .and_then(JsonValue::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    format!(
                        "{context} {kind} track[{index}] duplicateKeys[{duplicate_index}] has invalid canonicalKeyIndex"
                    )
                })?;
            let canonical = keys.get(canonical_key_index).ok_or_else(|| {
                format!(
                    "{context} {kind} track[{index}] duplicateKeys[{duplicate_index}] canonicalKeyIndex {canonical_key_index} is out of range"
                )
            })?;
            let mut canonical_payload = canonical.clone();
            canonical_payload
                .as_object_mut()
                .ok_or_else(|| {
                    format!("{context} {kind} track[{index}] canonical key is not an object")
                })?
                .remove("sourceKeyIndex");
            if duplicate.get("key") != Some(&canonical_payload) {
                return Err(format!(
                    "{context} {kind} track[{index}] duplicateKeys[{duplicate_index}] does not exactly match canonical key[{canonical_key_index}]"
                ));
            }
        }
        if source_key_count != keys.len() + duplicate_keys.len()
            || (0..source_key_count)
                .any(|source_index| !source_key_identities.contains(&source_index))
        {
            return Err(format!(
                "{context} {kind} track[{index}] sourceKeyCount {source_key_count} is not fully covered by {} canonical + {} identical duplicate keys",
                keys.len(),
                duplicate_keys.len()
            ));
        }
        let unbound_model_target = match track.get("unboundModelTarget") {
            Some(JsonValue::Bool(value)) => *value,
            Some(_) => {
                return Err(format!(
                    "{context} {kind} track[{index}] has non-boolean unboundModelTarget"
                ));
            }
            None => false,
        };
        match resolve_exact_animation_path(hierarchy_paths, path, context) {
            Ok(resolved) if unbound_model_target => {
                return Err(format!(
                    "{context} {kind} track[{index}] marks existing target {resolved:?} as unbound"
                ));
            }
            Ok(resolved) => {
                if !targets.insert((kind.to_string(), resolved.clone())) {
                    return Err(format!(
                        "{context} has duplicate sampleable {kind} target {resolved:?}"
                    ));
                }
            }
            Err(error)
                if unbound_model_target
                    && error.contains("has no suffix match under the true root") => {}
            Err(error) => return Err(error),
        }
    }
    Ok(maximum_time)
}

pub(super) fn validate_sampleable_float_tracks(
    tracks: &[JsonValue],
    source_count: usize,
    hierarchy_paths: &[String],
    context: &str,
    duration: f64,
) -> Result<Option<f64>, String> {
    let mut maximum_time = None::<f64>;
    let mut source_indices = BTreeSet::new();
    let mut targets = BTreeSet::new();
    let mut previous_source_index = None;
    for (index, track) in tracks.iter().enumerate() {
        let path = track
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("{context} float track[{index}] has no path"))?;
        let property = track
            .get("property")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty() && !value.chars().any(char::is_control))
            .ok_or_else(|| format!("{context} float track[{index}] has invalid property"))?;
        let class_id = track
            .get("classId")
            .and_then(JsonValue::as_i64)
            .filter(|value| i32::try_from(*value).is_ok() && *value > 0)
            .ok_or_else(|| format!("{context} float track[{index}] has invalid classId"))?;
        let source_encoding = track.get("sourceEncoding").and_then(JsonValue::as_str);
        if source_encoding != Some("plain") {
            return Err(format!(
                "{context} float track[{index}] has invalid sourceEncoding"
            ));
        }
        let source_index = track
            .get("sourceIndex")
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value < source_count)
            .ok_or_else(|| format!("{context} float track[{index}] has invalid sourceIndex"))?;
        if previous_source_index.is_some_and(|previous| previous >= source_index)
            || !source_indices.insert(source_index)
        {
            return Err(format!(
                "{context} float tracks are not in exact source order"
            ));
        }
        previous_source_index = Some(source_index);

        let script = track
            .get("script")
            .and_then(JsonValue::as_object)
            .ok_or_else(|| format!("{context} float track[{index}] has no typed script PPtr"))?;
        let file_id = script
            .get("fileId")
            .and_then(JsonValue::as_i64)
            .filter(|value| i32::try_from(*value).is_ok())
            .ok_or_else(|| format!("{context} float track[{index}] has invalid script fileId"))?;
        let script_path_id = script
            .get("pathId")
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| format!("{context} float track[{index}] has invalid script pathId"))?;
        for field in ["preInfinity", "postInfinity"] {
            track
                .get(field)
                .and_then(JsonValue::as_i64)
                .filter(|value| i32::try_from(*value).is_ok())
                .ok_or_else(|| format!("{context} float track[{index}] has invalid {field}"))?;
        }
        let keys = track
            .get("keys")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| format!("{context} float track[{index}] has no keys"))?;
        let duplicate_keys = track
            .get("duplicateKeys")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| format!("{context} float track[{index}] has no typed duplicateKeys"))?;
        if !duplicate_keys.is_empty() {
            return Err(format!(
                "{context} float track[{index}] has duplicate-time keys without native float duplicate-key provenance"
            ));
        }
        let source_key_count = track
            .get("sourceKeyCount")
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("{context} float track[{index}] has invalid sourceKeyCount"))?;
        // Unity can serialize an EditorCurveBinding whose AnimationCurve has
        // zero keys (Bubbie's stand3 splash_back _MainTex.offset.x does).
        // The binding is still exact source metadata even though it has
        // nothing to sample, so preserve it instead of fabricating a key or
        // rejecting the otherwise complete clip.
        if source_key_count != keys.len() {
            return Err(format!(
                "{context} float track[{index}] does not preserve every serialized key"
            ));
        }
        let interpolation = track
            .get("interpolation")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("{context} float track[{index}] has no interpolation"))?;
        let mut previous_time = None;
        let mut all_tangents = true;
        let mut no_tangents = true;
        for (key_index, key) in keys.iter().enumerate() {
            if key.get("sourceKeyIndex").and_then(JsonValue::as_u64)
                != u64::try_from(key_index).ok()
            {
                return Err(format!(
                    "{context} float track[{index}] key[{key_index}] lost its source identity"
                ));
            }
            let time = key
                .get("time")
                .and_then(JsonValue::as_f64)
                .filter(|time| {
                    // Serialized Unity material curves may carry one pre-roll
                    // key before t=0 (ES[736] uses -1/30 s for UV scrolling).
                    // It is part of the exact source curve, not a corrupt key.
                    time.is_finite() && (*time as f32).is_finite() && *time <= duration + 1.0e-6
                })
                .ok_or_else(|| {
                    format!("{context} float track[{index}] key[{key_index}] has invalid time")
                })?;
            if previous_time.is_some_and(|previous| previous >= time) {
                return Err(format!(
                    "{context} float track[{index}] keys are not strictly ordered"
                ));
            }
            previous_time = Some(time);
            maximum_time = Some(maximum_time.map_or(time, |current| current.max(time)));
            key.get("value")
                .and_then(JsonValue::as_f64)
                .filter(|value| value.is_finite() && (*value as f32).is_finite())
                .ok_or_else(|| {
                    format!("{context} float track[{index}] key[{key_index}] has invalid value")
                })?;
            let in_tangent = key.get("inTangent");
            let out_tangent = key.get("outTangent");
            all_tangents &= in_tangent.is_some() && out_tangent.is_some();
            no_tangents &= in_tangent.is_none() && out_tangent.is_none();
            for (field, tangent) in [("inTangent", in_tangent), ("outTangent", out_tangent)] {
                if let Some(tangent) = tangent {
                    tangent
                        .as_f64()
                        .filter(|value| value.is_finite() && (*value as f32).is_finite())
                        .ok_or_else(|| {
                            format!(
                                "{context} float track[{index}] key[{key_index}] has invalid {field}"
                            )
                        })?;
                }
            }
            if key
                .get("tangentMode")
                .is_some_and(|value| value.as_i64().and_then(|v| i32::try_from(v).ok()).is_none())
            {
                return Err(format!(
                    "{context} float track[{index}] key[{key_index}] has invalid tangentMode"
                ));
            }
        }
        if (interpolation == "CUBICSPLINE" && !all_tangents)
            || (matches!(interpolation, "LINEAR" | "STEP") && !no_tangents)
            || !matches!(interpolation, "CUBICSPLINE" | "LINEAR" | "STEP")
        {
            return Err(format!(
                "{context} float track[{index}] interpolation contradicts serialized tangents"
            ));
        }
        let resolved = resolve_exact_animation_path(hierarchy_paths, path, context)?;
        if !targets.insert((
            resolved,
            class_id,
            property.to_string(),
            file_id,
            script_path_id,
        )) {
            return Err(format!(
                "{context} repeats an exact float binding target/property"
            ));
        }
    }
    if source_indices.len() != source_count
        || (0..source_count).any(|source_index| !source_indices.contains(&source_index))
    {
        return Err(format!(
            "{context} discarded one or more serialized float curves"
        ));
    }
    Ok(maximum_time)
}
