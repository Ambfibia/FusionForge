use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn audit_curve_recoveries(
    values: &[Value],
    node_count: usize,
    hierarchy_paths: &[Option<String>],
    source_channels: &[AuditedAnimationChannel],
    clip_duration: Option<f64>,
    clip_sample_rate: Option<f64>,
    relative: &str,
    context: &str,
    features: &mut LogicalModelFeatureCounts,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    let mut rejected_identities = BTreeSet::new();
    let mut previous_order = None;
    for (recovery_index, value) in values.iter().enumerate() {
        let recovery: AnimationCurveRecovery = match serde_json::from_value(value.clone()) {
            Ok(recovery) => recovery,
            Err(error) => {
                push_violation(
                    violations,
                    "invalid_animation_curve_recovery",
                    relative,
                    format!("{context} curve recovery {recovery_index} is not typed: {error}"),
                );
                continue;
            }
        };
        let recovery_context = format!("{context} curve recovery {recovery_index}");
        let target = recovery.target_node as usize;
        audit_curve_recovery_target(
            &recovery,
            target,
            node_count,
            hierarchy_paths,
            relative,
            &recovery_context,
            violations,
        );
        let rank = trs_source_rank(recovery.kind, recovery.source_encoding);
        if recovery.source_encoding != EmptyTrsSourceEncoding::Plain
            || recovery.source.source_encoding != EmptyTrsSourceEncoding::Plain
            || rank.is_none()
        {
            push_violation(
                violations,
                "invalid_animation_curve_recovery_encoding",
                relative,
                format!("{recovery_context} is only proven for plain TRS curves"),
            );
        }
        let expected_field = match recovery.kind {
            EmptyTrsBindingKind::Translation => "m_PositionCurves",
            EmptyTrsBindingKind::Rotation => "m_RotationCurves",
            EmptyTrsBindingKind::Scale => "m_ScaleCurves",
        };
        if recovery.source.field != expected_field
            || recovery.reference.field != expected_field
            || recovery.reference.kind != recovery.kind
            || recovery.reference.path != recovery.target_path
            || recovery.reference.source_encoding != recovery.source_encoding
        {
            push_violation(
                violations,
                "invalid_animation_curve_recovery_identity",
                relative,
                format!("{recovery_context} source/reference identity differs"),
            );
        }
        let source_indices = &recovery.source.source_indices;
        if source_indices.len() < 2
            || source_indices.windows(2).any(|pair| pair[0] >= pair[1])
            || recovery.source.source_target_curve_count != to_u64(source_indices.len())
        {
            push_violation(
                violations,
                "invalid_animation_curve_recovery_source_indices",
                relative,
                format!("{recovery_context} source indices/count are incomplete"),
            );
        }
        if let (Some(rank), Some(first)) = (rank, source_indices.first().copied()) {
            let order = (rank, first);
            if previous_order.is_some_and(|previous| previous >= order) {
                push_violation(
                    violations,
                    "noncanonical_animation_curve_recovery_order",
                    relative,
                    format!("{recovery_context} is out of source order"),
                );
            }
            previous_order = Some(order);
        }
        if !same_optional_f64_bits(recovery.source_sample_rate, clip_sample_rate)
            || !same_optional_f64_bits(recovery.reference.sample_rate, recovery.source_sample_rate)
        {
            push_violation(
                violations,
                "animation_curve_recovery_sample_rate_mismatch",
                relative,
                format!("{recovery_context} sample rates are not bit-exact"),
            );
        }
        // Raw object ownership was checked by the source publisher. Native
        // recovery data retains the clip, target, ordering and key proof.
        validate_true_name(
            &recovery.reference.clip_name,
            "curve recovery reference animation",
            relative,
            violations,
        );

        let duration = clip_duration.filter(|duration| {
            duration.is_finite() && (*duration as f32).is_finite() && *duration >= 0.0
        });
        let canonical_payload = duration.and_then(|duration| {
            audit_constant_curve_recovery_keys(
                &recovery.canonical.keys,
                &recovery.canonical.duplicate_keys,
                recovery.canonical.source_key_count,
                recovery.canonical.source_index,
                &recovery.canonical.path,
                duration,
            )
        });
        if canonical_payload.is_none()
            || recovery.canonical.path != recovery.target_path
            || recovery.canonical.source_encoding != EmptyTrsSourceEncoding::Plain
            || !source_indices.contains(&recovery.canonical.source_index)
        {
            push_violation(
                violations,
                "invalid_animation_curve_recovery_canonical",
                relative,
                format!("{recovery_context} canonical constant provenance is invalid"),
            );
        }
        let canonical = source_channels
            .iter()
            .filter(|channel| channel.kind == recovery.kind)
            .nth(recovery.canonical.track_index as usize);
        if canonical.is_none_or(|channel| {
            channel.target_node != target
                || channel.source_index != recovery.canonical.source_index
                || channel.source_encoding != recovery.canonical.source_encoding
                || channel.source_key_count != recovery.canonical.source_key_count
                || channel.duplicate_key_count != 0
                || !recovery_sampler_times_match(&recovery.canonical.keys, &channel.times)
        }) {
            push_violation(
                violations,
                "unresolved_animation_curve_recovery_channel",
                relative,
                format!("{recovery_context} canonical metadata differs from the GLB channel"),
            );
        }

        let expected_rejected = source_indices
            .iter()
            .copied()
            .filter(|source_index| *source_index != recovery.canonical.source_index)
            .collect::<Vec<_>>();
        if recovery.rejected.len() != expected_rejected.len() {
            push_violation(
                violations,
                "incomplete_animation_curve_recovery_rejected",
                relative,
                format!("{recovery_context} rejected source list is incomplete"),
            );
        }
        let mut conflicting = false;
        for (rejected_index, (track, expected_source_index)) in
            recovery.rejected.iter().zip(expected_rejected).enumerate()
        {
            features.rejected_conflicting_trs_bindings += 1;
            features.rejected_conflicting_trs_keyframes += u64::from(track.source_key_count);
            let payload = duration.and_then(|duration| {
                audit_constant_curve_recovery_keys(
                    &track.keys,
                    &track.duplicate_keys,
                    track.source_key_count,
                    track.source_index,
                    &track.path,
                    duration,
                )
            });
            conflicting |=
                payload.is_some() && canonical_payload.is_some() && payload != canonical_payload;
            let identity = (recovery.kind, track.source_encoding, track.source_index);
            if track.source_index != expected_source_index
                || track.path != recovery.target_path
                || track.source_encoding != EmptyTrsSourceEncoding::Plain
                || payload.is_none()
                || !rejected_identities.insert(identity)
            {
                push_violation(
                    violations,
                    "invalid_animation_curve_recovery_rejected",
                    relative,
                    format!(
                        "{recovery_context} rejected track {rejected_index} is invalid or repeated"
                    ),
                );
            }
        }
        if recovery.rejected.is_empty() || !conflicting {
            push_violation(
                violations,
                "nonconflicting_animation_curve_recovery",
                relative,
                format!("{recovery_context} has no conflicting rejected source"),
            );
        }
        if !recovery.proof.exact_path
            || !recovery.proof.exact_sample_rate
            || !recovery.proof.all_source_curves_constant
            || !recovery.proof.canonical_matches_reference
            || recovery.proof.unique_canonical_candidate_count != 1
            || recovery.proof.matching_reference_count != 1
            || recovery.proof.source_target_curve_count != to_u64(source_indices.len())
        {
            push_violation(
                violations,
                "invalid_animation_curve_recovery_proof",
                relative,
                format!("{recovery_context} proof is incomplete or ambiguous"),
            );
        }
    }
}

pub(super) fn audit_constant_curve_recovery_keys(
    keys: &DuplicateTrsKeys,
    duplicate_keys: &[DuplicateAnimationKey],
    source_key_count: u32,
    source_index: u32,
    path: &str,
    duration: f64,
) -> Option<Value> {
    if path.is_empty()
        || source_key_count as usize != keys.len()
        || keys.is_empty()
        || !duplicate_keys.is_empty()
    {
        return None;
    }
    let serialized = match keys {
        DuplicateTrsKeys::Vec3(keys) => keys
            .iter()
            .map(serde_json::to_value)
            .collect::<std::result::Result<Vec<_>, _>>()
            .ok()?,
        DuplicateTrsKeys::Quaternion(keys) => keys
            .iter()
            .map(serde_json::to_value)
            .collect::<std::result::Result<Vec<_>, _>>()
            .ok()?,
    };
    let mut previous = None;
    let mut payload = None;
    for (key_index, mut key) in serialized.into_iter().enumerate() {
        let key_object = key.as_object_mut()?;
        let actual_source_index = key_object.remove("sourceKeyIndex")?.as_u64()?;
        let time = key_object.remove("time")?.as_f64()?;
        if actual_source_index != key_index as u64
            || !time.is_finite()
            || !(time as f32).is_finite()
            || time < 0.0
            || time > duration + 1.0e-6
            || previous.is_some_and(|previous| previous >= time)
        {
            return None;
        }
        previous = Some(time);
        let current = Value::Object(key_object.clone());
        if payload
            .as_ref()
            .is_some_and(|previous| previous != &current)
        {
            return None;
        }
        payload.get_or_insert(current);
    }
    let _ = source_index;
    payload
}

pub(super) fn audit_curve_recovery_target(
    recovery: &AnimationCurveRecovery,
    target: usize,
    node_count: usize,
    hierarchy_paths: &[Option<String>],
    relative: &str,
    context: &str,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    if target >= node_count {
        push_violation(
            violations,
            "invalid_animation_curve_recovery_target",
            relative,
            format!("{context} target is out of bounds"),
        );
        return;
    }
    let path = &recovery.target_path;
    if path.is_empty()
        || path.trim_matches('/') != path
        || path.contains('\\')
        || path.contains("//")
        || path.chars().any(char::is_control)
    {
        push_violation(
            violations,
            "invalid_animation_curve_recovery_path",
            relative,
            format!("{context} source path is invalid"),
        );
        return;
    }
    let suffix = format!("/{path}");
    let matches = hierarchy_paths
        .iter()
        .enumerate()
        .filter_map(|(node, candidate)| {
            candidate.as_ref().and_then(|candidate| {
                (candidate == path || candidate.ends_with(&suffix)).then_some(node)
            })
        })
        .collect::<Vec<_>>();
    if matches.as_slice() != [target] {
        push_violation(
            violations,
            "unresolved_animation_curve_recovery_path",
            relative,
            format!("{context} path does not resolve uniquely to target"),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn audit_time_recoveries(
    values: &[Value],
    node_count: usize,
    hierarchy_paths: &[Option<String>],
    source_channels: &[AuditedAnimationChannel],
    clip_duration: Option<f64>,
    clip_sample_rate: Option<f64>,
    relative: &str,
    context: &str,
    features: &mut LogicalModelFeatureCounts,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    let mut identities = BTreeSet::new();
    let mut previous_order = None;
    for (recovery_index, value) in values.iter().enumerate() {
        let recovery: AnimationTimeRecovery = match serde_json::from_value(value.clone()) {
            Ok(recovery) => recovery,
            Err(error) => {
                push_violation(
                    violations,
                    "invalid_animation_time_recovery",
                    relative,
                    format!("{context} time recovery {recovery_index} is not typed: {error}"),
                );
                continue;
            }
        };
        features.recovered_animation_keyframes += to_u64(recovery.recovered_times.len());
        let recovery_context = format!("{context} time recovery {recovery_index}");
        let target = recovery.target_node as usize;
        audit_time_recovery_target(
            &recovery,
            target,
            node_count,
            hierarchy_paths,
            relative,
            &recovery_context,
            violations,
        );

        let rank = trs_source_rank(recovery.kind, recovery.source_encoding);
        if recovery.source_encoding != EmptyTrsSourceEncoding::Plain || rank.is_none() {
            push_violation(
                violations,
                "invalid_animation_time_recovery_encoding",
                relative,
                format!("{recovery_context} is only proven for plain TRS curves"),
            );
        }
        let identity = (
            recovery.kind,
            recovery.source_encoding,
            recovery.source_index,
        );
        if !identities.insert(identity) {
            push_violation(
                violations,
                "duplicate_animation_time_recovery_identity",
                relative,
                format!("{recovery_context} repeats a recovered source identity"),
            );
        }
        if let Some(rank) = rank {
            let order = (rank, recovery.source_index);
            if previous_order.is_some_and(|previous| previous >= order) {
                push_violation(
                    violations,
                    "noncanonical_animation_time_recovery_order",
                    relative,
                    format!("{recovery_context} is out of source order"),
                );
            }
            previous_order = Some(order);
        }

        let duration = clip_duration.filter(|duration| {
            duration.is_finite() && (*duration as f32).is_finite() && *duration >= 0.0
        });
        let original_times_valid = duration.is_some_and(|duration| {
            !recovery.original_times.is_empty()
                && recovery.original_times.len() == recovery.recovered_times.len()
                && recovery.original_times.iter().all(|time| {
                    time.is_finite()
                        && (*time as f32).is_finite()
                        && *time >= 0.0
                        && *time <= duration + 1.0e-6
                })
        });
        if !original_times_valid
            || recovery
                .original_times
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        {
            push_violation(
                violations,
                "invalid_animation_time_recovery_original_times",
                relative,
                format!("{recovery_context} originalTimes are invalid or were already strict"),
            );
        }
        let recovered_times_valid = duration.is_some_and(|duration| {
            !recovery.recovered_times.is_empty()
                && recovery.recovered_times.iter().all(|time| {
                    time.is_finite()
                        && (*time as f32).is_finite()
                        && *time >= 0.0
                        && *time <= duration + 1.0e-6
                })
                && recovery
                    .recovered_times
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
        });
        if !recovered_times_valid {
            push_violation(
                violations,
                "invalid_animation_time_recovery_recovered_times",
                relative,
                format!("{recovery_context} recoveredTimes are not finite and strict"),
            );
        }

        if !same_optional_f64_bits(recovery.source_sample_rate, clip_sample_rate)
            || !same_optional_f64_bits(recovery.reference.sample_rate, recovery.source_sample_rate)
            || [recovery.source_sample_rate, recovery.reference.sample_rate]
                .into_iter()
                .flatten()
                .any(|rate| !rate.is_finite() || !(rate as f32).is_finite() || rate <= 0.0)
        {
            push_violation(
                violations,
                "animation_time_recovery_sample_rate_mismatch",
                relative,
                format!("{recovery_context} sample rates are invalid or not bit-exact"),
            );
        }
        validate_true_name(
            &recovery.reference.asset,
            "time recovery reference asset",
            relative,
            violations,
        );
        validate_true_name(
            &recovery.reference.clip_name,
            "time recovery reference animation",
            relative,
            violations,
        );
        if recovery.reference.path_id <= 0
            || recovery.reference.kind != recovery.kind
            || recovery.reference.path != recovery.target_path
            || recovery.reference.source_encoding != recovery.source_encoding
        {
            push_violation(
                violations,
                "invalid_animation_time_recovery_reference",
                relative,
                format!("{recovery_context} reference identity differs from recovered curve"),
            );
        }
        if !recovery.proof.exact_key_payload_excluding_time
            || !recovery.proof.exact_path
            || !recovery.proof.exact_sample_rate
            || recovery.proof.unique_recovered_time_vector_count != 1
            || recovery.proof.matching_reference_count == 0
        {
            push_violation(
                violations,
                "invalid_animation_time_recovery_proof",
                relative,
                format!("{recovery_context} proof is incomplete or ambiguous"),
            );
        }

        let canonical = source_channels
            .iter()
            .filter(|channel| {
                channel.kind == recovery.kind
                    && channel.target_node == target
                    && channel.source_encoding == recovery.source_encoding
                    && channel.source_index == recovery.source_index
            })
            .collect::<Vec<_>>();
        let [canonical] = canonical.as_slice() else {
            push_violation(
                violations,
                "unresolved_animation_time_recovery_channel",
                relative,
                format!("{recovery_context} does not resolve to exactly one canonical GLB channel"),
            );
            continue;
        };
        if canonical.source_key_count as usize != recovery.recovered_times.len()
            || canonical.duplicate_key_count != 0
            || canonical.source_key_indices.len() != recovery.recovered_times.len()
            || canonical.source_key_indices.len() != canonical.times.len()
        {
            push_violation(
                violations,
                "animation_time_recovery_channel_count_mismatch",
                relative,
                format!(
                    "{recovery_context} sourceKeyCount/channel alignment differs from recoveredTimes"
                ),
            );
            continue;
        }
        if canonical
            .source_key_indices
            .iter()
            .zip(&canonical.times)
            .any(|(&source_key_index, &time)| {
                recovery
                    .recovered_times
                    .get(source_key_index as usize)
                    .is_none_or(|recovered| (*recovered as f32).to_bits() != time.to_bits())
            })
        {
            push_violation(
                violations,
                "animation_time_recovery_sampler_mismatch",
                relative,
                format!(
                    "{recovery_context} recoveredTimes differ from the actual GLB sampler input"
                ),
            );
        }
    }
}

pub(super) fn audit_time_recovery_target(
    recovery: &AnimationTimeRecovery,
    target: usize,
    node_count: usize,
    hierarchy_paths: &[Option<String>],
    relative: &str,
    context: &str,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    if target >= node_count {
        push_violation(
            violations,
            "invalid_animation_time_recovery_target",
            relative,
            format!("{context} target is out of bounds"),
        );
        return;
    }
    if recovery.target_path.is_empty()
        || recovery.target_path.trim_matches('/') != recovery.target_path
        || recovery.target_path.contains('\\')
        || recovery.target_path.contains("//")
        || recovery.target_path.chars().any(char::is_control)
    {
        push_violation(
            violations,
            "invalid_animation_time_recovery_path",
            relative,
            format!("{context} source path is invalid"),
        );
        return;
    }
    let suffix = format!("/{}", recovery.target_path);
    let matches = hierarchy_paths
        .iter()
        .enumerate()
        .filter_map(|(node, candidate)| {
            candidate.as_ref().and_then(|candidate| {
                (candidate == &recovery.target_path || candidate.ends_with(&suffix)).then_some(node)
            })
        })
        .collect::<Vec<_>>();
    if matches.as_slice() != [target] {
        push_violation(
            violations,
            "unresolved_animation_time_recovery_path",
            relative,
            format!("{context} source path does not resolve uniquely to target"),
        );
    }
}

pub(super) fn audit_channel_key_provenance(
    channel: &Value,
    canonical_key_count: Option<usize>,
    relative: &str,
    context: &str,
    features: &mut LogicalModelFeatureCounts,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    let source_key_count = channel
        .pointer("/extras/sourceKeyCount")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok());
    let source_key_indices = channel
        .pointer("/extras/sourceKeyIndices")
        .and_then(Value::as_array);
    let duplicate_values = channel
        .pointer("/extras/duplicateKeys")
        .and_then(Value::as_array);
    let (Some(source_key_count), Some(source_key_indices), Some(duplicate_values)) =
        (source_key_count, source_key_indices, duplicate_values)
    else {
        push_violation(
            violations,
            "missing_animation_key_provenance",
            relative,
            format!("{context} has no typed sourceKeyCount/sourceKeyIndices/duplicateKeys"),
        );
        return;
    };
    if canonical_key_count != Some(source_key_indices.len()) {
        push_violation(
            violations,
            "animation_key_provenance_count_mismatch",
            relative,
            format!("{context} sourceKeyIndices do not align with sampler inputs"),
        );
    }
    let mut identities = BTreeSet::new();
    let mut previous = None;
    for (index, value) in source_key_indices.iter().enumerate() {
        let source_index = value.as_u64().and_then(|value| usize::try_from(value).ok());
        if source_index.is_none_or(|source_index| {
            source_index >= source_key_count
                || previous.is_some_and(|previous| previous >= source_index)
                || !identities.insert(source_index)
        }) {
            push_violation(
                violations,
                "invalid_animation_source_key_index",
                relative,
                format!("{context} sourceKeyIndices[{index}] is invalid/out of order"),
            );
        }
        previous = source_index;
    }
    previous = None;
    for (index, value) in duplicate_values.iter().enumerate() {
        let duplicate: DuplicateAnimationKey = match serde_json::from_value(value.clone()) {
            Ok(duplicate) => duplicate,
            Err(error) => {
                push_violation(
                    violations,
                    "invalid_duplicate_animation_key",
                    relative,
                    format!("{context} duplicateKeys[{index}] is not typed: {error}"),
                );
                continue;
            }
        };
        let source_index = duplicate.source_key_index as usize;
        if source_index >= source_key_count
            || previous.is_some_and(|previous| previous >= source_index)
            || !identities.insert(source_index)
            || duplicate.canonical_key_index as usize >= source_key_indices.len()
        {
            push_violation(
                violations,
                "invalid_duplicate_animation_key_identity",
                relative,
                format!("{context} duplicateKeys[{index}] identity/link is invalid"),
            );
        }
        previous = Some(source_index);
    }
    if identities.len() != source_key_count
        || (0..source_key_count).any(|source_index| !identities.contains(&source_index))
    {
        push_violation(
            violations,
            "incomplete_animation_source_key_coverage",
            relative,
            format!("{context} does not fully cover sourceKeyCount"),
        );
    }
    features.duplicate_same_time_keys += to_u64(duplicate_values.len());
}
