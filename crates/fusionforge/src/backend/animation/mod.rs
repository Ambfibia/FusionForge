use super::super::*;

pub(in super::super) fn imported_skeleton_to_preview(skeleton: &fusionforge::modding::ImportedSkeleton) -> JsonValue {
    let joints = skeleton
        .joints
        .iter()
        .map(|joint| {
            json!({
                "nodeIndex": joint.node_index,
                "path": joint.path,
                "parent": joint.parent_path,
                "translation": [joint.translation.0, joint.translation.1, joint.translation.2],
                "rotation": [joint.rotation.0, joint.rotation.1, joint.rotation.2, joint.rotation.3],
                "scale": [joint.scale.0, joint.scale.1, joint.scale.2],
            })
        })
        .collect::<Vec<_>>();
    json!({
        "source": "gltf-skeleton",
        "joints": joints,
        "skinJointPaths": skeleton.skin_joint_paths,
    })
}

pub(in super::super) fn imported_animation_to_preview(
    clip: &fusionforge::modding::ImportedAnimationClip,
    source_path: &Path,
    index: usize,
) -> JsonValue {
    json!({
        "source": "gltf-animation",
        "asset": source_path.to_string_lossy(),
        "pathId": index,
        "name": clip.name,
        "duration": clip.duration,
        "sampleRate": clip.sample_rate,
        "loop": true,
        "canPreviewPose": true,
        "previewSupport": "sampleable-trs",
        "curveCounts": {
            "rotation": clip.rotations.len(),
            "compressedRotation": 0,
            "euler": 0,
            "position": clip.translations.len(),
            "scale": clip.scales.len(),
        },
        "animationData": {
            "translations": imported_vec3_curves_to_preview(&clip.translations),
            "rotations": imported_quat_curves_to_preview(&clip.rotations),
            "scales": imported_vec3_curves_to_preview(&clip.scales),
        },
    })
}

pub(in super::super) fn unity_animation_curve_count(body: &fusionforge::UnityValue, key: &str) -> usize {
    fusionforge::value_array(body.get(key)).len()
}

pub(in super::super) fn unity_animation_event_count(body: &fusionforge::UnityValue) -> usize {
    unity_animation_curve_count(body, "m_Events")
}

pub(in super::super) fn unity_animation_duration(body: &fusionforge::UnityValue) -> Option<f64> {
    body.get("m_StopTime")
        .and_then(fusionforge::UnityValue::as_f64)
        .or_else(|| {
            let clip = body.get("m_MuscleClip")?;
            let stop = clip.get("m_StopTime")?.as_f64()?;
            let start = clip
                .get("m_StartTime")
                .and_then(fusionforge::UnityValue::as_f64)
                .unwrap_or(0.0);
            Some((stop - start).max(0.0))
        })
}

pub(in super::super) fn unity_animation_clip_preview(
    asset: &fusionforge::Asset,
    path_id: i64,
    body: &fusionforge::UnityValue,
) -> JsonValue {
    let rotation_curves = unity_animation_curve_count(body, "m_RotationCurves");
    let compressed_rotation_curves =
        unity_animation_curve_count(body, "m_CompressedRotationCurves");
    let euler_curves = unity_animation_curve_count(body, "m_EulerCurves");
    let position_curves = unity_animation_curve_count(body, "m_PositionCurves");
    let scale_curves = unity_animation_curve_count(body, "m_ScaleCurves");
    let float_curves = unity_animation_curve_count(body, "m_FloatCurves");
    let pptr_curves = unity_animation_curve_count(body, "m_PPtrCurves");
    let sample_rate = body
        .get("m_SampleRate")
        .and_then(fusionforge::UnityValue::as_f64)
        .or_else(|| {
            body.get("m_FrameRate")
                .and_then(fusionforge::UnityValue::as_f64)
        });
    json!({
        "source": "unity-animation-clip",
        "asset": asset.name,
        "pathId": path_id,
        "name": fusionforge::object_name(body),
        "duration": unity_animation_duration(body),
        "sampleRate": sample_rate,
        "wrapMode": body.get("m_WrapMode").and_then(fusionforge::UnityValue::as_i64),
        "curveCounts": {
            "rotation": rotation_curves,
            "compressedRotation": compressed_rotation_curves,
            "euler": euler_curves,
            "position": position_curves,
            "scale": scale_curves,
            "float": float_curves,
            "pptr": pptr_curves,
            "events": unity_animation_event_count(body),
        },
        "hasPoseCurves": rotation_curves + compressed_rotation_curves + euler_curves + position_curves + scale_curves > 0,
        "canPreviewPose": false,
        "previewSupport": "metadata-only",
        "previewUnsupportedReason": "Legacy Unity AnimationClip keyframes and renderer skin are not exported to the native preview yet.",
    })
}

pub(in super::super) fn preview_animation_semantic_key(animation: &JsonValue) -> String {
    serde_json::to_string(&preview_value_without_fields(
        Some(animation),
        &["asset", "pathId"],
    ))
    .unwrap_or_default()
}

pub(in super::super) fn skinned_renderer_bone_names(
    env: &fusionforge::UnityEnvironment,
    renderer_key: (usize, i64),
) -> Vec<String> {
    let Some(asset) = env.assets.get(renderer_key.0) else {
        return Vec::new();
    };
    let Some(info) = asset.objects.get(&renderer_key.1) else {
        return Vec::new();
    };
    let Ok(renderer) = asset.read_object(renderer_key.0, info) else {
        return Vec::new();
    };
    renderer
        .get("m_Bones")
        .and_then(fusionforge::UnityValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(|bone| {
            let pointer = bone.as_pointer()?;
            let transform_key = env.resolve_pointer(pointer).ok()?;
            let transform_asset = env.assets.get(transform_key.asset)?;
            let transform_info = transform_asset.objects.get(&transform_key.path_id)?;
            let transform = transform_asset
                .read_object(transform_key.asset, transform_info)
                .ok()?;
            let gameobject_pointer = transform.get("m_GameObject")?.as_pointer()?;
            let gameobject_key = env.resolve_pointer(gameobject_pointer).ok()?;
            let gameobject_asset = env.assets.get(gameobject_key.asset)?;
            let gameobject_info = gameobject_asset.objects.get(&gameobject_key.path_id)?;
            let gameobject = gameobject_asset
                .read_object(gameobject_key.asset, gameobject_info)
                .ok()?;
            Some(fusionforge::object_name(&gameobject))
        })
        .collect()
}

pub(in super::super) fn clean_gltf_runtime_bone_name(name: &str) -> String {
    let trimmed = name.trim();
    let without_ik = trimmed
        .split_once("-IKTarget")
        .map(|(base, _)| base)
        .unwrap_or(trimmed);
    without_ik
        .rsplit_once('.')
        .filter(|(_, suffix)| suffix.chars().all(|ch| ch.is_ascii_digit()))
        .map(|(base, _)| base)
        .unwrap_or(without_ik)
        .to_string()
}

pub(in super::super) fn clean_gltf_legacy_safe_bone_name(name: &str) -> String {
    let mut result = String::new();
    let mut previous_underscore = false;
    for ch in name.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '_' {
            ch
        } else {
            '_'
        };
        if mapped == '_' {
            if previous_underscore {
                continue;
            }
            previous_underscore = true;
        } else {
            previous_underscore = false;
        }
        result.push(mapped);
    }
    result.trim_matches('_').to_string()
}

pub(in super::super) fn clean_gltf_side_bone_group_rank(name: &str) -> u16 {
    if name.starts_with("calf") {
        0
    } else if name.starts_with("clavicle") {
        1
    } else if name.starts_with("finger") {
        2
    } else if name.starts_with("foot") {
        3
    } else if name.starts_with("forearm") {
        4
    } else if name.starts_with("foretwist") {
        5
    } else if name.starts_with("hand") {
        6
    } else if name.starts_with("thigh") {
        7
    } else if name.starts_with("toe") {
        8
    } else if name.starts_with("upperarm") {
        9
    } else {
        20
    }
}

pub(in super::super) fn remap_imported_clip_paths_for_runtime(
    clips: &mut [fusionforge::modding::ImportedAnimationClip],
    raw_joints: &[CleanGltfJoint],
    runtime_joints: &[CleanGltfJoint],
) -> Result<(), String> {
    let raw_paths = clean_gltf_runtime_joint_paths_by_node(raw_joints);
    let runtime_paths = clean_gltf_runtime_joint_paths_by_node(runtime_joints);
    let path_map = raw_paths
        .iter()
        .filter_map(|(node_index, raw_path)| {
            runtime_paths
                .get(node_index)
                .map(|runtime_path| (raw_path.clone(), runtime_path.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    for clip in clips {
        clip.translations.retain_mut(|curve| {
            if let Some(runtime_path) = path_map.get(&curve.path) {
                curve.path = runtime_path.clone();
                true
            } else {
                curve.path == "Bip01"
            }
        });
        clip.rotations.retain_mut(|curve| {
            if let Some(runtime_path) = path_map.get(&curve.path) {
                curve.path = runtime_path.clone();
                true
            } else {
                curve.path == "Bip01"
            }
        });
        clip.scales.retain_mut(|curve| {
            if let Some(runtime_path) = path_map.get(&curve.path) {
                curve.path = runtime_path.clone();
                true
            } else {
                curve.path == "Bip01"
            }
        });
    }
    Ok(())
}

pub(in super::super) fn clean_gltf_animation_paths(
    clips: &[fusionforge::modding::ImportedAnimationClip],
) -> BTreeSet<String> {
    let mut paths = BTreeSet::<String>::new();
    for clip in clips {
        for curve in &clip.translations {
            if !curve.path.trim().is_empty() {
                paths.insert(curve.path.clone());
            }
        }
        for curve in &clip.rotations {
            if !curve.path.trim().is_empty() {
                paths.insert(curve.path.clone());
            }
        }
        for curve in &clip.scales {
            if !curve.path.trim().is_empty() {
                paths.insert(curve.path.clone());
            }
        }
    }
    paths
}

pub(in super::super) fn missing_clean_gltf_animation_paths(
    joints: &[CleanGltfJoint],
    clips: &[fusionforge::modding::ImportedAnimationClip],
) -> Vec<String> {
    let runtime_paths = clean_gltf_runtime_joint_paths(joints);
    clean_gltf_animation_paths(clips)
        .into_iter()
        .filter(|path| !runtime_paths.contains(path))
        .collect()
}

pub(in super::super) fn normalize_clean_gltf_npc_clip_names(clips: &mut [fusionforge::modding::ImportedAnimationClip]) {
    if clips.len() != 1 {
        return;
    }
    let clip = &mut clips[0];
    if clip.name != "nif-default" {
        clip.name = "stand1".to_string();
    }
}

pub(in super::super) fn expand_single_idle_clip_aliases(clips: &mut Vec<fusionforge::modding::ImportedAnimationClip>) {
    if clips.len() != 1 {
        return;
    }
    let Some(base_clip) = clips.first().cloned() else {
        return;
    };
    if base_clip.name != "stand1" {
        return;
    }
    for alias in ["stand2", "stand3"] {
        let mut clip = base_clip.clone();
        clip.name = alias.to_string();
        clips.push(clip);
    }
}

pub(in super::super) fn clean_animation(
    asset: &fusionforge::Asset,
    game_object_path_id: i64,
    clip_path_ids: &[i64],
) -> Result<fusionforge::UnityValue, String> {
    let mut value = asset.empty_object_value_for_class(111)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "m_GameObject".to_string(),
            unity_local_pointer(0, game_object_path_id),
        );
        object.insert("m_Enabled".to_string(), fusionforge::UnityValue::Bool(true));
        object.insert(
            "m_Animation".to_string(),
            clip_path_ids
                .first()
                .map(|path_id| unity_local_pointer(0, *path_id))
                .unwrap_or_else(unity_null_pointer),
        );
        object.insert(
            "m_Animations".to_string(),
            fusionforge::UnityValue::Array(
                clip_path_ids
                    .iter()
                    .map(|path_id| unity_local_pointer(0, *path_id))
                    .collect(),
            ),
        );
        object.insert(
            "m_AnimatePhysics".to_string(),
            fusionforge::UnityValue::Bool(false),
        );
        object.insert(
            "m_AnimateIfVisible".to_string(),
            fusionforge::UnityValue::Bool(true),
        );
        object.insert(
            "m_PlayAutomatically".to_string(),
            fusionforge::UnityValue::Bool(true),
        );
        object.insert("m_WrapMode".to_string(), fusionforge::UnityValue::Int(0));
    }
    Ok(value)
}

pub(in super::super) fn find_animation_component_key(
    env: &fusionforge::UnityEnvironment,
    selected: &BTreeSet<(usize, i64)>,
) -> Option<(usize, i64)> {
    selected
        .iter()
        .copied()
        .find(|(asset_index, path_id)| {
            env.assets
                .get(*asset_index)
                .and_then(|asset| {
                    asset
                        .objects
                        .get(path_id)
                        .map(|info| asset.object_type_name(info) == "Animation")
                })
                .unwrap_or(false)
        })
        .or_else(|| {
            env.assets
                .iter()
                .enumerate()
                .find_map(|(asset_index, asset)| {
                    asset.objects.values().find_map(|info| {
                        (asset.object_type_name(info) == "Animation")
                            .then_some((asset_index, info.path_id))
                    })
                })
        })
}

pub(in super::super) fn first_animation_clip_info(asset: &fusionforge::Asset) -> Option<&fusionforge::ObjectInfo> {
    asset
        .objects
        .values()
        .find(|info| asset.object_type_name(info) == "AnimationClip")
}

pub(in super::super) fn append_animation_clip_pointers(
    value: &mut fusionforge::UnityValue,
    asset_index: usize,
    clip_path_ids: &[i64],
) -> Result<(), String> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| "Animation component is not an object".to_string())?;
    let pointers = clip_path_ids
        .iter()
        .copied()
        .map(|path_id| unity_local_pointer(asset_index, path_id))
        .collect::<Vec<_>>();
    if let Some(first) = pointers.first().cloned() {
        object.insert("m_Animation".to_string(), first);
    }
    let animations = object
        .entry("m_Animations".to_string())
        .or_insert_with(|| fusionforge::UnityValue::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| "Animation.m_Animations is not an array".to_string())?;
    animations.retain(|value| {
        value
            .as_pointer()
            .is_none_or(|pointer| !clip_path_ids.contains(&pointer.path_id))
    });
    animations.extend(pointers);
    object.insert("m_Enabled".to_string(), fusionforge::UnityValue::Bool(true));
    object.insert(
        "m_PlayAutomatically".to_string(),
        fusionforge::UnityValue::Bool(true),
    );
    Ok(())
}

pub(in super::super) fn infer_authoring_npc_animation_set_from_template(
    project: &Path,
    source_table: &fusionforge::UnityValue,
    template_npc_id: usize,
    blueprint: &mut NpcBlueprint,
    warnings: &mut Vec<String>,
) {
    if blueprint
        .animation_set
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
    {
        return;
    }

    let template_model_paths = npc_model_paths_from_table(source_table, template_npc_id);
    if template_model_paths.is_empty() {
        warnings.push(format!(
            "Could not infer animationSet for authoring NPC '{}': template NPC {} has no model paths in TableData.",
            blueprint.name, template_npc_id
        ));
        return;
    }

    match find_npc_template_model_bundle(project, &template_model_paths) {
        Ok(bundle_path) => {
            blueprint.animation_set = Some(bundle_path.to_string_lossy().to_string());
            warnings.push(format!(
                "Inherited legacy animationSet {} from template NPC {} for authoring model {}.",
                bundle_path.display(),
                template_npc_id,
                blueprint.name
            ));
        }
        Err(err) => warnings.push(format!(
            "Could not infer animationSet for authoring NPC '{}': {}",
            blueprint.name, err
        )),
    }
}
