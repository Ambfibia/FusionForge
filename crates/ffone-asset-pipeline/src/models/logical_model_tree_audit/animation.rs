use super::*;

#[derive(Clone, Debug)]
pub(super) struct AuditedAnimationChannel {
    pub(super) kind: EmptyTrsBindingKind,
    pub(super) target_node: usize,
    pub(super) source_index: u32,
    pub(super) source_encoding: EmptyTrsSourceEncoding,
    pub(super) source_key_count: u32,
    pub(super) source_key_indices: Vec<u32>,
    pub(super) times: Vec<f32>,
    pub(super) duplicate_key_count: usize,
}

pub(super) fn audit_animation_metadata(
    metadata: Option<&Value>,
    node_count: usize,
    hierarchy_paths: &[Option<String>],
    standard_targets: &BTreeSet<(usize, EmptyTrsBindingKind)>,
    source_channels: &[AuditedAnimationChannel],
    clip_duration: Option<f64>,
    clip_sample_rate: Option<f64>,
    relative: &str,
    context: &str,
    features: &mut LogicalModelFeatureCounts,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    let Some(metadata) = metadata else {
        push_violation(
            violations,
            "missing_animation_metadata",
            relative,
            format!("{context} has no exact non-TRS metadata"),
        );
        return;
    };
    if metadata
        .get("unsupported")
        .and_then(Value::as_array)
        .is_some_and(|values| !values.is_empty())
    {
        push_violation(
            violations,
            "unsupported_animation_bindings",
            relative,
            format!("{context} still contains unsupported source bindings"),
        );
    }
    let Some(empty_bindings) = metadata.get("emptyTrsBindings").and_then(Value::as_array) else {
        push_violation(
            violations,
            "missing_empty_trs_bindings",
            relative,
            format!("{context} has no typed emptyTrsBindings array"),
        );
        return;
    };
    features.empty_trs_bindings += to_u64(empty_bindings.len());
    let mut identities = BTreeSet::new();
    let mut previous_order = None;
    for (binding_index, value) in empty_bindings.iter().enumerate() {
        let binding: EmptyTrsBinding = match serde_json::from_value(value.clone()) {
            Ok(binding) => binding,
            Err(error) => {
                push_violation(
                    violations,
                    "invalid_empty_trs_binding",
                    relative,
                    format!("{context} empty TRS binding {binding_index} is not typed: {error}"),
                );
                continue;
            }
        };
        let target = binding.target_node as usize;
        if target >= node_count {
            push_violation(
                violations,
                "invalid_empty_trs_target",
                relative,
                format!("{context} empty TRS binding {binding_index} target is out of bounds"),
            );
            continue;
        }
        if binding.target_path.trim_matches('/') != binding.target_path
            || binding.target_path.contains('\\')
            || binding.target_path.contains("//")
            || binding.target_path.chars().any(char::is_control)
        {
            push_violation(
                violations,
                "invalid_empty_trs_path",
                relative,
                format!("{context} empty TRS binding {binding_index} path is invalid"),
            );
        } else if binding.target_path.is_empty() {
            let roots = hierarchy_paths
                .iter()
                .enumerate()
                .filter_map(|(node, candidate)| {
                    candidate
                        .as_ref()
                        .filter(|candidate| !candidate.contains('/'))
                        .map(|_| node)
                })
                .collect::<Vec<_>>();
            if roots.as_slice() != [target] {
                push_violation(
                    violations,
                    "unresolved_empty_trs_path",
                    relative,
                    format!(
                        "{context} empty TRS binding {binding_index} root path does not resolve uniquely to target"
                    ),
                );
            }
        } else {
            let suffix = format!("/{}", binding.target_path);
            let matches = hierarchy_paths
                .iter()
                .enumerate()
                .filter_map(|(node, candidate)| {
                    candidate.as_ref().and_then(|candidate| {
                        (candidate == &binding.target_path || candidate.ends_with(&suffix))
                            .then_some(node)
                    })
                })
                .collect::<Vec<_>>();
            if matches.as_slice() != [target] {
                push_violation(
                    violations,
                    "unresolved_empty_trs_path",
                    relative,
                    format!(
                        "{context} empty TRS binding {binding_index} path does not resolve uniquely to target"
                    ),
                );
            }
        }
        let identity = (binding.kind, binding.source_encoding, binding.source_index);
        if !identities.insert(identity) {
            push_violation(
                violations,
                "duplicate_empty_trs_identity",
                relative,
                format!("{context} empty TRS binding {binding_index} repeats source identity"),
            );
        }
        let order = match (binding.kind, binding.source_encoding) {
            (EmptyTrsBindingKind::Translation, EmptyTrsSourceEncoding::Plain) => Some(0u8),
            (EmptyTrsBindingKind::Rotation, EmptyTrsSourceEncoding::Plain) => Some(1),
            (EmptyTrsBindingKind::Rotation, EmptyTrsSourceEncoding::Compressed) => Some(2),
            (EmptyTrsBindingKind::Scale, EmptyTrsSourceEncoding::Plain) => Some(3),
            (_, EmptyTrsSourceEncoding::Compressed) => None,
        };
        if let Some(order) = order {
            let current = (order, binding.source_index);
            if previous_order.is_some_and(|previous| previous >= current) {
                push_violation(
                    violations,
                    "noncanonical_empty_trs_order",
                    relative,
                    format!("{context} empty TRS binding {binding_index} is out of source order"),
                );
            }
            previous_order = Some(current);
        } else {
            push_violation(
                violations,
                "invalid_empty_trs_encoding",
                relative,
                format!("{context} empty TRS binding {binding_index} encoding is impossible"),
            );
        }
        if standard_targets.contains(&(target, binding.kind)) {
            push_violation(
                violations,
                "empty_trs_standard_collision",
                relative,
                format!("{context} empty TRS binding {binding_index} collides with a channel"),
            );
        }
    }
    let Some(duplicate_bindings) = metadata
        .get("duplicateTrsBindings")
        .and_then(Value::as_array)
    else {
        push_violation(
            violations,
            "missing_duplicate_trs_bindings",
            relative,
            format!("{context} has no typed duplicateTrsBindings array"),
        );
        return;
    };
    features.duplicate_trs_bindings += to_u64(duplicate_bindings.len());
    previous_order = None;
    for (binding_index, value) in duplicate_bindings.iter().enumerate() {
        let binding: DuplicateTrsBinding = match serde_json::from_value(value.clone()) {
            Ok(binding) => binding,
            Err(error) => {
                push_violation(
                    violations,
                    "invalid_duplicate_trs_binding",
                    relative,
                    format!(
                        "{context} duplicate TRS binding {binding_index} is not typed: {error}"
                    ),
                );
                continue;
            }
        };
        let target = binding.target_node as usize;
        if target >= node_count || !standard_targets.contains(&(target, binding.kind)) {
            push_violation(
                violations,
                "invalid_duplicate_trs_target",
                relative,
                format!(
                    "{context} duplicate TRS binding {binding_index} has no canonical channel target"
                ),
            );
        }
        if binding.target_path.is_empty()
            || binding.target_path.trim_matches('/') != binding.target_path
            || binding.target_path.contains('\\')
            || binding.target_path.contains("//")
            || binding.target_path.chars().any(char::is_control)
        {
            push_violation(
                violations,
                "invalid_duplicate_trs_path",
                relative,
                format!("{context} duplicate TRS binding {binding_index} path is invalid"),
            );
        } else {
            let suffix = format!("/{}", binding.target_path);
            let matches = hierarchy_paths
                .iter()
                .enumerate()
                .filter_map(|(node, candidate)| {
                    candidate.as_ref().and_then(|candidate| {
                        (candidate == &binding.target_path || candidate.ends_with(&suffix))
                            .then_some(node)
                    })
                })
                .collect::<Vec<_>>();
            if matches.as_slice() != [target] {
                push_violation(
                    violations,
                    "unresolved_duplicate_trs_path",
                    relative,
                    format!(
                        "{context} duplicate TRS binding {binding_index} path does not resolve uniquely"
                    ),
                );
            }
        }
        let identity = (binding.kind, binding.source_encoding, binding.source_index);
        if !identities.insert(identity) {
            push_violation(
                violations,
                "duplicate_trs_source_identity",
                relative,
                format!("{context} duplicate TRS binding {binding_index} repeats source identity"),
            );
        }
        let rank = match (binding.kind, binding.source_encoding) {
            (EmptyTrsBindingKind::Translation, EmptyTrsSourceEncoding::Plain) => Some(0u8),
            (EmptyTrsBindingKind::Rotation, EmptyTrsSourceEncoding::Plain) => Some(1),
            (EmptyTrsBindingKind::Rotation, EmptyTrsSourceEncoding::Compressed) => Some(2),
            (EmptyTrsBindingKind::Scale, EmptyTrsSourceEncoding::Plain) => Some(3),
            (_, EmptyTrsSourceEncoding::Compressed) => None,
        };
        if let Some(rank) = rank {
            let current = (rank, binding.source_index);
            if previous_order.is_some_and(|previous| previous >= current) {
                push_violation(
                    violations,
                    "noncanonical_duplicate_trs_order",
                    relative,
                    format!("{context} duplicate TRS binding {binding_index} is out of order"),
                );
            }
            previous_order = Some(current);
        } else {
            push_violation(
                violations,
                "invalid_duplicate_trs_encoding",
                relative,
                format!("{context} duplicate TRS binding {binding_index} encoding is impossible"),
            );
        }
        let source_indices = match &binding.keys {
            DuplicateTrsKeys::Vec3(keys) => keys
                .iter()
                .map(|key| key.source_key_index)
                .collect::<Vec<_>>(),
            DuplicateTrsKeys::Quaternion(keys) => keys
                .iter()
                .map(|key| key.source_key_index)
                .collect::<Vec<_>>(),
        };
        if !typed_key_provenance_is_complete(
            &source_indices,
            &binding.duplicate_keys,
            binding.source_key_count,
        ) {
            push_violation(
                violations,
                "incomplete_duplicate_trs_key_coverage",
                relative,
                format!(
                    "{context} duplicate TRS binding {binding_index} does not fully cover sourceKeyCount"
                ),
            );
        }
        features.duplicate_trs_keyframes += u64::from(binding.source_key_count);
        features.duplicate_same_time_keys += to_u64(binding.duplicate_keys.len());
    }
    let Some(time_recoveries) = metadata.get("timeRecoveries").and_then(Value::as_array) else {
        push_violation(
            violations,
            "missing_animation_time_recoveries",
            relative,
            format!("{context} has no typed timeRecoveries array"),
        );
        return;
    };
    features.animation_time_recoveries += to_u64(time_recoveries.len());
    audit_time_recoveries(
        time_recoveries,
        node_count,
        hierarchy_paths,
        source_channels,
        clip_duration,
        clip_sample_rate,
        relative,
        context,
        features,
        violations,
    );
    let Some(curve_recoveries) = metadata.get("curveRecoveries").and_then(Value::as_array) else {
        push_violation(
            violations,
            "missing_animation_curve_recoveries",
            relative,
            format!("{context} has no typed curveRecoveries array"),
        );
        return;
    };
    features.animation_curve_recoveries += to_u64(curve_recoveries.len());
    audit_curve_recoveries(
        curve_recoveries,
        node_count,
        hierarchy_paths,
        source_channels,
        clip_duration,
        clip_sample_rate,
        relative,
        context,
        features,
        violations,
    );
    for (curve_index, curve) in metadata
        .get("floatCurves")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let target = curve
            .get("targetNode")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        let times = curve.get("times").and_then(Value::as_array);
        let values = curve.get("values").and_then(Value::as_array);
        if target.is_none_or(|target| target >= node_count)
            // A serialized Unity material binding may intentionally have no
            // keys. It is still exact animation metadata (Bubbie's stand3
            // `_MainTex.offset.x` binding is one such case), so coherence is
            // presence plus matching cardinality rather than non-emptiness.
            || times.is_none()
            || values.is_none()
            || times.map(Vec::len) != values.map(Vec::len)
        {
            push_violation(
                violations,
                "invalid_float_curve",
                relative,
                format!("{context} float curve {curve_index} is incoherent"),
            );
        }
    }
    for (curve_index, curve) in metadata
        .get("objectCurves")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let target = curve
            .get("targetNode")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        let keys = curve.get("keys").and_then(Value::as_array);
        if target.is_none_or(|target| target >= node_count) || keys.is_none_or(Vec::is_empty) {
            push_violation(
                violations,
                "invalid_object_curve",
                relative,
                format!("{context} object curve {curve_index} is incoherent"),
            );
        }
        features.object_reference_keys += keys.map_or(0, |keys| to_u64(keys.len()));
    }
    let Some(events) = metadata.get("events").and_then(Value::as_array) else {
        push_violation(
            violations,
            "missing_animation_events",
            relative,
            format!("{context} has no typed events array"),
        );
        return;
    };
    features.animation_events += to_u64(events.len());
    for (event_index, value) in events.iter().enumerate() {
        let event: AnimationEvent = match serde_json::from_value(value.clone()) {
            Ok(event) => event,
            Err(error) => {
                push_violation(
                    violations,
                    "invalid_animation_event",
                    relative,
                    format!("{context} event {event_index} is not typed: {error}"),
                );
                continue;
            }
        };
        let event_context = format!("{context} event {event_index}");
        if event.time < 0.0
            || !event.time.is_finite()
            || !(event.time as f32).is_finite()
            || clip_duration.is_none_or(|duration| event.time > duration + 1.0e-6)
        {
            push_violation(
                violations,
                "invalid_animation_event_time",
                relative,
                format!("{event_context} time is invalid"),
            );
        }
        match event.object_parameter_provenance {
            AnimationEventObjectParameterProvenance::Missing { .. } => {
                if event.object_parameter.is_some() {
                    push_violation(
                        violations,
                        "animation_event_object_provenance_mismatch",
                        relative,
                        format!("{event_context} missing provenance contradicts objectParameter"),
                    );
                }
            }
            AnimationEventObjectParameterProvenance::SerializedPointer {
                path_id,
                interpretation,
                ..
            } => match interpretation {
                SerializedEventObjectParameterInterpretation::NullPathId => {
                    if path_id != 0 || event.object_parameter.is_some() {
                        push_violation(
                            violations,
                            "animation_event_null_pointer_provenance_mismatch",
                            relative,
                            format!("{event_context} null-path-id provenance is inconsistent"),
                        );
                    } else {
                        features.animation_event_null_object_pointers += 1;
                    }
                }
                SerializedEventObjectParameterInterpretation::NonNullUnresolved => {
                    push_violation(
                        violations,
                        "unresolved_animation_event_object_pointer",
                        relative,
                        format!("{event_context} retains an unresolved non-null pointer"),
                    );
                }
                SerializedEventObjectParameterInterpretation::NonNullMetadataOnly => {
                    if path_id == 0
                        || event.object_parameter.is_some()
                        || event.function_name != "sound"
                        || event.string_parameter.trim().is_empty()
                    {
                        push_violation(
                            violations,
                            "animation_event_metadata_pointer_provenance_mismatch",
                            relative,
                            format!("{event_context} metadata-only sound pointer is inconsistent"),
                        );
                    }
                }
                SerializedEventObjectParameterInterpretation::NonNullLegacyUnused => {
                    if path_id == 0
                        || event.object_parameter.is_some()
                        || event.function_name != "end"
                        || !event.string_parameter.is_empty()
                    {
                        push_violation(
                            violations,
                            "animation_event_legacy_unused_pointer_provenance_mismatch",
                            relative,
                            format!("{event_context} legacy-unused end pointer is inconsistent"),
                        );
                    }
                }
            },
        }
    }
}

pub(super) fn audited_animation_channel(
    channel: &Value,
    kind: EmptyTrsBindingKind,
    target_node: usize,
    input: AccessorInfo,
    binary: &[u8],
) -> Option<AuditedAnimationChannel> {
    if input.component_type != 5_126 || input.component_count != 1 {
        return None;
    }
    let source_index = channel
        .pointer("/extras/sourceIndex")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())?;
    let source_encoding =
        serde_json::from_value(channel.pointer("/extras/sourceEncoding").cloned()?).ok()?;
    let source_key_count = channel
        .pointer("/extras/sourceKeyCount")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())?;
    let source_key_indices = channel
        .pointer("/extras/sourceKeyIndices")
        .and_then(Value::as_array)?
        .iter()
        .map(|value| value.as_u64().and_then(|value| u32::try_from(value).ok()))
        .collect::<Option<Vec<_>>>()?;
    let duplicate_key_count = channel
        .pointer("/extras/duplicateKeys")
        .and_then(Value::as_array)?
        .len();
    let times = (0..input.count)
        .map(|key| read_f32(input, binary, key, 0))
        .collect::<Option<Vec<_>>>()?;
    Some(AuditedAnimationChannel {
        kind,
        target_node,
        source_index,
        source_encoding,
        source_key_count,
        source_key_indices,
        times,
        duplicate_key_count,
    })
}

pub(super) fn valid_current_pose_bind_deviation(coordinate: &CoordinateAuditReport) -> bool {
    let (Some(maximum), Some(worst)) = (
        coordinate.current_pose_bind_identity_deviation_max,
        coordinate
            .current_pose_bind_identity_deviation_worst
            .as_ref(),
    ) else {
        return false;
    };
    maximum.is_finite()
        && maximum >= 0.0
        && worst.metric == "maxAbs(inverse(meshGlobal)*jointGlobal*inverseBind-identity)"
        && worst.max_abs_deviation == maximum
        && worst.max_abs_deviation.is_finite()
        && worst.joint_positive_weight_sum.is_finite()
        && worst.joint_positive_weight_sum >= 0.0
        && worst.max_element_row < 4
        && worst.max_element_column < 4
        && worst.max_element_actual.is_finite()
        && worst.max_element_identity.is_finite()
        && matches!(worst.max_element_identity, 0.0 | 1.0)
        && !worst.mesh_node_path.trim().is_empty()
        && !worst.mesh_name.trim().is_empty()
        && !worst.skin_name.trim().is_empty()
        && !worst.joint_node_path.trim().is_empty()
}
