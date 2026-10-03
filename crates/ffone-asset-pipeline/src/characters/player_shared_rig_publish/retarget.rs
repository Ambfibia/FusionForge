use super::*;

/// Transfer local motion relative to the source rest pose, not its absolute
/// bone lengths/orientations. Tangents are vectors and must not receive offsets.
pub(super) fn retarget_rest_pose(
    clip: &mut AnimationClip,
    target: &[PlayerRigNode],
    source: &[PlayerRigNode],
) -> Result<()> {
    let relative = |path: &str| path.split_once('/').map_or("", |(_, path)| path).to_owned();
    let sources: BTreeMap<_, _> = source.iter().map(|n| (relative(&n.full_path), n)).collect();
    for channel in &mut clip.channels {
        let dst = target
            .get(channel.target_node as usize)
            .ok_or_else(|| rig_message("retarget target"))?;
        let src = sources
            .get(&relative(&dst.full_path))
            .ok_or_else(|| rig_message(format!("retarget source {}", dst.full_path)))?;
        let norm: f64 = src.rotation.iter().map(|v| v * v).sum();
        if norm < 1e-12 || src.scale.iter().any(|v| v.abs() < 1e-12) {
            return rig_error("invalid source rest transform".to_owned());
        }
        let inverse = quat_conjugate(src.rotation).map(|v| v / norm);
        let correction = quat_multiply(dst.rotation, inverse);
        let apply = |values: &mut TrackValues, tangent: bool| match values {
            TrackValues::Translation(values) => {
                for value in values {
                    if !tangent {
                        for axis in 0..3 {
                            value[axis] += dst.translation[axis] - src.translation[axis];
                        }
                    }
                }
            }
            TrackValues::Rotation(values) => {
                for value in values {
                    *value = quat_multiply(correction, *value);
                }
            }
            TrackValues::Scale(values) => {
                for value in values {
                    for axis in 0..3 {
                        value[axis] *= dst.scale[axis] / src.scale[axis];
                    }
                }
            }
        };
        apply(&mut channel.values, false);
        if let Some(values) = &mut channel.in_tangents {
            apply(values, true);
        }
        if let Some(values) = &mut channel.out_tangents {
            apply(values, true);
        }
    }
    Ok(())
}

/// In-memory raw-source conversion for the thirteen female-authored male
/// extensions. Return original and retargeted payloads so publication can
/// reject unrelated local curve edits instead of silently replacing them.
pub fn rebuild_male_emote_payloads(contract: &[u8], objects: Value) -> Result<(Vec<u8>, Vec<u8>)> {
    let parse_error = |source| PipelineError::Json {
        path: "player emote conversion".into(),
        source,
    };
    let contract: PlayerSharedRigContract =
        serde_json::from_slice(contract).map_err(parse_error)?;
    let objects: Vec<DumpObject> = serde_json::from_value(objects).map_err(parse_error)?;
    let by_path_id: HashMap<_, _> = objects
        .iter()
        .enumerate()
        .map(|(i, o)| (o.path_id, i))
        .collect();
    if by_path_id.len() != objects.len() {
        return rig_error("duplicate source clips".to_owned());
    }
    let catalog = DumpCatalog {
        objects,
        by_path_id,
    };
    let male = contract
        .genders
        .iter()
        .find(|g| g.gender == PlayerRigGender::Male)
        .ok_or_else(|| rig_message("male rig"))?;
    let female = contract
        .genders
        .iter()
        .find(|g| g.gender == PlayerRigGender::Female)
        .ok_or_else(|| rig_message("female rig"))?;
    let mut animations = Vec::new();
    for spec in CUSTOM_RUNTIME_CLIPS {
        let (name, id, _, minimum) = spec.source_for(PlayerRigGender::Male);
        if !name.starts_with("f_") {
            continue;
        }
        animations.push(decode_retargeted_custom_clip(
            &catalog,
            &male.nodes,
            "m",
            spec.semantic_name,
            name,
            id,
            minimum,
        )?);
    }
    let mut model = NativeModel {
        schema: ffone_skinned_model::MODEL_SCHEMA.to_owned(),
        name: "m".to_owned(),
        native_coordinate_contract: exact_native_coordinate_contract(),
        roots: vec![0],
        nodes: male
            .nodes
            .iter()
            .map(|node| ModelNode {
                name: node.true_name.clone(),
                legacy_name: None,
                legacy_sibling_ordinal: None,
                parent: node.parent_actor_bone_index,
                translation: node.translation,
                rotation: node.rotation,
                scale: node.scale,
                mesh: None,
                skin: None,
            })
            .collect(),
        meshes: vec![],
        skins: vec![],
        materials: vec![],
        textures: vec![],
        samplers: vec![],
        animations,
    };
    let before = encode_glb(&model).map_err(|e| rig_message(e.to_string()))?;
    for clip in &mut model.animations {
        retarget_rest_pose(clip, &male.nodes, &female.nodes)?;
    }
    let after = encode_glb(&model).map_err(|e| rig_message(e.to_string()))?;
    Ok((before, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retarget_preserves_motion_and_cubic_derivatives_across_different_rest_poses() {
        let half = std::f64::consts::FRAC_1_SQRT_2;
        let node = |root: &str, translation, rotation, scale| PlayerRigNode {
            actor_bone_index: 0,
            source_transform_path_id: 1,
            source_game_object_path_id: 2,
            true_name: "shoulder".into(),
            full_path: format!("{root}/shoulder"),
            parent_actor_bone_index: None,
            translation,
            rotation,
            scale,
        };
        let src = node("w", [1.0, 2.0, 3.0], [half, 0.0, 0.0, half], [2.0; 3]);
        let dst = node("m", [4.0, 6.0, 8.0], [0.0, half, 0.0, half], [3.0; 3]);
        let delta = [0.0, 0.0, half, half];
        let tangent = [0.25, -0.5, 0.75, -1.0];
        let channel = |values, tangents| AnimationChannel {
            target_node: 0,
            source_index: 0,
            source_encoding: EmptyTrsSourceEncoding::Plain,
            source_key_count: 2,
            source_key_indices: vec![0, 1],
            duplicate_keys: vec![],
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values,
            in_tangents: Some(tangents),
            out_tangents: None,
            tangent_modes: vec![],
        };
        let mut clip = AnimationClip {
            name: "emote".into(),
            duration: 1.0,
            declared_duration: None,
            keyed_duration: Some(1.0),
            event_duration: None,
            sample_rate: None,
            wrap_mode: None,
            looped: false,
            metadata: AnimationMetadata::default(),
            channels: vec![
                channel(
                    TrackValues::Translation(vec![src.translation, [2.0, 4.0, 6.0]]),
                    TrackValues::Translation(vec![[7.0, 8.0, 9.0]; 2]),
                ),
                channel(
                    TrackValues::Rotation(vec![src.rotation, quat_multiply(src.rotation, delta)]),
                    TrackValues::Rotation(vec![tangent; 2]),
                ),
                channel(
                    TrackValues::Scale(vec![src.scale, [4.0; 3]]),
                    TrackValues::Scale(vec![[1.0; 3]; 2]),
                ),
            ],
        };
        retarget_rest_pose(
            &mut clip,
            std::slice::from_ref(&dst),
            std::slice::from_ref(&src),
        )
        .unwrap();
        assert_eq!(
            clip.channels[0].values,
            TrackValues::Translation(vec![dst.translation, [5.0, 8.0, 11.0]])
        );
        assert_eq!(
            clip.channels[0].in_tangents,
            Some(TrackValues::Translation(vec![[7.0, 8.0, 9.0]; 2]))
        );
        let TrackValues::Rotation(rotations) = &clip.channels[1].values else {
            panic!()
        };
        for (actual, expected) in rotations
            .iter()
            .zip([dst.rotation, quat_multiply(dst.rotation, delta)])
        {
            for axis in 0..4 {
                assert!((actual[axis] - expected[axis]).abs() < 1e-12);
            }
        }
        let correction = quat_multiply(dst.rotation, quat_conjugate(src.rotation));
        let Some(TrackValues::Rotation(tangents)) = &clip.channels[1].in_tangents else {
            panic!()
        };
        for axis in 0..4 {
            assert!((tangents[0][axis] - quat_multiply(correction, tangent)[axis]).abs() < 1e-12);
        }
        assert_eq!(
            clip.channels[2].values,
            TrackValues::Scale(vec![dst.scale, [6.0; 3]])
        );
        assert_eq!(
            clip.channels[2].in_tangents,
            Some(TrackValues::Scale(vec![[1.5; 3]; 2]))
        );
    }
}
