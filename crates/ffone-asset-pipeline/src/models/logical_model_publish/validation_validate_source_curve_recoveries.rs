use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CoordinateAuditReport {
    pub status: String,
    pub runtime_spawn_policy: String,
    pub runtime_spawn_policy_note: String,
    pub root_translation: [f64; 3],
    pub root_rotation: [f64; 4],
    pub root_scale: [f64; 3],
    pub unit_scale: f64,
    pub auto_centered: bool,
    pub auto_scaled: bool,
    pub raw_local_bounds: CoordinateBounds,
    pub static_rest_world_bounds: CoordinateBounds,
    pub source_triangle_winding: String,
    pub published_triangle_winding: String,
    pub winding_swap_applied_per_triangle: bool,
    pub inverse_bind_matrices: u64,
    pub skinning_basis_parity_status: String,
    pub skinning_basis_parity_max_error: Option<f64>,
    pub current_pose_bind_identity_deviation_max: Option<f64>,
    pub current_pose_bind_identity_deviation_worst: Option<CurrentPoseBindIdentityDeviation>,
}

pub(super) fn validate_renderer_slots(
    mesh: &SourceMesh,
    renderer: &SourceRendererMaterialBinding,
) -> Result<()> {
    let used_slots = mesh
        .groups
        .iter()
        .map(|group| group.material_index)
        .collect::<BTreeSet<_>>();
    // Unity accepts more Renderer materials than Mesh submeshes. In that
    // legacy case every extra material renders the last submesh again. This
    // occurs on primary Fusion characters (Belladonna's eye is the focused
    // acceptance case), so rejecting the source loses real draw calls.
    // A renderer with fewer slots than the mesh consumes remains invalid.
    if renderer.material_slots.len() < used_slots.len()
        || used_slots.iter().copied().ne(0..used_slots.len())
    {
        return invalid(format!(
            "mesh {:?} submeshes are not a contiguous prefix of renderer {:?} material slots",
            mesh.name, renderer.renderer.id
        ));
    }
    Ok(())
}

pub(super) fn validate_source_object(
    map_id: &str,
    object_id: &str,
    source: &SourceObjectReference,
    expected_type: &str,
) -> Result<()> {
    if map_id != object_id
        || object_id != source.id
        || source.object_type != expected_type
        || source.path_id == 0
    {
        return invalid(format!(
            "{expected_type} exact source identity is contradictory for {map_id:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_pointer_identity(
    pointer: &SourcePointer,
    object: &SourceObjectReference,
    context: &str,
) -> Result<()> {
    if pointer.is_null
        || pointer.path_id != object.path_id
        || (pointer.file_id == 0 && pointer.source_asset_index != object.asset_index)
    {
        return invalid(format!(
            "{context} pointer ({}/{}/{}) does not match exact source identity ({}/{})",
            pointer.source_asset_index,
            pointer.file_id,
            pointer.path_id,
            object.asset_index,
            object.path_id
        ));
    }
    Ok(())
}

pub(super) fn validate_source_header(source: &SourceDocument) -> Result<()> {
    if source.schema != LOGICAL_MODEL_SOURCE_SCHEMA {
        return invalid(format!(
            "source schema must be {LOGICAL_MODEL_SOURCE_SCHEMA:?}"
        ));
    }
    if source.selection_mode != "exact-container-route" || source.status != "ready" {
        return invalid("source must be a ready exact-container-route export");
    }
    let coordinates = &source.native_coordinate_contract;
    let current_winding_contract = coordinates.source_triangle_winding
        == "unity-source-indices-in-h-reflected-native-space"
        && coordinates.published_triangle_winding
            == "gltf-counter-clockwise-per-triangle-normal-aligned"
        && coordinates.winding_conversion_owner == "ffone-asset-pipeline-triangle-normal-audit";
    // Deterministically upgrade already-extracted v1 sources. Their arrays are
    // in the same H-reflected native space and retain the same source indices;
    // only this historical metadata incorrectly requested a second swap.
    let legacy_double_swap_contract = coordinates.source_triangle_winding
        == "unity-clockwise-preserved"
        && coordinates.published_triangle_winding
            == "swap-index-1-and-2-for-gltf-counter-clockwise"
        && coordinates.winding_conversion_owner == "ffone-asset-pipeline";
    if coordinates.schema != "ffone.native-coordinate-contract.v1"
        || coordinates.published_space != "gltf-right-handed-y-up"
        || coordinates.position != "[-unity.x,unity.y,unity.z]"
        || coordinates.normal != "[-unity.x,unity.y,unity.z]"
        || coordinates.translation != "[-unity.x,unity.y,unity.z]"
        || coordinates.rotation != "[unity.x,-unity.y,-unity.z,unity.w]"
        || coordinates.scale != "unchanged"
        || coordinates.uv != "[unity.u,1-unity.v]"
        || coordinates.inverse_bind_matrix != "H*unity*H where H=diag(-1,1,1,1)"
        || !(current_winding_contract || legacy_double_swap_contract)
        || coordinates.unit_scale != "1-unity-unit-equals-1-bevy-unit"
        || coordinates.origin_policy != "source-root-trs-unchanged-no-auto-centering"
        || coordinates.auto_centered
        || coordinates.auto_scaled
    {
        return invalid("source nativeCoordinateContract is missing or contradictory");
    }
    if source.logical_name.trim().is_empty() {
        return invalid("source logicalName is empty");
    }
    if !source.warnings.is_empty() {
        return invalid(format!(
            "source warnings must be resolved before publish: {}",
            source.warnings.join(" | ")
        ));
    }
    if let Some(proof) = &source.exact_mesh_selection_proof {
        if proof.schema != "ffone.exact-character-root-mesh-selection.v1"
            || proof.policy != "retain-only-meshes-under-proven-selected-character-roots"
            || proof.excluded_candidate_meshes == 0
            || proof.selected_meshes != source.meshes.len()
            || proof.source_warning
                != format!(
                    "Ignored {} mesh(es) from unrelated character roots in the shared preload range.",
                    proof.excluded_candidate_meshes
                )
            || proof.warning_disposition != "resolved-as-positive-character-root-ownership-proof"
        {
            return invalid("exactMeshSelectionProof is missing or contradictory");
        }
    }
    let expected = normalized_route(&source.exact_container_route);
    if expected.is_empty()
        || source.container_paths.len() != 1
        || source.matched_paths.len() != 1
        || normalized_route(&source.container_paths[0]) != expected
        || normalized_route(&source.matched_paths[0]) != expected
    {
        return invalid(
            "exact container route does not match one containerPaths/matchedPaths entry",
        );
    }
    for kfm in &source.kfm {
        if kfm.source != "kfm"
            || normalized_route(&kfm.path) != expected
            || !kfm.clips.is_empty()
            || !kfm.references.is_empty()
        {
            return invalid(
                "KFM metadata contains external clips/references that are not represented by the exact Unity hierarchy",
            );
        }
    }
    if source.model_hierarchy.source != "unity-transform-hierarchy" {
        return invalid("modelHierarchy is not an exact Unity Transform hierarchy");
    }
    if source.meshes.is_empty() {
        return invalid("logical model contains no bound meshes");
    }
    Ok(())
}

pub(super) fn validate_hierarchy_path(node: &SourceNode) -> Result<()> {
    if node.name.trim().is_empty()
        || node.name.chars().any(char::is_control)
        || node.name.contains(['/', '\\'])
    {
        return invalid(format!("invalid true hierarchy node name {:?}", node.name));
    }
    if node.path.trim_matches('/') != node.path
        || node.path.is_empty()
        || node.path.contains('\\')
        || node.path.contains("//")
        || node.path.chars().any(char::is_control)
    {
        return invalid(format!("invalid hierarchy path {:?}", node.path));
    }
    Ok(())
}

pub(super) fn validate_binding_root(
    source: &SourceDocument,
    binding: &SourceBinding,
    root_index: u32,
) -> Result<()> {
    let root = &source.model_hierarchy.nodes[root_index as usize];
    if binding.root_transform_name != source.logical_name
        || binding.root_transform_name != root.name
        || binding.root_transform_path_id != root.transform_path_id
    {
        return invalid(format!(
            "binding {:?} is not rooted at the sole logical model root",
            binding.transform_path
        ));
    }
    Ok(())
}

pub(super) fn validate_rigid_binding(
    mesh: &SourceMesh,
    binding: &SourceBinding,
    paths: &BTreeMap<String, u32>,
) -> Result<()> {
    let Some(skin) = &mesh.skin else {
        return Ok(());
    };
    if skin.source != "unity-rigid-mesh-attachment" {
        return invalid(format!(
            "MeshFilter mesh {:?} contains non-rigid skin source {:?}",
            mesh.name, skin.source
        ));
    }
    let (_, component_asset, component_path) = binding_component_key(binding)?;
    if skin.mesh_filter_asset_index != Some(component_asset)
        || skin.mesh_filter_path_id != Some(component_path)
        || skin.attachment_transform_path_id != Some(binding.transform_path_id)
    {
        return invalid(format!(
            "rigid attachment metadata differs from MeshFilter binding for {:?}",
            mesh.name
        ));
    }
    if skin.decode_warning.is_some()
        || !skin.inverse_bind_matrices.is_empty()
        || skin.renderer_asset_index.is_some()
        || skin.renderer_path_id.is_some()
        || skin.renderer_transform_path_id.is_some()
    {
        return invalid(format!(
            "rigid MeshFilter {:?} contains skinned-renderer-only data",
            mesh.name
        ));
    }
    if !skin.joint_paths.is_empty() {
        if skin.joint_paths.len() != 1 {
            return invalid(format!(
                "rigid MeshFilter {:?} has more than one attachment joint",
                mesh.name
            ));
        }
        let _ = suffix_node_path(paths, &skin.joint_paths[0], "rigid attachment joint")?;
    }
    let vertices = mesh.positions.len() / 3;
    if !skin.bone_indices.is_empty() || !skin.weights.is_empty() {
        if skin.bone_indices.len() != vertices.saturating_mul(4)
            || skin.weights.len() != vertices.saturating_mul(4)
        {
            return invalid(format!(
                "rigid MeshFilter {:?} weight cardinality differs from vertices",
                mesh.name
            ));
        }
        for (indices, weights) in skin
            .bone_indices
            .chunks_exact(4)
            .zip(skin.weights.chunks_exact(4))
        {
            if indices != [0, 0, 0, 0]
                || (weights[0] - 1.0).abs() > 1.0e-9
                || weights[1..].iter().any(|weight| weight.abs() > 1.0e-9)
            {
                return invalid(format!(
                    "MeshFilter {:?} has non-rigid weights and cannot be published without a skin",
                    mesh.name
                ));
            }
        }
    }
    if let Some(matrix) = &skin.rest_vertex_transform {
        if matrix.iter().flatten().any(|value| !value.is_finite()) {
            return invalid(format!(
                "rigid attachment transform for {:?} is non-finite",
                mesh.name
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_source_event_object_parameter(
    animation: &SourceAnimation,
    event: &SourceEvent,
) -> Result<()> {
    let context = format!(
        "animation {:?} event {:?}",
        animation.name, event.function_name
    );
    match &event.object_parameter_provenance {
        AnimationEventObjectParameterProvenance::Missing { .. } => {
            if event.object_parameter.is_some() {
                return invalid(format!(
                    "{context} missing object provenance contradicts objectParameter"
                ));
            }
        }
        AnimationEventObjectParameterProvenance::SerializedPointer {
            source_asset_index,
            file_id,
            path_id,
            interpretation,
        } => match interpretation {
            SerializedEventObjectParameterInterpretation::NullPathId => {
                if *path_id != 0 || event.object_parameter.is_some() {
                    return invalid(format!(
                        "{context} null-path-id object provenance is inconsistent"
                    ));
                }
            }
            SerializedEventObjectParameterInterpretation::NonNullUnresolved => {
                let expected = serde_json::json!({
                    "sourceAssetIndex": source_asset_index,
                    "fileId": file_id,
                    "pathId": path_id,
                });
                if *path_id == 0 || event.object_parameter.as_ref() != Some(&expected) {
                    return invalid(format!(
                        "{context} unresolved non-null object provenance is inconsistent"
                    ));
                }
                return invalid(format!(
                    "{context} has an unresolved non-null Unity object pointer"
                ));
            }
            SerializedEventObjectParameterInterpretation::NonNullMetadataOnly => {
                let expected = serde_json::json!({
                    "sourceAssetIndex": source_asset_index,
                    "fileId": file_id,
                    "pathId": path_id,
                });
                if *path_id == 0
                    || event.object_parameter.as_ref() != Some(&expected)
                    || event.function_name != "sound"
                    || event.string_parameter.trim().is_empty()
                {
                    return invalid(format!(
                        "{context} metadata-only object provenance is inconsistent"
                    ));
                }
            }
            SerializedEventObjectParameterInterpretation::NonNullLegacyUnused => {
                let expected = serde_json::json!({
                    "sourceAssetIndex": source_asset_index,
                    "fileId": file_id,
                    "pathId": path_id,
                });
                if *path_id == 0
                    || event.object_parameter.as_ref() != Some(&expected)
                    || !is_exact_larry_unused_end_event(
                        animation,
                        event,
                        *source_asset_index,
                        *file_id,
                        *path_id,
                    )
                {
                    return invalid(format!(
                        "{context} legacy-unused object provenance is inconsistent"
                    ));
                }
            }
        },
    }
    Ok(())
}

pub(super) fn validate_source_curve_recoveries(source: &SourceDocument, animation_index: usize) -> Result<()> {
    let animation = &source.animations[animation_index];
    let mut previous_order = None;
    for (recovery_index, recovery) in animation.animation_data.curve_recoveries.iter().enumerate() {
        let context = format!(
            "animation {:?} curve recovery {recovery_index}",
            animation.name
        );
        let Some((rank, raw_count)) = raw_trs_source(
            &animation.curve_counts,
            recovery.kind,
            recovery.source_encoding,
        ) else {
            return invalid(format!("{context} uses an impossible source encoding"));
        };
        let field = source_curve_field(recovery.kind);
        if recovery.source_encoding != EmptyTrsSourceEncoding::Plain
            || recovery.source.source_encoding != EmptyTrsSourceEncoding::Plain
            || recovery.source.field != field
            || recovery.reference.field != field
            || recovery.reference.kind != recovery.kind
            || recovery.reference.path != recovery.path
            || recovery.reference.source_encoding != recovery.source_encoding
        {
            return invalid(format!(
                "{context} source/reference identity differs from its kind/path"
            ));
        }
        let source_indices = &recovery.source.source_indices;
        if source_indices.len() < 2
            || source_indices.windows(2).any(|pair| pair[0] >= pair[1])
            || source_indices.iter().any(|index| *index >= raw_count)
            || recovery.source.source_target_curve_count != source_indices.len()
        {
            return invalid(format!(
                "{context} source indices/count are not strict and complete"
            ));
        }
        let order = (rank, source_indices[0]);
        if previous_order.is_some_and(|previous| previous >= order) {
            return invalid(format!(
                "animation {:?} curveRecoveries are not in canonical source order",
                animation.name
            ));
        }
        previous_order = Some(order);
        if !same_optional_source_f64_bits(recovery.source_sample_rate, animation.sample_rate)
            || !same_optional_source_f64_bits(
                recovery.reference.sample_rate,
                recovery.source_sample_rate,
            )
        {
            return invalid(format!("{context} sample rates do not match bit-exactly"));
        }
        if recovery.reference.path_id <= 0
            || recovery.reference.asset.is_empty()
            || recovery.reference.clip_name.is_empty()
        {
            return invalid(format!("{context} reference identity is invalid"));
        }

        let canonical_actual =
            source_track_at(animation, recovery.kind, recovery.canonical.track_index).ok_or_else(
                || invalid_error(format!("{context} canonical trackIndex is out of range")),
            )?;
        if !source_canonical_recovery_matches(&recovery.canonical, canonical_actual)
            || recovery.canonical.path != recovery.path
            || recovery.canonical.source_encoding != EmptyTrsSourceEncoding::Plain
            || !source_indices.contains(&recovery.canonical.source_index)
        {
            return invalid(format!(
                "{context} canonical provenance differs from the published source track"
            ));
        }
        let canonical_payload = source_constant_curve_payload(
            &recovery.canonical.keys,
            &recovery.canonical.duplicate_keys,
            recovery.canonical.source_key_count,
            &recovery.canonical.interpolation,
            animation.duration,
            &format!("{context} canonical"),
        )?;
        let expected_rejected = source_indices
            .iter()
            .copied()
            .filter(|source_index| *source_index != recovery.canonical.source_index)
            .collect::<Vec<_>>();
        if recovery.rejected.len() != expected_rejected.len() {
            return invalid(format!("{context} rejected provenance is incomplete"));
        }
        let mut source_payloads =
            vec![(recovery.canonical.source_index, canonical_payload.clone())];
        let mut conflicting = false;
        for (rejected_index, (track, expected_source_index)) in
            recovery.rejected.iter().zip(expected_rejected).enumerate()
        {
            if track.source_index != expected_source_index
                || track.path != recovery.path
                || track.source_encoding != EmptyTrsSourceEncoding::Plain
            {
                return invalid(format!(
                    "{context} rejected track {rejected_index} identity/order differs"
                ));
            }
            let payload = source_constant_curve_payload(
                &track.keys,
                &track.duplicate_keys,
                track.source_key_count,
                &track.interpolation,
                animation.duration,
                &format!("{context} rejected track {rejected_index}"),
            )?;
            conflicting |= payload != canonical_payload;
            source_payloads.push((track.source_index, payload));
        }
        if recovery.rejected.is_empty() || !conflicting {
            return invalid(format!(
                "{context} has no rejected conflicting source curve"
            ));
        }
        if !recovery.proof.exact_path
            || !recovery.proof.exact_sample_rate
            || !recovery.proof.all_source_curves_constant
            || !recovery.proof.canonical_matches_reference
            || recovery.proof.unique_canonical_candidate_count != 1
            || recovery.proof.matching_reference_count != 1
            || recovery.proof.source_target_curve_count != source_indices.len() as u64
        {
            return invalid(format!("{context} proof is incomplete or ambiguous"));
        }

        let mut matching_pairs = Vec::new();
        for (candidate_animation_index, candidate) in source.animations.iter().enumerate() {
            if candidate_animation_index == animation_index
                || !same_optional_source_f64_bits(
                    candidate.sample_rate,
                    recovery.source_sample_rate,
                )
                || source_target_binding_count(candidate, recovery.kind, &recovery.path) != 1
            {
                continue;
            }
            for candidate_track in source_tracks(candidate, recovery.kind) {
                if source_track_path(candidate_track) != recovery.path
                    || source_track_encoding(candidate_track) != EmptyTrsSourceEncoding::Plain
                {
                    continue;
                }
                let Some(candidate_payload) = source_track_constant_payload(
                    candidate_track,
                    candidate.duration,
                    "curve recovery sibling candidate",
                )?
                else {
                    continue;
                };
                for (target_source_index, target_payload) in &source_payloads {
                    if target_payload == &candidate_payload {
                        matching_pairs.push((
                            *target_source_index,
                            candidate_animation_index,
                            source_track_index(candidate_track),
                        ));
                    }
                }
            }
        }
        let declared_match = matching_pairs.iter().any(
            |(target_source_index, candidate_animation_index, candidate_source_index)| {
                let candidate = &source.animations[*candidate_animation_index];
                *target_source_index == recovery.canonical.source_index
                    && *candidate_source_index == recovery.reference.source_index
                    && candidate.asset.as_deref() == Some(recovery.reference.asset.as_str())
                    && candidate.path_id == Some(recovery.reference.path_id)
                    && candidate.name == recovery.reference.clip_name
            },
        );
        if matching_pairs.len() != 1 || !declared_match {
            return invalid(format!(
                "{context} does not have one unique exact sibling authority"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_source_recovery_times(
    values: impl IntoIterator<Item = f64>,
    duration: f64,
    context: &str,
) -> Result<()> {
    let mut previous = None;
    for value in values {
        if !value.is_finite()
            || !(value as f32).is_finite()
            || value < 0.0
            || value > duration + 1.0e-6
            || previous.is_some_and(|previous| previous >= value)
        {
            return invalid(format!("{context} key times are not finite and strict"));
        }
        previous = Some(value);
    }
    Ok(())
}

pub(super) fn validate_source_recovery_interpolation_vec3(
    keys: &[SourceVec3Key],
    interpolation: &str,
    context: &str,
) -> Result<()> {
    validate_source_recovery_interpolation(
        keys.iter()
            .map(|key| (key.in_tangent.is_some(), key.out_tangent.is_some())),
        interpolation,
        context,
    )
}

pub(super) fn validate_source_recovery_interpolation_quaternion(
    keys: &[SourceQuatKey],
    interpolation: &str,
    context: &str,
) -> Result<()> {
    validate_source_recovery_interpolation(
        keys.iter()
            .map(|key| (key.in_tangent.is_some(), key.out_tangent.is_some())),
        interpolation,
        context,
    )
}

pub(super) fn validate_source_recovery_interpolation(
    tangents: impl IntoIterator<Item = (bool, bool)>,
    interpolation: &str,
    context: &str,
) -> Result<()> {
    let tangents = tangents.into_iter().collect::<Vec<_>>();
    let expected = if tangents.iter().all(|value| *value == (true, true)) {
        "CUBICSPLINE"
    } else if tangents.iter().all(|value| *value == (false, false)) {
        "LINEAR"
    } else {
        return invalid(format!("{context} only partially preserves tangents"));
    };
    if interpolation != expected {
        return invalid(format!(
            "{context} interpolation differs from exact tangents"
        ));
    }
    Ok(())
}

pub(super) fn validate_finite_source_values(values: &[f64], context: &str) -> Result<()> {
    if values
        .iter()
        .any(|value| !value.is_finite() || !(*value as f32).is_finite())
    {
        return invalid(format!("{context} contains a non-finite value"));
    }
    Ok(())
}

pub(super) fn validate_source_identity(
    animation_name: &str,
    label: &str,
    kind: EmptyTrsBindingKind,
    encoding: EmptyTrsSourceEncoding,
    source_index: usize,
    raw_count: usize,
    identities: &mut BTreeSet<(EmptyTrsBindingKind, EmptyTrsSourceEncoding, usize)>,
) -> Result<()> {
    if source_index >= raw_count {
        return invalid(format!(
            "animation {animation_name:?} {label} sourceIndex {source_index} is outside raw {kind:?}/{encoding:?} count {raw_count}"
        ));
    }
    if !identities.insert((kind, encoding, source_index)) {
        return invalid(format!(
            "animation {animation_name:?} {label} repeats source identity {kind:?}/{encoding:?}/{source_index}"
        ));
    }
    Ok(())
}

pub(super) fn validate_source_track_keys(
    track: &SourceCanonicalTrack<'_>,
    animation_name: &str,
    label: &str,
) -> Result<()> {
    match track {
        SourceCanonicalTrack::Vec3(track) => validate_vec3_key_provenance(
            &track.keys,
            &track.duplicate_keys,
            track.source_key_count,
            animation_name,
            label,
        ),
        SourceCanonicalTrack::Quaternion(track) => validate_quaternion_key_provenance(
            &track.keys,
            &track.duplicate_keys,
            track.source_key_count,
            animation_name,
            label,
        ),
    }
}

pub(super) fn validate_vec3_key_provenance(
    keys: &[SourceVec3Key],
    duplicate_keys: &[SourceDuplicateAnimationKey],
    source_key_count: usize,
    animation_name: &str,
    label: &str,
) -> Result<()> {
    validate_key_identity_coverage(
        keys.iter().map(|key| key.source_key_index),
        duplicate_keys,
        source_key_count,
        animation_name,
        label,
    )?;
    for (duplicate_index, duplicate) in duplicate_keys.iter().enumerate() {
        let canonical = keys.get(duplicate.canonical_key_index).ok_or_else(|| {
            invalid_error(format!(
                "animation {animation_name:?} {label} duplicateKeys[{duplicate_index}] canonicalKeyIndex is out of range"
            ))
        })?;
        let expected = SourceExactTrsKeyPayload::Vec3(SourceVec3KeyPayload {
            time: canonical.time,
            value: canonical.value,
            in_tangent: canonical.in_tangent,
            out_tangent: canonical.out_tangent,
            tangent_mode: canonical.tangent_mode,
        });
        if duplicate.key != expected {
            return invalid(format!(
                "animation {animation_name:?} {label} duplicateKeys[{duplicate_index}] differs from its canonical key"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_quaternion_key_provenance(
    keys: &[SourceQuatKey],
    duplicate_keys: &[SourceDuplicateAnimationKey],
    source_key_count: usize,
    animation_name: &str,
    label: &str,
) -> Result<()> {
    validate_key_identity_coverage(
        keys.iter().map(|key| key.source_key_index),
        duplicate_keys,
        source_key_count,
        animation_name,
        label,
    )?;
    for (duplicate_index, duplicate) in duplicate_keys.iter().enumerate() {
        let canonical = keys.get(duplicate.canonical_key_index).ok_or_else(|| {
            invalid_error(format!(
                "animation {animation_name:?} {label} duplicateKeys[{duplicate_index}] canonicalKeyIndex is out of range"
            ))
        })?;
        let expected = SourceExactTrsKeyPayload::Quaternion(SourceQuatKeyPayload {
            time: canonical.time,
            value: canonical.value,
            in_tangent: canonical.in_tangent,
            out_tangent: canonical.out_tangent,
            tangent_mode: canonical.tangent_mode,
        });
        if duplicate.key != expected {
            return invalid(format!(
                "animation {animation_name:?} {label} duplicateKeys[{duplicate_index}] differs from its canonical key"
            ));
        }
    }
    Ok(())
}
