use super::*;

pub(super) fn same_optional_source_f64_bits(left: Option<f64>, right: Option<f64>) -> bool {
    left.map(f64::to_bits) == right.map(f64::to_bits)
}

pub(super) fn track_kind(kind: EmptyTrsBindingKind) -> TrackKind {
    match kind {
        EmptyTrsBindingKind::Translation => TrackKind::Translation,
        EmptyTrsBindingKind::Rotation => TrackKind::Rotation,
        EmptyTrsBindingKind::Scale => TrackKind::Scale,
    }
}

pub(super) fn raw_trs_source(
    counts: &SourceCurveCounts,
    kind: EmptyTrsBindingKind,
    encoding: EmptyTrsSourceEncoding,
) -> Option<(u8, usize)> {
    match (kind, encoding) {
        (EmptyTrsBindingKind::Translation, EmptyTrsSourceEncoding::Plain) => {
            Some((0, counts.position))
        }
        (EmptyTrsBindingKind::Rotation, EmptyTrsSourceEncoding::Plain) => {
            Some((1, counts.rotation))
        }
        (EmptyTrsBindingKind::Rotation, EmptyTrsSourceEncoding::Compressed) => {
            Some((2, counts.compressed_rotation))
        }
        (EmptyTrsBindingKind::Scale, EmptyTrsSourceEncoding::Plain) => Some((3, counts.scale)),
        (_, EmptyTrsSourceEncoding::Compressed) => None,
    }
}

pub(super) fn same_optional_duration(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => same_duration(left, right),
        (None, None) => true,
        _ => false,
    }
}

pub(super) fn same_duration(left: f64, right: f64) -> bool {
    let tolerance = 1.0e-12 * left.abs().max(right.abs()).max(1.0);
    (left - right).abs() <= tolerance
}

pub(super) fn vec3_channel(
    track: &SourceVec3Track,
    kind: TrackKind,
    paths: &BTreeMap<String, u32>,
    root_path: &str,
) -> Result<AnimationChannel> {
    let interpolation = parse_interpolation(&track.interpolation)?;
    let target_node = animation_node_path(paths, root_path, &track.path, "animation track")?;
    let times = track.keys.iter().map(|key| key.time).collect::<Vec<_>>();
    let values = track.keys.iter().map(|key| key.value).collect::<Vec<_>>();
    let tangent_modes = tangent_modes(track.keys.iter().map(|key| key.tangent_mode), &track.path)?;
    let (in_tangents, out_tangents) = match interpolation {
        Interpolation::CubicSpline => (
            Some(TrackValues::from_vec3(
                kind,
                required_vec3_tangents(&track.keys, true, &track.path)?,
            )?),
            Some(TrackValues::from_vec3(
                kind,
                required_vec3_tangents(&track.keys, false, &track.path)?,
            )?),
        ),
        Interpolation::Linear | Interpolation::Step => {
            if track
                .keys
                .iter()
                .any(|key| key.in_tangent.is_some() || key.out_tangent.is_some())
            {
                return invalid(format!(
                    "non-cubic track {:?} contains source tangents",
                    track.path
                ));
            }
            (None, None)
        }
    };
    Ok(AnimationChannel {
        target_node,
        source_index: u32_index(track.source_index, "animation source index")?,
        source_encoding: track.source_encoding,
        source_key_count: u32_index(track.source_key_count, "animation source key count")?,
        source_key_indices: track
            .keys
            .iter()
            .map(|key| u32_index(key.source_key_index, "animation source key index"))
            .collect::<Result<Vec<_>>>()?,
        duplicate_keys: convert_duplicate_animation_keys(&track.duplicate_keys)?,
        interpolation,
        times,
        values: TrackValues::from_vec3(kind, values)?,
        in_tangents,
        out_tangents,
        tangent_modes,
    })
}

pub(super) fn quat_channel(
    track: &SourceQuatTrack,
    paths: &BTreeMap<String, u32>,
    root_path: &str,
) -> Result<AnimationChannel> {
    let interpolation = parse_interpolation(&track.interpolation)?;
    let target_node =
        animation_node_path(paths, root_path, &track.path, "animation rotation track")?;
    let times = track.keys.iter().map(|key| key.time).collect::<Vec<_>>();
    let values = track.keys.iter().map(|key| key.value).collect::<Vec<_>>();
    let tangent_modes = tangent_modes(track.keys.iter().map(|key| key.tangent_mode), &track.path)?;
    let (in_tangents, out_tangents) = match interpolation {
        Interpolation::CubicSpline => (
            Some(TrackValues::Rotation(required_quat_tangents(
                &track.keys,
                true,
                &track.path,
            )?)),
            Some(TrackValues::Rotation(required_quat_tangents(
                &track.keys,
                false,
                &track.path,
            )?)),
        ),
        Interpolation::Linear | Interpolation::Step => {
            if track
                .keys
                .iter()
                .any(|key| key.in_tangent.is_some() || key.out_tangent.is_some())
            {
                return invalid(format!(
                    "non-cubic rotation track {:?} contains source tangents",
                    track.path
                ));
            }
            (None, None)
        }
    };
    Ok(AnimationChannel {
        target_node,
        source_index: u32_index(track.source_index, "animation source index")?,
        source_encoding: track.source_encoding,
        source_key_count: u32_index(track.source_key_count, "animation source key count")?,
        source_key_indices: track
            .keys
            .iter()
            .map(|key| u32_index(key.source_key_index, "animation source key index"))
            .collect::<Result<Vec<_>>>()?,
        duplicate_keys: convert_duplicate_animation_keys(&track.duplicate_keys)?,
        interpolation,
        times,
        values: TrackValues::Rotation(values),
        in_tangents,
        out_tangents,
        tangent_modes,
    })
}

pub(super) fn exact_keys_last_time(keys: &DuplicateTrsKeys) -> Option<f64> {
    match keys {
        DuplicateTrsKeys::Vec3(keys) => keys.last().map(|key| key.time),
        DuplicateTrsKeys::Quaternion(keys) => keys.last().map(|key| key.time),
    }
}

pub(super) fn required_vec3_tangents(
    keys: &[SourceVec3Key],
    input: bool,
    path: &str,
) -> Result<Vec<[f64; 3]>> {
    keys.iter()
        .map(|key| {
            if input {
                key.in_tangent
            } else {
                key.out_tangent
            }
            .ok_or_else(|| invalid_error(format!("CUBICSPLINE track {path:?} is missing tangents")))
        })
        .collect()
}

pub(super) fn required_quat_tangents(
    keys: &[SourceQuatKey],
    input: bool,
    path: &str,
) -> Result<Vec<[f64; 4]>> {
    keys.iter()
        .map(|key| {
            if input {
                key.in_tangent
            } else {
                key.out_tangent
            }
            .ok_or_else(|| invalid_error(format!("CUBICSPLINE track {path:?} is missing tangents")))
        })
        .collect()
}

pub(super) fn tangent_modes(values: impl IntoIterator<Item = Option<i32>>, path: &str) -> Result<Vec<i32>> {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.iter().all(Option::is_none) {
        return Ok(Vec::new());
    }
    if values.iter().any(Option::is_none) {
        return invalid(format!(
            "track {path:?} only partially preserves tangentMode"
        ));
    }
    Ok(values.into_iter().flatten().collect())
}

pub(super) fn source_feature_counts(
    source: &SourceDocument,
    visual_selection: &SourceVisualSelection,
    paths: &BTreeMap<String, u32>,
) -> Result<ModelFeatureCounts> {
    let mut counts = ModelFeatureCounts::default();
    counts.nodes = u64_count(source.model_hierarchy.nodes.len(), "source node count")?;
    for (mesh_index, mesh) in source.meshes.iter().enumerate() {
        let vertices = mesh.positions.len() / 3;
        for (binding_index, binding) in mesh.source_bindings.iter().enumerate() {
            if !visual_selection.includes(mesh_index, binding_index) {
                continue;
            }
            let node_index = exact_node_path(paths, &binding.transform_path, "feature count")?;
            let renderer_index = exact_renderer_material_binding(
                source,
                mesh,
                binding,
                &source.model_hierarchy.nodes[node_index as usize],
            )?;
            // A Unity renderer with extra material slots repeats its final
            // submesh, so source draw-call/slot parity is the renderer slot
            // count rather than the raw Mesh submesh count.
            let draw_parts = source.renderer_material_bindings[renderer_index]
                .material_slots
                .len();
            add_count(&mut counts.mesh_parts, draw_parts, "source mesh part count")?;
            add_count(
                &mut counts.material_slots,
                draw_parts,
                "source material slot count",
            )?;
            if binding.component_type == "SkinnedMeshRenderer" {
                counts.skinned_meshes = counts
                    .skinned_meshes
                    .checked_add(1)
                    .ok_or_else(|| invalid_error("source skinned mesh count overflow"))?;
                let skin = mesh.skin.as_ref().ok_or_else(|| {
                    invalid_error(format!("skinned mesh {:?} lost its skin", mesh.name))
                })?;
                add_count(
                    &mut counts.joints,
                    skin.joint_paths.len(),
                    "source joint count",
                )?;
                add_count(
                    &mut counts.inverse_bind_matrices,
                    skin.inverse_bind_matrices.len(),
                    "source inverse-bind count",
                )?;
                add_count(
                    &mut counts.weighted_vertices,
                    vertices,
                    "source weighted vertex count",
                )?;
            }
        }
    }
    counts.animation_clips = u64_count(
        source
            .animations
            .iter()
            .filter(|animation| source_animation_has_model_payload(animation))
            .count(),
        "source animation clip count",
    )?;
    for animation in &source.animations {
        let tracks = animation
            .animation_data
            .translations
            .iter()
            .filter(|track| !track.unbound_model_target)
            .count()
            + animation
                .animation_data
                .rotations
                .iter()
                .filter(|track| !track.unbound_model_target)
                .count()
            + animation
                .animation_data
                .scales
                .iter()
                .filter(|track| !track.unbound_model_target)
                .count();
        add_count(
            &mut counts.animation_channels,
            tracks,
            "source animation channel count",
        )?;
        add_count(
            &mut counts.float_curves,
            animation.animation_data.float_curves.len(),
            "source float curve count",
        )?;
        add_count(
            &mut counts.empty_trs_bindings,
            animation
                .animation_data
                .empty_trs_bindings
                .iter()
                .filter(|binding| !binding.unbound_model_target)
                .count(),
            "source empty TRS binding count",
        )?;
        add_count(
            &mut counts.duplicate_trs_bindings,
            animation.animation_data.duplicate_trs_bindings.len(),
            "source duplicate TRS binding count",
        )?;
        for (interpolation, key_count) in animation
            .animation_data
            .translations
            .iter()
            .filter(|track| !track.unbound_model_target)
            .map(|track| (track.interpolation.as_str(), track.keys.len()))
            .chain(
                animation
                    .animation_data
                    .rotations
                    .iter()
                    .filter(|track| !track.unbound_model_target)
                    .map(|track| (track.interpolation.as_str(), track.keys.len())),
            )
            .chain(
                animation
                    .animation_data
                    .scales
                    .iter()
                    .filter(|track| !track.unbound_model_target)
                    .map(|track| (track.interpolation.as_str(), track.keys.len())),
            )
        {
            add_count(
                &mut counts.animation_keyframes,
                key_count,
                "source animation keyframe count",
            )?;
            if interpolation == "CUBICSPLINE" {
                add_count(
                    &mut counts.cubic_spline_keyframes,
                    key_count,
                    "source cubic keyframe count",
                )?;
            }
        }
        for duplicate_key_count in animation
            .animation_data
            .translations
            .iter()
            .filter(|track| !track.unbound_model_target)
            .map(|track| track.duplicate_keys.len())
            .chain(
                animation
                    .animation_data
                    .rotations
                    .iter()
                    .filter(|track| !track.unbound_model_target)
                    .map(|track| track.duplicate_keys.len()),
            )
            .chain(
                animation
                    .animation_data
                    .scales
                    .iter()
                    .filter(|track| !track.unbound_model_target)
                    .map(|track| track.duplicate_keys.len()),
            )
        {
            add_count(
                &mut counts.duplicate_same_time_keys,
                duplicate_key_count,
                "source duplicate same-time key count",
            )?;
        }
        add_count(
            &mut counts.animation_time_recoveries,
            animation.animation_data.time_recoveries.len(),
            "source animation time recovery count",
        )?;
        for recovery in &animation.animation_data.time_recoveries {
            add_count(
                &mut counts.recovered_animation_keyframes,
                recovery.recovered_times.len(),
                "source recovered animation keyframe count",
            )?;
        }
        add_count(
            &mut counts.animation_curve_recoveries,
            animation.animation_data.curve_recoveries.len(),
            "source animation curve recovery count",
        )?;
        for recovery in &animation.animation_data.curve_recoveries {
            add_count(
                &mut counts.rejected_conflicting_trs_bindings,
                recovery.rejected.len(),
                "source rejected conflicting TRS binding count",
            )?;
            for track in &recovery.rejected {
                add_count(
                    &mut counts.rejected_conflicting_trs_keyframes,
                    track.source_key_count,
                    "source rejected conflicting TRS keyframe count",
                )?;
            }
        }
        for binding in &animation.animation_data.duplicate_trs_bindings {
            add_count(
                &mut counts.duplicate_trs_keyframes,
                binding.source_key_count,
                "source duplicate TRS keyframe count",
            )?;
            add_count(
                &mut counts.duplicate_same_time_keys,
                binding.duplicate_keys.len(),
                "source duplicate same-time key count",
            )?;
        }
        add_count(
            &mut counts.animation_events,
            animation.events.len(),
            "source animation event count",
        )?;
        add_count(
            &mut counts.animation_event_null_object_pointers,
            animation
                .events
                .iter()
                .filter(|event| {
                    matches!(
                        event.object_parameter_provenance,
                        AnimationEventObjectParameterProvenance::SerializedPointer {
                            path_id: 0,
                            interpretation:
                                SerializedEventObjectParameterInterpretation::NullPathId,
                            ..
                        }
                    )
                })
                .count(),
            "source animation event null object pointer count",
        )?;
    }
    Ok(counts)
}

pub(super) fn feature_counts(model: &NativeModel) -> Result<ModelFeatureCounts> {
    let mut counts = ModelFeatureCounts {
        nodes: u64_count(model.nodes.len(), "published node count")?,
        animation_clips: u64_count(model.animations.len(), "published animation clip count")?,
        ..ModelFeatureCounts::default()
    };
    for mesh in &model.meshes {
        add_count(
            &mut counts.mesh_parts,
            mesh.primitives.len(),
            "published mesh part count",
        )?;
        add_count(
            &mut counts.material_slots,
            mesh.primitives.len(),
            "published material slot count",
        )?;
    }
    for skin in &model.skins {
        add_count(
            &mut counts.joints,
            skin.joints.len(),
            "published joint count",
        )?;
        add_count(
            &mut counts.inverse_bind_matrices,
            skin.inverse_bind_matrices.len(),
            "published inverse-bind count",
        )?;
    }
    for node in &model.nodes {
        let (Some(mesh_index), Some(_skin_index)) = (node.mesh, node.skin) else {
            continue;
        };
        counts.skinned_meshes = counts
            .skinned_meshes
            .checked_add(1)
            .ok_or_else(|| invalid_error("published skinned mesh count overflow"))?;
        let mesh = &model.meshes[mesh_index as usize];
        let first = mesh
            .primitives
            .first()
            .ok_or_else(|| invalid_error("published skinned mesh has no primitives"))?;
        if mesh.primitives.iter().any(|primitive| {
            primitive.positions.len() != first.positions.len()
                || primitive.joints.len() != first.joints.len()
                || primitive.weights.len() != first.weights.len()
        }) {
            return invalid("published skinned submeshes disagree on source vertex cardinality");
        }
        add_count(
            &mut counts.weighted_vertices,
            first.joints.len(),
            "published weighted vertex count",
        )?;
    }
    for clip in &model.animations {
        add_count(
            &mut counts.animation_channels,
            clip.channels.len(),
            "published animation channel count",
        )?;
        add_count(
            &mut counts.float_curves,
            clip.metadata.float_curves.len(),
            "published float curve count",
        )?;
        for channel in &clip.channels {
            add_count(
                &mut counts.animation_keyframes,
                channel.times.len(),
                "published animation keyframe count",
            )?;
            if channel.interpolation == Interpolation::CubicSpline {
                add_count(
                    &mut counts.cubic_spline_keyframes,
                    channel.times.len(),
                    "published cubic keyframe count",
                )?;
            }
            add_count(
                &mut counts.duplicate_same_time_keys,
                channel.duplicate_keys.len(),
                "published duplicate same-time key count",
            )?;
        }
        add_count(
            &mut counts.animation_events,
            clip.metadata.events.len(),
            "published animation event count",
        )?;
        add_count(
            &mut counts.animation_event_null_object_pointers,
            clip.metadata
                .events
                .iter()
                .filter(|event| {
                    matches!(
                        event.object_parameter_provenance,
                        AnimationEventObjectParameterProvenance::SerializedPointer {
                            path_id: 0,
                            interpretation:
                                SerializedEventObjectParameterInterpretation::NullPathId,
                            ..
                        }
                    )
                })
                .count(),
            "published animation event null object pointer count",
        )?;
        add_count(
            &mut counts.empty_trs_bindings,
            clip.metadata.empty_trs_bindings.len(),
            "published empty TRS binding count",
        )?;
        add_count(
            &mut counts.duplicate_trs_bindings,
            clip.metadata.duplicate_trs_bindings.len(),
            "published duplicate TRS binding count",
        )?;
        for binding in &clip.metadata.duplicate_trs_bindings {
            add_count(
                &mut counts.duplicate_trs_keyframes,
                binding.source_key_count as usize,
                "published duplicate TRS keyframe count",
            )?;
            add_count(
                &mut counts.duplicate_same_time_keys,
                binding.duplicate_keys.len(),
                "published duplicate same-time key count",
            )?;
        }
        add_count(
            &mut counts.animation_time_recoveries,
            clip.metadata.time_recoveries.len(),
            "published animation time recovery count",
        )?;
        for recovery in &clip.metadata.time_recoveries {
            add_count(
                &mut counts.recovered_animation_keyframes,
                recovery.recovered_times.len(),
                "published recovered animation keyframe count",
            )?;
        }
        add_count(
            &mut counts.animation_curve_recoveries,
            clip.metadata.curve_recoveries.len(),
            "published animation curve recovery count",
        )?;
        for recovery in &clip.metadata.curve_recoveries {
            add_count(
                &mut counts.rejected_conflicting_trs_bindings,
                recovery.rejected.len(),
                "published rejected conflicting TRS binding count",
            )?;
            for track in &recovery.rejected {
                add_count(
                    &mut counts.rejected_conflicting_trs_keyframes,
                    track.source_key_count as usize,
                    "published rejected conflicting TRS keyframe count",
                )?;
            }
        }
        let object_keys = clip
            .metadata
            .object_curves
            .iter()
            .try_fold(0usize, |total, curve| total.checked_add(curve.keys.len()))
            .ok_or_else(|| invalid_error("published object-reference key count overflow"))?;
        add_count(
            &mut counts.object_reference_keys,
            object_keys,
            "published object-reference key count",
        )?;
    }
    Ok(counts)
}

pub(super) fn matrix_max_difference(left: Mat4, right: Mat4) -> f64 {
    left.to_cols_array()
        .into_iter()
        .zip(right.to_cols_array())
        .map(|(left, right)| f64::from((left - right).abs()))
        .fold(0.0, f64::max)
}

pub(super) fn matrix_identity_deviation(matrix: Mat4) -> MatrixIdentityDeviation {
    let mut result = MatrixIdentityDeviation {
        max_abs: -1.0,
        row: 0,
        column: 0,
        actual: 0.0,
        identity: 1.0,
    };
    for (column, values) in matrix.to_cols_array().chunks_exact(4).enumerate() {
        for (row, actual) in values.iter().enumerate() {
            let identity = if row == column { 1.0 } else { 0.0 };
            let deviation = f64::from((*actual - identity).abs());
            if deviation > result.max_abs {
                result = MatrixIdentityDeviation {
                    max_abs: deviation,
                    row,
                    column,
                    actual: f64::from(*actual),
                    identity: f64::from(identity),
                };
            }
        }
    }
    result
}

pub(super) fn native_matrix(matrix: [[f64; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array(&[
        matrix[0][0] as f32,
        matrix[1][0] as f32,
        matrix[2][0] as f32,
        matrix[3][0] as f32,
        matrix[0][1] as f32,
        matrix[1][1] as f32,
        matrix[2][1] as f32,
        matrix[3][1] as f32,
        matrix[0][2] as f32,
        matrix[1][2] as f32,
        matrix[2][2] as f32,
        matrix[3][2] as f32,
        matrix[0][3] as f32,
        matrix[1][3] as f32,
        matrix[2][3] as f32,
        matrix[3][3] as f32,
    ])
}

pub(super) fn vec3_values(values: &[f64], label: &str, mesh: &str) -> Result<Vec<[f64; 3]>> {
    if values.is_empty() || values.len() % 3 != 0 {
        return invalid(format!("mesh {mesh:?} has invalid flat {label}"));
    }
    Ok(values
        .chunks_exact(3)
        .map(|value| [value[0], value[1], value[2]])
        .collect())
}

pub(super) fn vec2_values(values: &[f64], label: &str, mesh: &str) -> Result<Vec<[f64; 2]>> {
    if values.is_empty() || values.len() % 2 != 0 {
        return invalid(format!("mesh {mesh:?} has invalid flat {label}"));
    }
    Ok(values
        .chunks_exact(2)
        .map(|value| [value[0], value[1]])
        .collect())
}

pub(super) fn u64_count(value: usize, label: &str) -> Result<u64> {
    value
        .try_into()
        .map_err(|_| invalid_error(format!("{label} exceeds u64")))
}

pub(super) fn u32_count(value: usize, label: &str) -> Result<u32> {
    u32::try_from(value).map_err(|_| invalid_error(format!("{label} exceeds u32")))
}

pub(super) fn add_count(target: &mut u64, value: usize, label: &str) -> Result<()> {
    *target = target
        .checked_add(u64_count(value, label)?)
        .ok_or_else(|| invalid_error(format!("{label} overflow")))?;
    Ok(())
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
