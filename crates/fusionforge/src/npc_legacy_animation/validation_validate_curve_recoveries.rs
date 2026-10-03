use super::*;

pub(super) fn validate_exact_event_object_parameter(
    event: &JsonValue,
    event_index: usize,
    context: &str,
) -> Result<(), String> {
    let event_context = format!("{context} event[{event_index}]");
    let object_parameter = event
        .get("objectParameter")
        .ok_or_else(|| format!("{event_context} has no typed objectParameter"))?;
    let provenance = event
        .get("objectParameterProvenance")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| format!("{event_context} has no objectParameterProvenance object"))?;
    let presence = provenance
        .get("presence")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("{event_context} object provenance has no presence"))?;
    match presence {
        "missing" => {
            if !object_parameter.is_null()
                || provenance.get("interpretation").and_then(JsonValue::as_str) != Some("missing")
                || provenance.len() != 2
            {
                return Err(format!(
                    "{event_context} missing object parameter provenance is inconsistent"
                ));
            }
        }
        "serialized-pointer" => {
            let source_asset_index = provenance
                .get("sourceAssetIndex")
                .and_then(JsonValue::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    format!("{event_context} object provenance has invalid sourceAssetIndex")
                })?;
            let file_id = provenance
                .get("fileId")
                .and_then(JsonValue::as_i64)
                .and_then(|value| i32::try_from(value).ok())
                .ok_or_else(|| format!("{event_context} object provenance has invalid fileId"))?;
            let path_id = provenance
                .get("pathId")
                .and_then(JsonValue::as_i64)
                .ok_or_else(|| format!("{event_context} object provenance has invalid pathId"))?;
            let interpretation = provenance
                .get("interpretation")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| {
                    format!("{event_context} object provenance has no interpretation")
                })?;
            if provenance.len() != 5 {
                return Err(format!(
                    "{event_context} serialized object provenance has unknown/missing fields"
                ));
            }
            if path_id == 0 {
                if !object_parameter.is_null() || interpretation != "null-path-id" {
                    return Err(format!(
                        "{event_context} null-path-id object provenance is inconsistent"
                    ));
                }
            } else {
                let expected = json!({
                    "sourceAssetIndex": source_asset_index,
                    "fileId": file_id,
                    "pathId": path_id,
                });
                let metadata_only_sound = interpretation == "non-null-metadata-only"
                    && event.get("functionName").and_then(JsonValue::as_str) == Some("sound")
                    && event
                        .get("stringParameter")
                        .and_then(JsonValue::as_str)
                        .is_some_and(|value| !value.trim().is_empty());
                let legacy_unused = interpretation == "non-null-legacy-unused"
                    && event.get("functionName").and_then(JsonValue::as_str) == Some("end")
                    && event.get("stringParameter").and_then(JsonValue::as_str) == Some("");
                if object_parameter != &expected
                    || !(interpretation == "non-null-unresolved"
                        || metadata_only_sound
                        || legacy_unused)
                {
                    return Err(format!(
                        "{event_context} unresolved non-null object provenance is inconsistent"
                    ));
                }
            }
        }
        other => {
            return Err(format!(
                "{event_context} has unsupported object parameter presence {other:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_exact_event_float_parameter(
    event: &JsonValue,
    event_index: usize,
    context: &str,
) -> Result<(), String> {
    let event_context = format!("{context} event[{event_index}]");
    let value = event
        .get("floatParameter")
        .and_then(JsonValue::as_f64)
        .filter(|value| value.is_finite() && (*value as f32).is_finite())
        .ok_or_else(|| format!("{event_context} has invalid floatParameter"))?;
    let Some(provenance) = event.get("floatParameterProvenance") else {
        return Ok(());
    };
    let provenance = provenance
        .as_object()
        .ok_or_else(|| format!("{event_context} floatParameterProvenance is not an object"))?;
    if value != 0.0
        || provenance.len() != 3
        || provenance
            .get("sourceAssetFormat")
            .and_then(JsonValue::as_u64)
            != Some(6)
        || provenance.get("rawFloat32Bits").and_then(JsonValue::as_str) != Some("0xffffff00")
        || provenance.get("interpretation").and_then(JsonValue::as_str)
            != Some("legacy-unused-nonfinite-normalized-to-zero")
    {
        return Err(format!(
            "{event_context} floatParameterProvenance is inconsistent"
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_curve_recoveries(
    animation: &JsonValue,
    recoveries: &[JsonValue],
    translations: &[JsonValue],
    rotations: &[JsonValue],
    scales: &[JsonValue],
    animations: &[JsonValue],
    hierarchy_paths: &[String],
    context: &str,
    source_identities: &mut BTreeSet<(String, String, usize)>,
) -> Result<(), String> {
    let target_duration = exact_required_duration(animation, "duration", context)?;
    let target_sample_rate = exact_nullable_f64(animation.get("sampleRate"), context)?;
    let mut previous_first_order = None::<(u8, usize)>;
    for (recovery_index, recovery) in recoveries.iter().enumerate() {
        let recovery_context = format!("{context} curveRecoveries[{recovery_index}]");
        let kind = recovery
            .get("kind")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("{recovery_context} has no kind"))?;
        let path = recovery
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("{recovery_context} has no path"))?;
        let (rank, field, raw_count, canonical_tracks) = match kind {
            "translation" => (
                0_u8,
                "m_PositionCurves",
                exact_curve_count(animation, "position", context)?,
                translations,
            ),
            "rotation" => (
                1_u8,
                "m_RotationCurves",
                exact_curve_count(animation, "rotation", context)?,
                rotations,
            ),
            "scale" => (
                3_u8,
                "m_ScaleCurves",
                exact_curve_count(animation, "scale", context)?,
                scales,
            ),
            _ => return Err(format!("{recovery_context} has unsupported kind {kind:?}")),
        };
        if recovery.get("sourceEncoding").and_then(JsonValue::as_str) != Some("plain")
            || recovery.get("reason").and_then(JsonValue::as_str)
                != Some("conflicting-duplicate-constant-curve-resolved-from-exact-sibling-curve")
        {
            return Err(format!(
                "{recovery_context} has unsupported encoding/reason"
            ));
        }
        resolve_exact_animation_path(hierarchy_paths, path, context)?;
        let source = recovery
            .get("source")
            .ok_or_else(|| format!("{recovery_context} has no source provenance"))?;
        if source.get("field").and_then(JsonValue::as_str) != Some(field)
            || source.get("sourceEncoding").and_then(JsonValue::as_str) != Some("plain")
        {
            return Err(format!(
                "{recovery_context} source provenance differs from its kind"
            ));
        }
        let source_indices = source
            .get("sourceIndices")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| format!("{recovery_context} sourceIndices is not an array"))?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .filter(|value| *value < raw_count)
                    .ok_or_else(|| format!("{recovery_context} has invalid sourceIndex"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if source_indices.len() < 2
            || source_indices.windows(2).any(|pair| pair[0] >= pair[1])
            || source
                .get("sourceTargetCurveCount")
                .and_then(JsonValue::as_u64)
                != u64::try_from(source_indices.len()).ok()
        {
            return Err(format!(
                "{recovery_context} source indices/count are not strict and complete"
            ));
        }
        let first_order = (rank, source_indices[0]);
        if previous_first_order.is_some_and(|previous| previous >= first_order) {
            return Err(format!("{context} curveRecoveries are not in source order"));
        }
        // Different target paths can interleave in the serialized array, e.g.
        // A, B, A, B. Order groups by their first occurrence; source_identities
        // below still proves every individual source index is unique.
        previous_first_order = Some(first_order);

        let canonical = recovery
            .get("canonical")
            .ok_or_else(|| format!("{recovery_context} has no canonical provenance"))?;
        let canonical_track_index = canonical
            .get("trackIndex")
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("{recovery_context} canonical trackIndex is invalid"))?;
        let canonical_track = canonical_tracks
            .get(canonical_track_index)
            .ok_or_else(|| format!("{recovery_context} canonical trackIndex is out of range"))?;
        for field in [
            "path",
            "interpolation",
            "keys",
            "duplicateKeys",
            "sourceKeyCount",
            "sourceIndex",
            "sourceEncoding",
        ] {
            if canonical.get(field) != canonical_track.get(field) {
                return Err(format!(
                    "{recovery_context} canonical {field} differs from the published track"
                ));
            }
        }
        let (canonical_source_index, canonical_payload) = validate_constant_recovery_track(
            canonical,
            kind,
            path,
            target_duration,
            &format!("{recovery_context}.canonical"),
        )?;
        if !source_indices.contains(&canonical_source_index) {
            return Err(format!(
                "{recovery_context} canonical source is outside sourceIndices"
            ));
        }

        let rejected = recovery
            .get("rejected")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| format!("{recovery_context} rejected is not an array"))?;
        if rejected.is_empty() {
            return Err(format!(
                "{recovery_context} has no rejected conflicting source"
            ));
        }
        let mut rejected_indices = Vec::with_capacity(rejected.len());
        let mut source_payloads = vec![(canonical_source_index, canonical_payload.clone())];
        for (rejected_index, track) in rejected.iter().enumerate() {
            let (source_index, payload) = validate_constant_recovery_track(
                track,
                kind,
                path,
                target_duration,
                &format!("{recovery_context}.rejected[{rejected_index}]"),
            )?;
            if !source_identities.insert((kind.to_string(), "plain".to_string(), source_index)) {
                return Err(format!(
                    "{recovery_context} rejected sourceIndex {source_index} is repeated"
                ));
            }
            rejected_indices.push(source_index);
            source_payloads.push((source_index, payload));
        }
        let expected_rejected = source_indices
            .iter()
            .copied()
            .filter(|source_index| *source_index != canonical_source_index)
            .collect::<Vec<_>>();
        if rejected_indices != expected_rejected
            || source_payloads
                .iter()
                .skip(1)
                .all(|(_, payload)| payload == &canonical_payload)
        {
            return Err(format!(
                "{recovery_context} rejected provenance is incomplete or not conflicting"
            ));
        }

        let source_sample_rate = exact_nullable_f64(
            recovery.get("sourceSampleRate"),
            &format!("{recovery_context}.sourceSampleRate"),
        )?;
        if !optional_f64_bits_equal(source_sample_rate, target_sample_rate) {
            return Err(format!(
                "{recovery_context} source sample rate differs from its clip"
            ));
        }
        let reference = recovery
            .get("reference")
            .ok_or_else(|| format!("{recovery_context} has no reference"))?;
        let reference_asset = reference
            .get("asset")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("{recovery_context} reference has no asset"))?;
        let reference_path_id = reference
            .get("pathId")
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| format!("{recovery_context} reference has no pathId"))?;
        let reference_clip_name = reference
            .get("clipName")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("{recovery_context} reference has no clipName"))?;
        let reference_source_index = reference
            .get("sourceIndex")
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("{recovery_context} reference sourceIndex is invalid"))?;
        let reference_sample_rate = exact_nullable_f64(
            reference.get("sampleRate"),
            &format!("{recovery_context}.reference.sampleRate"),
        )?;
        if reference.get("field").and_then(JsonValue::as_str) != Some(field)
            || reference.get("kind").and_then(JsonValue::as_str) != Some(kind)
            || reference.get("path").and_then(JsonValue::as_str) != Some(path)
            || reference.get("sourceEncoding").and_then(JsonValue::as_str) != Some("plain")
            || !optional_f64_bits_equal(reference_sample_rate, source_sample_rate)
        {
            return Err(format!("{recovery_context} reference identity differs"));
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
            .ok_or_else(|| format!("{recovery_context} sibling reference clip is absent"))?;
        if std::ptr::eq(reference_animation, animation) {
            return Err(format!("{recovery_context} references its own clip"));
        }
        let actual_reference_rate = exact_nullable_f64(
            reference_animation.get("sampleRate"),
            &format!("{recovery_context}.referenceAnimation.sampleRate"),
        )?;
        if !optional_f64_bits_equal(actual_reference_rate, source_sample_rate) {
            return Err(format!("{recovery_context} sibling sample rate differs"));
        }
        let reference_tracks = animation_tracks(reference_animation, kind);
        let reference_track = reference_tracks
            .iter()
            .find(|track| {
                track.get("sourceEncoding").and_then(JsonValue::as_str) == Some("plain")
                    && track.get("sourceIndex").and_then(JsonValue::as_u64)
                        == u64::try_from(reference_source_index).ok()
            })
            .ok_or_else(|| format!("{recovery_context} sibling reference track is absent"))?;
        let reference_duration = exact_required_duration(
            reference_animation,
            "duration",
            &format!("{recovery_context}.referenceAnimation"),
        )?;
        let (_, reference_payload) = validate_constant_recovery_track(
            reference_track,
            kind,
            path,
            reference_duration,
            &format!("{recovery_context}.referenceTrack"),
        )?;
        if raw_target_binding_count(reference_animation, kind, path) != 1
            || reference_payload != canonical_payload
        {
            return Err(format!(
                "{recovery_context} sibling is not a single exact constant authority"
            ));
        }

        let mut matching_pairs = Vec::<(usize, &JsonValue, usize)>::new();
        for candidate_animation in animations {
            if std::ptr::eq(candidate_animation, animation)
                || exact_nullable_f64(
                    candidate_animation.get("sampleRate"),
                    "candidate sampleRate",
                )
                .ok()
                .is_none_or(|rate| !optional_f64_bits_equal(rate, source_sample_rate))
                || raw_target_binding_count(candidate_animation, kind, path) != 1
            {
                continue;
            }
            let candidate_duration = match exact_required_duration(
                candidate_animation,
                "duration",
                "candidate duration",
            ) {
                Ok(value) => value,
                Err(_) => continue,
            };
            for candidate_track in animation_tracks(candidate_animation, kind) {
                if candidate_track.get("path").and_then(JsonValue::as_str) != Some(path) {
                    continue;
                }
                let Ok((candidate_source_index, candidate_payload)) =
                    validate_constant_recovery_track(
                        candidate_track,
                        kind,
                        path,
                        candidate_duration,
                        "candidate constant track",
                    )
                else {
                    continue;
                };
                for (target_source_index, target_payload) in &source_payloads {
                    if target_payload == &candidate_payload {
                        matching_pairs.push((
                            *target_source_index,
                            candidate_animation,
                            candidate_source_index,
                        ));
                    }
                }
            }
        }
        let declared_pair_matches = matching_pairs.iter().any(
            |(target_source_index, candidate_animation, candidate_source_index)| {
                *target_source_index == canonical_source_index
                    && *candidate_source_index == reference_source_index
                    && std::ptr::eq(*candidate_animation, reference_animation)
            },
        );
        if matching_pairs.len() != 1 || !declared_pair_matches {
            return Err(format!(
                "{recovery_context} does not have one unique candidate/reference authority"
            ));
        }
        let proof = recovery
            .get("proof")
            .ok_or_else(|| format!("{recovery_context} has no proof"))?;
        if proof.get("exactPath").and_then(JsonValue::as_bool) != Some(true)
            || proof.get("exactSampleRate").and_then(JsonValue::as_bool) != Some(true)
            || proof
                .get("allSourceCurvesConstant")
                .and_then(JsonValue::as_bool)
                != Some(true)
            || proof
                .get("canonicalMatchesReference")
                .and_then(JsonValue::as_bool)
                != Some(true)
            || proof
                .get("uniqueCanonicalCandidateCount")
                .and_then(JsonValue::as_u64)
                != Some(1)
            || proof
                .get("matchingReferenceCount")
                .and_then(JsonValue::as_u64)
                != Some(1)
            || proof
                .get("sourceTargetCurveCount")
                .and_then(JsonValue::as_u64)
                != u64::try_from(source_indices.len()).ok()
        {
            return Err(format!(
                "{recovery_context} proof differs from recomputed facts"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_constant_recovery_track(
    track: &JsonValue,
    kind: &str,
    path: &str,
    duration: f64,
    context: &str,
) -> Result<(usize, JsonValue), String> {
    if track.get("path").and_then(JsonValue::as_str) != Some(path)
        || track.get("sourceEncoding").and_then(JsonValue::as_str) != Some("plain")
    {
        return Err(format!(
            "{context} is not the exact {kind} plain target {path:?}"
        ));
    }
    let source_index = track
        .get("sourceIndex")
        .and_then(JsonValue::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("{context} has invalid sourceIndex"))?;
    let keys = track
        .get("keys")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| format!("{context} has no keys"))?;
    let duplicate_keys = track
        .get("duplicateKeys")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| format!("{context} has no typed duplicateKeys"))?;
    let source_key_count = track
        .get("sourceKeyCount")
        .and_then(JsonValue::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("{context} has invalid sourceKeyCount"))?;
    if keys.is_empty() || !duplicate_keys.is_empty() || source_key_count != keys.len() {
        return Err(format!(
            "{context} is not a complete non-duplicate constant curve"
        ));
    }
    let mut previous_time = None::<f64>;
    let mut constant_payload = None::<JsonValue>;
    for (key_index, key) in keys.iter().enumerate() {
        if !constant_key_has_zero_tangents(key) {
            return Err(format!("{context} has non-zero Hermite tangents"));
        }
        if key.get("sourceKeyIndex").and_then(JsonValue::as_u64) != u64::try_from(key_index).ok() {
            return Err(format!(
                "{context} key provenance is not complete source order"
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
            .ok_or_else(|| format!("{context} has invalid key time"))?;
        if previous_time.is_some_and(|previous| previous >= time) {
            return Err(format!("{context} key times are not strict"));
        }
        previous_time = Some(time);
        let mut payload = key.clone();
        let object = payload
            .as_object_mut()
            .ok_or_else(|| format!("{context} key is not an object"))?;
        object.remove("time");
        object.remove("sourceKeyIndex");
        if constant_payload
            .as_ref()
            .is_some_and(|constant| constant != &payload)
        {
            return Err(format!("{context} is not constant"));
        }
        constant_payload.get_or_insert(payload);
    }
    Ok((
        source_index,
        constant_payload.expect("non-empty keys have a payload"),
    ))
}
