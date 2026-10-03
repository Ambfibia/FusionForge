use super::*;

pub(super) fn build_skin(
    mesh: &SourceMesh,
    binding: &SourceBinding,
    paths: &BTreeMap<String, u32>,
    skeleton_root: u32,
) -> Result<(ModelSkin, Vec<[u16; 4]>, Vec<[f64; 4]>)> {
    let skin = mesh.skin.as_ref().ok_or_else(|| {
        invalid_error(format!(
            "SkinnedMeshRenderer mesh {:?} has no skin payload",
            mesh.name
        ))
    })?;
    if skin.source != "unity-skinned-mesh-renderer" {
        return invalid(format!(
            "SkinnedMeshRenderer mesh {:?} has source {:?}",
            mesh.name, skin.source
        ));
    }
    let (_, renderer_asset, renderer_path) = binding_component_key(binding)?;
    if skin.renderer_asset_index != Some(renderer_asset)
        || skin.renderer_path_id != Some(renderer_path)
        || skin.renderer_transform_path_id != Some(binding.transform_path_id)
    {
        return invalid(format!(
            "skin renderer identity differs from source binding for {:?}",
            mesh.name
        ));
    }
    if skin.mesh_filter_asset_index.is_some()
        || skin.mesh_filter_path_id.is_some()
        || skin.attachment_transform_path_id.is_some()
        || skin.rest_vertex_transform.is_some()
    {
        return invalid(format!(
            "skinned mesh {:?} contains rigid MeshFilter metadata",
            mesh.name
        ));
    }
    if let Some(warning) = skin.decode_warning.as_deref() {
        if warning != "implicit rigid one-bone skin reconstructed from renderer palette" {
            return invalid(format!(
                "unresolved skin decode warning for {:?}: {warning}",
                mesh.name
            ));
        }
    }
    if skin.joint_paths.is_empty() || skin.joint_paths.len() != skin.inverse_bind_matrices.len() {
        return invalid(format!(
            "skin joint/inverse-bind count mismatch for {:?}",
            mesh.name
        ));
    }
    let mut mapped = BTreeSet::new();
    let mut joints = Vec::with_capacity(skin.joint_paths.len());
    for path in &skin.joint_paths {
        let joint = suffix_node_path(paths, path, "skin joint")?;
        if !mapped.insert(joint) {
            return invalid(format!(
                "skin {:?} maps duplicate joint path {:?}",
                mesh.name, path
            ));
        }
        joints.push(joint);
    }
    let vertices = mesh.positions.len() / 3;
    if skin.bone_indices.len() != vertices.saturating_mul(4)
        || skin.weights.len() != vertices.saturating_mul(4)
    {
        return invalid(format!(
            "skin weight cardinality differs from vertices for {:?}",
            mesh.name
        ));
    }
    let bone_indices = skin
        .bone_indices
        .chunks_exact(4)
        .map(|values| [values[0], values[1], values[2], values[3]])
        .collect::<Vec<_>>();
    let weights = skin
        .weights
        .chunks_exact(4)
        .map(|values| [values[0], values[1], values[2], values[3]])
        .collect::<Vec<_>>();
    Ok((
        ModelSkin {
            name: mesh.name.clone(),
            skeleton_root,
            joints,
            inverse_bind_matrices: skin.inverse_bind_matrices.clone(),
        },
        bone_indices,
        weights,
    ))
}

pub(super) fn build_primitives(
    mesh: &SourceMesh,
    renderer_slots: &[ResolvedMaterialSlot],
    skin: Option<(&[[u16; 4]], &[[f64; 4]])>,
) -> Result<Vec<ModelPrimitive>> {
    if mesh.kind != "mesh" || mesh.name.trim().is_empty() || mesh.name.chars().any(char::is_control)
    {
        return invalid(format!("invalid source mesh {:?}", mesh.name));
    }
    // GLB accessors store FLOAT, not the export JSON's f64 intermediates.
    // Classify winding on those exact persisted values: rounding a very thin
    // triangle can reverse its normal agreement (primary npc_bentennyson).
    // This does not change the serialized vertex bytes; it moves their existing
    // quantization before the orientation decision and native validation.
    let positions = vec3_values(&mesh.positions, "positions", &mesh.name)?
        .into_iter()
        .map(|v| v.map(|component| f64::from(component as f32)))
        .collect::<Vec<_>>();
    let vertex_count = positions.len();
    let normals = if mesh.normals.is_empty() {
        Vec::new()
    } else {
        let values = vec3_values(&mesh.normals, "normals", &mesh.name)?;
        if values.len() != vertex_count {
            return invalid(format!("mesh {:?} normal count mismatch", mesh.name));
        }
        values
            .into_iter()
            .map(|v| v.map(|component| f64::from(component as f32)))
            .collect()
    };
    let uvs = if mesh.uvs.is_empty() {
        Vec::new()
    } else {
        let values = vec2_values(&mesh.uvs, "UVs", &mesh.name)?;
        if values.len() != vertex_count {
            return invalid(format!("mesh {:?} UV count mismatch", mesh.name));
        }
        values
    };
    let (joints, weights) = skin
        .map(|(joints, weights)| (joints.to_vec(), weights.to_vec()))
        .unwrap_or_default();
    if (!joints.is_empty() && joints.len() != vertex_count)
        || (!weights.is_empty() && weights.len() != vertex_count)
    {
        return invalid(format!(
            "mesh {:?} skin attribute count mismatch",
            mesh.name
        ));
    }
    if mesh.groups.is_empty() {
        return invalid(format!("mesh {:?} has no submesh groups", mesh.name));
    }
    let mut cursor = 0usize;
    let mut primitives = Vec::with_capacity(renderer_slots.len().max(mesh.groups.len()));
    for group in &mesh.groups {
        let end = group
            .start
            .checked_add(group.count)
            .ok_or_else(|| invalid_error(format!("mesh {:?} group range overflow", mesh.name)))?;
        if group.start != cursor
            || group.count == 0
            || group.count % 3 != 0
            || end > mesh.indices.len()
        {
            return invalid(format!(
                "mesh {:?} groups do not exactly partition triangular indices",
                mesh.name
            ));
        }
        let resolved = renderer_slots.get(group.material_index).ok_or_else(|| {
            invalid_error(format!(
                "mesh {:?} renderer material index {} is out of bounds",
                mesh.name, group.material_index
            ))
        })?;
        // Exact Unity dumps can contain mixed serialized winding even within one
        // primitive. Decide each normal-bearing triangle independently in native
        // H-reflected space; preserve degenerate and normal-less cards verbatim.
        let mut indices = mesh.indices[group.start..end].to_vec();
        for (triangle_index, triangle) in indices.chunks_exact_mut(3).enumerate() {
            let opposed = triangle_winding_opposes_normals(&positions, &normals, triangle)
                .map_err(|error| {
                    invalid_error(format!(
                        "mesh {:?} primitive {} triangle {} winding audit failed: {error}",
                        mesh.name, group.material_index, triangle_index
                    ))
                })?;
            if opposed == Some(true) {
                triangle.swap(1, 2);
            }
        }
        primitives.push(ModelPrimitive {
            material: resolved.material,
            material_slot: resolved.name.clone(),
            positions: positions.clone(),
            normals: normals.clone(),
            uvs: uvs.clone(),
            joints: joints.clone(),
            weights: weights.clone(),
            indices,
        });
        cursor = end;
    }
    if cursor != mesh.indices.len() {
        return invalid(format!(
            "mesh {:?} groups leave source indices unpublished",
            mesh.name
        ));
    }
    // Unity renders the final submesh once for every material slot beyond the
    // mesh submesh count. Preserve those extra draw calls as glTF primitives
    // sharing the exact same geometry and using the corresponding slot.
    let used_slot_count = mesh
        .groups
        .iter()
        .map(|group| group.material_index)
        .max()
        .and_then(|index| index.checked_add(1))
        .ok_or_else(|| invalid_error(format!("mesh {:?} has no material slots", mesh.name)))?;
    if renderer_slots.len() > used_slot_count {
        let last_submesh = primitives.last().cloned().ok_or_else(|| {
            invalid_error(format!(
                "mesh {:?} has no final submesh to repeat",
                mesh.name
            ))
        })?;
        for resolved in &renderer_slots[used_slot_count..] {
            let mut repeated = last_submesh.clone();
            repeated.material = resolved.material;
            repeated.material_slot = resolved.name.clone();
            primitives.push(repeated);
        }
    }
    Ok(primitives)
}

pub(super) fn build_animations(
    source: &SourceDocument,
    paths: &BTreeMap<String, u32>,
    root_path: &str,
) -> Result<Vec<AnimationClip>> {
    let mut animations = Vec::with_capacity(source.animations.len());
    for (animation_index, animation) in source.animations.iter().enumerate() {
        let counts = &animation.curve_counts;
        if counts.euler != 0 || counts.pptr != 0 {
            return invalid(format!(
                "animation {:?} has unsupported Euler/PPtr curves ({}/{})",
                animation.name, counts.euler, counts.pptr
            ));
        }
        if counts.float != animation.animation_data.float_curves.len() {
            return invalid(format!(
                "animation {:?} float curveCounts differ from exact decoded bindings",
                animation.name
            ));
        }
        validate_source_curve_recoveries(source, animation_index)?;
        let rotations_are_compressed = reconcile_animation_curve_counts(animation)?;
        if rotations_are_compressed
            && animation
                .animation_data
                .rotations
                .iter()
                .any(|track| track.interpolation != "LINEAR")
        {
            return invalid(format!(
                "animation {:?} compressed rotation was not preserved as LINEAR",
                animation.name
            ));
        }

        let mut targets = BTreeSet::new();
        let mut channels = Vec::with_capacity(
            animation.animation_data.translations.len()
                + animation.animation_data.rotations.len()
                + animation.animation_data.scales.len(),
        );
        for track in &animation.animation_data.translations {
            if track.unbound_model_target {
                validate_unbound_animation_node_path(
                    paths,
                    root_path,
                    &track.path,
                    "unbound translation track",
                )?;
                continue;
            }
            let channel = vec3_channel(track, TrackKind::Translation, paths, root_path)?;
            if !targets.insert((channel.target_node, TrackKind::Translation)) {
                return invalid(format!(
                    "animation {:?} has duplicate translation target {:?}",
                    animation.name, track.path
                ));
            }
            channels.push(channel);
        }
        for track in &animation.animation_data.rotations {
            if track.unbound_model_target {
                validate_unbound_animation_node_path(
                    paths,
                    root_path,
                    &track.path,
                    "unbound rotation track",
                )?;
                continue;
            }
            let channel = quat_channel(track, paths, root_path)?;
            if !targets.insert((channel.target_node, TrackKind::Rotation)) {
                return invalid(format!(
                    "animation {:?} has duplicate rotation target {:?}",
                    animation.name, track.path
                ));
            }
            channels.push(channel);
        }
        for track in &animation.animation_data.scales {
            if track.unbound_model_target {
                validate_unbound_animation_node_path(
                    paths,
                    root_path,
                    &track.path,
                    "unbound scale track",
                )?;
                continue;
            }
            let channel = vec3_channel(track, TrackKind::Scale, paths, root_path)?;
            if !targets.insert((channel.target_node, TrackKind::Scale)) {
                return invalid(format!(
                    "animation {:?} has duplicate scale target {:?}",
                    animation.name, track.path
                ));
            }
            channels.push(channel);
        }
        let mut float_curves = Vec::with_capacity(animation.animation_data.float_curves.len());
        for (expected_source_index, track) in
            animation.animation_data.float_curves.iter().enumerate()
        {
            if track.source_index != expected_source_index
                || track.source_encoding != EmptyTrsSourceEncoding::Plain
            {
                return invalid(format!(
                    "animation {:?} float curves are not in exact plain source order",
                    animation.name
                ));
            }
            float_curves.push(convert_float_curve(track, paths, root_path)?);
        }
        let mut empty_trs_bindings =
            Vec::with_capacity(animation.animation_data.empty_trs_bindings.len());
        for binding in &animation.animation_data.empty_trs_bindings {
            if binding.unbound_model_target {
                validate_unbound_animation_node_path(
                    paths,
                    root_path,
                    &binding.path,
                    "unbound empty animation TRS binding",
                )?;
                continue;
            }
            let target_node = animation_node_path(
                paths,
                root_path,
                &binding.path,
                "empty animation TRS binding",
            )?;
            if targets.contains(&(target_node, track_kind(binding.kind))) {
                return invalid(format!(
                    "animation {:?} empty {:?} binding {:?} collides with a non-empty channel",
                    animation.name, binding.kind, binding.path
                ));
            }
            empty_trs_bindings.push(EmptyTrsBinding {
                kind: binding.kind,
                target_node,
                target_path: binding.path.clone(),
                source_index: u32_index(binding.source_index, "empty TRS source index")?,
                source_encoding: binding.source_encoding,
            });
        }
        let mut duplicate_trs_bindings =
            Vec::with_capacity(animation.animation_data.duplicate_trs_bindings.len());
        for binding in &animation.animation_data.duplicate_trs_bindings {
            let target_node = animation_node_path(
                paths,
                root_path,
                &binding.path,
                "duplicate animation TRS binding",
            )?;
            duplicate_trs_bindings.push(DuplicateTrsBinding {
                kind: binding.kind,
                target_node,
                target_path: binding.path.clone(),
                source_index: u32_index(binding.source_index, "duplicate TRS source index")?,
                source_encoding: binding.source_encoding,
                relation: binding.relation,
                canonical_track_index: u32_index(
                    binding.canonical_track_index,
                    "duplicate TRS canonical track index",
                )?,
                canonical_source_index: u32_index(
                    binding.canonical_source_index,
                    "duplicate TRS canonical source index",
                )?,
                canonical_source_encoding: binding.canonical_source_encoding,
                source_key_count: u32_index(
                    binding.source_key_count,
                    "duplicate TRS source key count",
                )?,
                duplicate_keys: convert_duplicate_animation_keys(&binding.duplicate_keys)?,
                keys: convert_duplicate_keys(&binding.keys)?,
                resolution_proof: binding.resolution_proof.clone(),
            });
        }
        let mut time_recoveries =
            Vec::with_capacity(animation.animation_data.time_recoveries.len());
        for recovery in &animation.animation_data.time_recoveries {
            let target_node = animation_node_path(
                paths,
                root_path,
                &recovery.path,
                "recovered animation TRS binding",
            )?;
            time_recoveries.push(AnimationTimeRecovery {
                kind: recovery.kind,
                target_node,
                target_path: recovery.path.clone(),
                source_encoding: recovery.source_encoding,
                source_index: u32_index(
                    recovery.source_index,
                    "recovered animation TRS source index",
                )?,
                reason: recovery.reason,
                original_times: recovery.original_times.clone(),
                recovered_times: recovery.recovered_times.clone(),
                source_sample_rate: recovery.source_sample_rate,
                reference: recovery.reference.clone(),
                proof: recovery.proof.clone(),
            });
        }
        let mut curve_recoveries =
            Vec::with_capacity(animation.animation_data.curve_recoveries.len());
        for recovery in &animation.animation_data.curve_recoveries {
            let target_node = animation_node_path(
                paths,
                root_path,
                &recovery.path,
                "conflicting duplicate animation TRS recovery",
            )?;
            curve_recoveries.push(AnimationCurveRecovery {
                kind: recovery.kind,
                target_node,
                target_path: recovery.path.clone(),
                source_encoding: recovery.source_encoding,
                reason: recovery.reason,
                source_sample_rate: recovery.source_sample_rate,
                source: AnimationCurveRecoverySource {
                    field: recovery.source.field.clone(),
                    source_encoding: recovery.source.source_encoding,
                    source_indices: recovery
                        .source
                        .source_indices
                        .iter()
                        .map(|&index| u32_index(index, "curve recovery source index"))
                        .collect::<Result<Vec<_>>>()?,
                    source_target_curve_count: u64::try_from(
                        recovery.source.source_target_curve_count,
                    )
                    .map_err(|_| invalid_error("curve recovery source curve count exceeds u64"))?,
                },
                canonical: convert_curve_recovery_canonical(&recovery.canonical)?,
                rejected: recovery
                    .rejected
                    .iter()
                    .map(convert_curve_recovery_track)
                    .collect::<Result<Vec<_>>>()?,
                reference: AnimationCurveRecoveryReference {
                    asset: recovery.reference.asset.clone(),
                    path_id: recovery.reference.path_id,
                    clip_name: recovery.reference.clip_name.clone(),
                    field: recovery.reference.field.clone(),
                    kind: recovery.reference.kind,
                    path: recovery.reference.path.clone(),
                    source_encoding: recovery.reference.source_encoding,
                    source_index: u32_index(
                        recovery.reference.source_index,
                        "curve recovery reference source index",
                    )?,
                    sample_rate: recovery.reference.sample_rate,
                },
                proof: recovery.proof.clone(),
            });
        }
        let mut events = Vec::with_capacity(animation.events.len());
        for event in &animation.events {
            validate_source_event_object_parameter(animation, event)?;
            events.push(AnimationEvent {
                time: event.time,
                function_name: event.function_name.clone(),
                string_parameter: event.string_parameter.clone(),
                float_parameter: event.float_parameter,
                float_parameter_provenance: event.float_parameter_provenance.clone(),
                int_parameter: event.int_parameter,
                object_parameter: None,
                object_parameter_provenance: event.object_parameter_provenance.clone(),
                message_options: event.message_options,
            });
        }
        let keyed_duration = channels
            .iter()
            .filter_map(|channel| channel.times.last().copied())
            .chain(
                float_curves
                    .iter()
                    .filter_map(|curve| curve.times.last().copied()),
            )
            .chain(curve_recoveries.iter().flat_map(|recovery| {
                std::iter::once(&recovery.canonical.keys)
                    .chain(recovery.rejected.iter().map(|track| &track.keys))
                    .filter_map(exact_keys_last_time)
            }))
            .chain(
                animation
                    .animation_data
                    .translations
                    .iter()
                    .filter(|track| track.unbound_model_target)
                    .filter_map(|track| track.keys.last().map(|key| key.time)),
            )
            .chain(
                animation
                    .animation_data
                    .rotations
                    .iter()
                    .filter(|track| track.unbound_model_target)
                    .filter_map(|track| track.keys.last().map(|key| key.time)),
            )
            .chain(
                animation
                    .animation_data
                    .scales
                    .iter()
                    .filter(|track| track.unbound_model_target)
                    .filter_map(|track| track.keys.last().map(|key| key.time)),
            )
            .reduce(f64::max);
        let event_duration = events.iter().map(|event| event.time).reduce(f64::max);
        validate_animation_duration_provenance(animation, keyed_duration, event_duration)?;
        // Shared preload ranges sometimes attach a complete AnimationClip to
        // an equipment object even though every serialized binding targets a
        // different sibling model. The exporter marks those bindings
        // `unboundModelTarget`; retain and validate that source evidence, but
        // do not emit an illegal empty glTF animation for this logical model.
        if channels.is_empty()
            && float_curves.is_empty()
            && events.is_empty()
            && empty_trs_bindings.is_empty()
            && duplicate_trs_bindings.is_empty()
            && time_recoveries.is_empty()
            && curve_recoveries.is_empty()
        {
            continue;
        }
        animations.push(AnimationClip {
            name: animation.name.clone(),
            duration: animation.duration,
            declared_duration: animation.declared_duration.0,
            keyed_duration: animation.keyed_duration.0,
            event_duration: animation.event_duration.0,
            sample_rate: animation.sample_rate,
            wrap_mode: animation.wrap_mode,
            looped: animation.looped,
            channels,
            metadata: AnimationMetadata {
                float_curves,
                object_curves: Vec::new(),
                events,
                empty_trs_bindings,
                duplicate_trs_bindings,
                time_recoveries,
                curve_recoveries,
                unsupported: Vec::new(),
            },
        });
    }
    Ok(animations)
}

/// `NpcAnimation.MakeUperLayer` marks ordinary `melee*` and `wound*` states
/// as `AnimationBlendMode.Additive`, except for the earlier `upper` and
/// `event` branches. Unity evaluates those states relative to their first
/// sample. glTF has no equivalent clip flag, so explicitly upgraded native
/// packages store translation/scale deltas and identity-relative rotations for
/// Bevy's additive animation graph.
pub(super) fn rebase_legacy_npc_additive_clips(clips: &mut [AnimationClip]) -> Result<()> {
    for clip in clips {
        if !legacy_npc_clip_is_additive(&clip.name) {
            continue;
        }
        for channel in &mut clip.channels {
            match &mut channel.values {
                TrackValues::Translation(values) | TrackValues::Scale(values) => {
                    let reference = values.first().copied().ok_or_else(|| {
                        invalid_error(format!(
                            "legacy additive clip {:?} channel {} has no samples",
                            clip.name, channel.source_index
                        ))
                    })?;
                    for value in values {
                        for (component, reference) in value.iter_mut().zip(reference) {
                            *component -= reference;
                        }
                    }
                    // Subtracting a constant reference leaves cubic
                    // translation/scale derivatives unchanged.
                }
                TrackValues::Rotation(values) => {
                    let reference = values.first().copied().ok_or_else(|| {
                        invalid_error(format!(
                            "legacy additive clip {:?} channel {} has no samples",
                            clip.name, channel.source_index
                        ))
                    })?;
                    let inverse_reference = quat_conjugate(reference);
                    for value in values {
                        *value = quat_multiply(*value, inverse_reference);
                    }
                    if let Some(TrackValues::Rotation(tangents)) = &mut channel.in_tangents {
                        for tangent in tangents {
                            *tangent = quat_multiply(*tangent, inverse_reference);
                        }
                    }
                    if let Some(TrackValues::Rotation(tangents)) = &mut channel.out_tangents {
                        for tangent in tangents {
                            *tangent = quat_multiply(*tangent, inverse_reference);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn quat_conjugate(value: [f64; 4]) -> [f64; 4] {
    [-value[0], -value[1], -value[2], value[3]]
}

pub(super) fn quat_multiply(left: [f64; 4], right: [f64; 4]) -> [f64; 4] {
    let [lx, ly, lz, lw] = left;
    let [rx, ry, rz, rw] = right;
    [
        lw * rx + lx * rw + ly * rz - lz * ry,
        lw * ry - lx * rz + ly * rw + lz * rx,
        lw * rz + lx * ry - ly * rx + lz * rw,
        lw * rw - lx * rx - ly * ry - lz * rz,
    ]
}

pub(super) fn source_tracks(
    animation: &SourceAnimation,
    kind: EmptyTrsBindingKind,
) -> Vec<SourceRecoveryTrackRef<'_>> {
    match kind {
        EmptyTrsBindingKind::Translation => animation
            .animation_data
            .translations
            .iter()
            .map(SourceRecoveryTrackRef::Vec3)
            .collect(),
        EmptyTrsBindingKind::Rotation => animation
            .animation_data
            .rotations
            .iter()
            .map(SourceRecoveryTrackRef::Quaternion)
            .collect(),
        EmptyTrsBindingKind::Scale => animation
            .animation_data
            .scales
            .iter()
            .map(SourceRecoveryTrackRef::Vec3)
            .collect(),
    }
}

pub(super) fn source_track_at(
    animation: &SourceAnimation,
    kind: EmptyTrsBindingKind,
    index: usize,
) -> Option<SourceRecoveryTrackRef<'_>> {
    match kind {
        EmptyTrsBindingKind::Translation => animation
            .animation_data
            .translations
            .get(index)
            .map(SourceRecoveryTrackRef::Vec3),
        EmptyTrsBindingKind::Rotation => animation
            .animation_data
            .rotations
            .get(index)
            .map(SourceRecoveryTrackRef::Quaternion),
        EmptyTrsBindingKind::Scale => animation
            .animation_data
            .scales
            .get(index)
            .map(SourceRecoveryTrackRef::Vec3),
    }
}

pub(super) fn source_track_encoding(track: SourceRecoveryTrackRef<'_>) -> EmptyTrsSourceEncoding {
    match track {
        SourceRecoveryTrackRef::Vec3(track) => track.source_encoding,
        SourceRecoveryTrackRef::Quaternion(track) => track.source_encoding,
    }
}

pub(super) fn source_canonical_recovery_matches(
    recovery: &SourceAnimationCurveRecoveryCanonicalTrack,
    actual: SourceRecoveryTrackRef<'_>,
) -> bool {
    match (actual, &recovery.keys) {
        (SourceRecoveryTrackRef::Vec3(actual), SourceDuplicateTrsKeys::Vec3(keys)) => {
            recovery.path == actual.path
                && recovery.interpolation == actual.interpolation
                && keys == &actual.keys
                && recovery.duplicate_keys == actual.duplicate_keys
                && recovery.source_key_count == actual.source_key_count
                && recovery.source_index == actual.source_index
                && recovery.source_encoding == actual.source_encoding
        }
        (SourceRecoveryTrackRef::Quaternion(actual), SourceDuplicateTrsKeys::Quaternion(keys)) => {
            recovery.path == actual.path
                && recovery.interpolation == actual.interpolation
                && keys == &actual.keys
                && recovery.duplicate_keys == actual.duplicate_keys
                && recovery.source_key_count == actual.source_key_count
                && recovery.source_index == actual.source_index
                && recovery.source_encoding == actual.source_encoding
        }
        _ => false,
    }
}

pub(super) fn source_keys_len(keys: &SourceDuplicateTrsKeys) -> usize {
    match keys {
        SourceDuplicateTrsKeys::Vec3(keys) => keys.len(),
        SourceDuplicateTrsKeys::Quaternion(keys) => keys.len(),
    }
}

pub(super) fn source_target_binding_count(
    animation: &SourceAnimation,
    kind: EmptyTrsBindingKind,
    path: &str,
) -> usize {
    source_tracks(animation, kind)
        .into_iter()
        .filter(|track| source_track_path(*track) == path)
        .count()
        + animation
            .animation_data
            .empty_trs_bindings
            .iter()
            .filter(|binding| binding.kind == kind && binding.path == path)
            .count()
        + animation
            .animation_data
            .duplicate_trs_bindings
            .iter()
            .filter(|binding| binding.kind == kind && binding.path == path)
            .count()
        + animation
            .animation_data
            .curve_recoveries
            .iter()
            .filter(|recovery| recovery.kind == kind && recovery.path == path)
            .map(|recovery| recovery.rejected.len())
            .sum::<usize>()
}

pub(super) fn source_curve_field(kind: EmptyTrsBindingKind) -> &'static str {
    match kind {
        EmptyTrsBindingKind::Translation => "m_PositionCurves",
        EmptyTrsBindingKind::Rotation => "m_RotationCurves",
        EmptyTrsBindingKind::Scale => "m_ScaleCurves",
    }
}
