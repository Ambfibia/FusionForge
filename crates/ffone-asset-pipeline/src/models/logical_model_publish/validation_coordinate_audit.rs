use super::*;

pub(super) fn validate_key_identity_coverage(
    canonical_indices: impl IntoIterator<Item = usize>,
    duplicate_keys: &[SourceDuplicateAnimationKey],
    source_key_count: usize,
    animation_name: &str,
    label: &str,
) -> Result<()> {
    let canonical_indices = canonical_indices.into_iter().collect::<Vec<_>>();
    if canonical_indices.is_empty() || source_key_count == 0 {
        return invalid(format!(
            "animation {animation_name:?} {label} has no canonical/source keys"
        ));
    }
    let mut identities = BTreeSet::new();
    let mut previous = None;
    for source_index in canonical_indices {
        if source_index >= source_key_count
            || previous.is_some_and(|previous| previous >= source_index)
            || !identities.insert(source_index)
        {
            return invalid(format!(
                "animation {animation_name:?} {label} canonical sourceKeyIndex order is invalid"
            ));
        }
        previous = Some(source_index);
    }
    previous = None;
    for (duplicate_index, duplicate) in duplicate_keys.iter().enumerate() {
        if duplicate.source_key_index >= source_key_count
            || previous.is_some_and(|previous| previous >= duplicate.source_key_index)
            || !identities.insert(duplicate.source_key_index)
        {
            return invalid(format!(
                "animation {animation_name:?} {label} duplicateKeys[{duplicate_index}] source identity/order is invalid"
            ));
        }
        previous = Some(duplicate.source_key_index);
    }
    if identities.len() != source_key_count
        || (0..source_key_count).any(|source_index| !identities.contains(&source_index))
    {
        return invalid(format!(
            "animation {animation_name:?} {label} sourceKeyCount is not fully covered"
        ));
    }
    Ok(())
}

pub(super) fn validate_duplicate_trs_source(
    animation: &SourceAnimation,
    binding: &SourceDuplicateTrsBinding,
) -> Result<()> {
    match binding.relation {
        DuplicateTrsRelation::Identical => {
            if binding.resolution_proof.is_some() {
                return invalid(format!(
                    "animation {:?} identical duplicate TRS binding unexpectedly has overwrite proof",
                    animation.name
                ));
            }
        }
        DuplicateTrsRelation::SerializedLastWriteWins => {
            let proof = binding.resolution_proof.as_ref().ok_or_else(|| {
                invalid_error(format!(
                    "animation {:?} serialized-last-write duplicate has no proof",
                    animation.name
                ))
            })?;
            if proof.rule != SerializedCurveOverwriteRule::LaterSerializedBindingOverwritesEarlier
                || animation.asset.as_deref() != Some(proof.asset.as_str())
                || animation.path_id != Some(proof.path_id)
                || animation.name != proof.clip_name
                || !proof.exact_target_path
                || !proof.exact_source_array_order
                || binding.source_encoding != binding.canonical_source_encoding
                || binding.source_index >= binding.canonical_source_index
            {
                return invalid(format!(
                    "animation {:?} serialized-last-write duplicate proof does not match its exact source/order",
                    animation.name
                ));
            }
        }
    }
    match binding.kind {
        EmptyTrsBindingKind::Translation | EmptyTrsBindingKind::Scale => {
            let tracks = if binding.kind == EmptyTrsBindingKind::Translation {
                &animation.animation_data.translations
            } else {
                &animation.animation_data.scales
            };
            let canonical = tracks.get(binding.canonical_track_index).ok_or_else(|| {
                invalid_error(format!(
                    "animation {:?} duplicate canonicalTrackIndex is out of range",
                    animation.name
                ))
            })?;
            let SourceDuplicateTrsKeys::Vec3(keys) = &binding.keys else {
                return invalid(format!(
                    "animation {:?} duplicate {:?} keys have the wrong component shape",
                    animation.name, binding.kind
                ));
            };
            validate_vec3_key_provenance(
                keys,
                &binding.duplicate_keys,
                binding.source_key_count,
                &animation.name,
                "duplicate TRS binding",
            )?;
            if binding.path != canonical.path
                || binding.canonical_source_index != canonical.source_index
                || binding.canonical_source_encoding != canonical.source_encoding
            {
                return invalid(format!(
                    "animation {:?} duplicate TRS binding does not target its canonical track",
                    animation.name
                ));
            }
            if binding.relation == DuplicateTrsRelation::Identical
                && (binding.source_key_count != canonical.source_key_count
                    || binding.duplicate_keys != canonical.duplicate_keys
                    || keys != &canonical.keys)
            {
                return invalid(format!(
                    "animation {:?} identical duplicate TRS binding does not exactly match its canonical track",
                    animation.name
                ));
            }
            if binding.relation == DuplicateTrsRelation::SerializedLastWriteWins
                && binding.source_key_count == canonical.source_key_count
                && binding.duplicate_keys == canonical.duplicate_keys
                && keys == &canonical.keys
            {
                return invalid(format!(
                    "animation {:?} serialized-last-write duplicate is actually identical",
                    animation.name
                ));
            }
        }
        EmptyTrsBindingKind::Rotation => {
            let canonical = animation
                .animation_data
                .rotations
                .get(binding.canonical_track_index)
                .ok_or_else(|| {
                    invalid_error(format!(
                        "animation {:?} duplicate canonicalTrackIndex is out of range",
                        animation.name
                    ))
                })?;
            let SourceDuplicateTrsKeys::Quaternion(keys) = &binding.keys else {
                return invalid(format!(
                    "animation {:?} duplicate rotation keys have the wrong component shape",
                    animation.name
                ));
            };
            validate_quaternion_key_provenance(
                keys,
                &binding.duplicate_keys,
                binding.source_key_count,
                &animation.name,
                "duplicate TRS binding",
            )?;
            if binding.path != canonical.path
                || binding.canonical_source_index != canonical.source_index
                || binding.canonical_source_encoding != canonical.source_encoding
            {
                return invalid(format!(
                    "animation {:?} duplicate TRS binding does not target its canonical track",
                    animation.name
                ));
            }
            if binding.relation == DuplicateTrsRelation::Identical
                && (binding.source_key_count != canonical.source_key_count
                    || binding.duplicate_keys != canonical.duplicate_keys
                    || keys != &canonical.keys)
            {
                return invalid(format!(
                    "animation {:?} identical duplicate TRS binding does not exactly match its canonical track",
                    animation.name
                ));
            }
            if binding.relation == DuplicateTrsRelation::SerializedLastWriteWins
                && binding.source_key_count == canonical.source_key_count
                && binding.duplicate_keys == canonical.duplicate_keys
                && keys == &canonical.keys
            {
                return invalid(format!(
                    "animation {:?} serialized-last-write duplicate is actually identical",
                    animation.name
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn coordinate_audit(model: &NativeModel) -> Result<CoordinateAuditReport> {
    let globals = model_global_transforms(model)?;
    let node_paths = model_node_paths(model)?;
    let root = model
        .roots
        .first()
        .and_then(|index| model.nodes.get(*index as usize))
        .ok_or_else(|| invalid_error("coordinate audit has no exact root node"))?;
    let mut raw_bounds = BoundsAccumulator::default();
    let mut world_bounds = BoundsAccumulator::default();
    let mut skinning_basis_parity_max_error = None::<f64>;
    let mut current_pose_bind_identity_deviation_max = None::<f64>;
    let mut current_pose_bind_identity_deviation_worst = None;
    let mut inverse_bind_matrices = 0u64;

    for skin in &model.skins {
        add_count(
            &mut inverse_bind_matrices,
            skin.inverse_bind_matrices.len(),
            "coordinate inverse-bind count",
        )?;
    }

    for (node_index, node) in model.nodes.iter().enumerate() {
        let Some(mesh_index) = node.mesh else {
            continue;
        };
        let mesh = &model.meshes[mesh_index as usize];
        let mesh_global = globals[node_index];
        let skin_index = node.skin;
        let skin = skin_index.map(|skin| &model.skins[skin as usize]);
        let joint_positive_weight_sums = skin.map(|skin| {
            let mut sums = vec![0.0_f64; skin.joints.len()];
            if let Some(primitive) = mesh.primitives.first() {
                for (joints, weights) in primitive.joints.iter().zip(&primitive.weights) {
                    for influence in 0..4 {
                        let weight = weights[influence].max(0.0);
                        if weight > 0.0 {
                            sums[joints[influence] as usize] += weight;
                        }
                    }
                }
            }
            sums
        });
        let skin_matrices = skin
            .map(|skin| {
                if mesh_global.determinant().abs() <= f32::EPSILON {
                    return invalid(
                        "coordinate audit found a non-invertible skinned mesh transform",
                    );
                }
                let inverse_mesh = mesh_global.inverse();
                skin.joints
                    .iter()
                    .zip(&skin.inverse_bind_matrices)
                    .enumerate()
                    .map(|(joint_palette_index, (joint, inverse_bind))| {
                        let joint_world = globals[*joint as usize];
                        let inverse_bind = native_matrix(*inverse_bind);
                        let native_skin = joint_world * inverse_bind;
                        let parity_error = skinning_basis_parity_error(joint_world, inverse_bind);
                        skinning_basis_parity_max_error = Some(
                            skinning_basis_parity_max_error
                                .map_or(parity_error, |current| current.max(parity_error)),
                        );

                        // This is deliberately a descriptive deviation, not a
                        // bind-pose validity gate. Unity bind poses are
                        // boneWorldToLocal * rendererLocalToWorld; a serialized
                        // skeleton pose may differ substantially from the mesh's
                        // authored bind pose while the skinning equation remains
                        // exact.
                        let local_skin = inverse_mesh * native_skin;
                        let detail = matrix_identity_deviation(local_skin);
                        if current_pose_bind_identity_deviation_max
                            .is_none_or(|current| detail.max_abs > current)
                        {
                            current_pose_bind_identity_deviation_max = Some(detail.max_abs);
                            current_pose_bind_identity_deviation_worst =
                                Some(CurrentPoseBindIdentityDeviation {
                                    metric: "maxAbs(inverse(meshGlobal)*jointGlobal*inverseBind-identity)"
                                        .to_owned(),
                                    mesh_node_index: u32_count(node_index, "mesh node index")?,
                                    mesh_node_path: node_paths[node_index].clone(),
                                    mesh_name: mesh.name.clone(),
                                    skin_index: skin_index.expect("skin closure requires index"),
                                    skin_name: skin.name.clone(),
                                    joint_palette_index: u32_count(
                                        joint_palette_index,
                                        "joint palette index",
                                    )?,
                                    joint_node_index: *joint,
                                    joint_node_path: node_paths[*joint as usize].clone(),
                                    joint_positive_weight_sum: joint_positive_weight_sums
                                        .as_ref()
                                        .expect("skin closure requires weight sums")
                                        [joint_palette_index],
                                    max_abs_deviation: detail.max_abs,
                                    max_element_row: u32_count(detail.row, "matrix row")?,
                                    max_element_column: u32_count(detail.column, "matrix column")?,
                                    max_element_actual: detail.actual,
                                    max_element_identity: detail.identity,
                                });
                        }
                        Ok(native_skin)
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;

        for primitive in &mesh.primitives {
            for (vertex_index, source_position) in primitive.positions.iter().enumerate() {
                let local = Vec3::new(
                    source_position[0] as f32,
                    source_position[1] as f32,
                    source_position[2] as f32,
                );
                raw_bounds.include(local)?;
                let world = if let (Some(matrices), Some(_skin)) = (&skin_matrices, skin) {
                    let joints = primitive.joints.get(vertex_index).ok_or_else(|| {
                        invalid_error("coordinate audit skinned vertex lost JOINTS_0")
                    })?;
                    let weights = primitive.weights.get(vertex_index).ok_or_else(|| {
                        invalid_error("coordinate audit skinned vertex lost WEIGHTS_0")
                    })?;
                    let homogeneous = Vec4::new(local.x, local.y, local.z, 1.0);
                    let mut blended = Vec4::ZERO;
                    for influence in 0..4 {
                        let weight = weights[influence] as f32;
                        if weight == 0.0 {
                            continue;
                        }
                        let matrix = matrices.get(joints[influence] as usize).ok_or_else(|| {
                            invalid_error("coordinate audit joint palette index is out of bounds")
                        })?;
                        blended += (*matrix * homogeneous) * weight;
                    }
                    if !blended.is_finite() || blended.w.abs() <= f32::EPSILON {
                        return invalid("coordinate audit produced an invalid skinned rest vertex");
                    }
                    blended.truncate() / blended.w
                } else {
                    mesh_global.transform_point3(local)
                };
                world_bounds.include(world)?;
            }
        }
    }

    Ok(CoordinateAuditReport {
        status: "artifact-space-proven-runtime-spawn-policy-pending".to_owned(),
        runtime_spawn_policy: "runtime-archetype-dependent-pending".to_owned(),
        runtime_spawn_policy_note: "The model artifact preserves authored root TRS exactly. Legacy NPC spawning may replace root position/rotation/scale, while Nano spawning may retain authored scale; runtime archetype policy is a separate acceptance gate."
            .to_owned(),
        root_translation: root.translation.map(|value| f64::from(value as f32)),
        root_rotation: root.rotation.map(|value| f64::from(value as f32)),
        root_scale: root.scale.map(|value| f64::from(value as f32)),
        unit_scale: 1.0,
        auto_centered: false,
        auto_scaled: false,
        raw_local_bounds: raw_bounds.finish("raw local")?,
        static_rest_world_bounds: world_bounds.finish("static rest world")?,
        source_triangle_winding: "unity-source-indices-in-h-reflected-native-space".to_owned(),
        published_triangle_winding: "gltf-counter-clockwise-per-triangle-normal-aligned".to_owned(),
        winding_swap_applied_per_triangle: true,
        inverse_bind_matrices,
        skinning_basis_parity_status: if model.skins.is_empty() {
            "not-applicable-rigid".to_owned()
        } else {
            "unity-to-native-h-conjugation-algebraically-proven".to_owned()
        },
        skinning_basis_parity_max_error,
        current_pose_bind_identity_deviation_max,
        current_pose_bind_identity_deviation_worst,
    })
}

pub(super) fn skinning_basis_parity_error(joint_native: Mat4, inverse_bind_native: Mat4) -> f64 {
    let h = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    let joint_unity = h * joint_native * h;
    let inverse_bind_unity = h * inverse_bind_native * h;
    let native_skin = joint_native * inverse_bind_native;
    let unity_skin_reexpressed_native = h * joint_unity * inverse_bind_unity * h;
    matrix_max_difference(native_skin, unity_skin_reexpressed_native)
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::LogicalModelPublish(message.into())
}
