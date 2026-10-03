use super::*;

pub(super) fn collect_clip_keys(
    env: &fusionforge::UnityEnvironment,
    selected_meshes: &BTreeSet<ObjectKey>,
    renderers: &[SkinnedRenderer],
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    selected_objects: Option<&BTreeSet<ObjectKey>>,
) -> BTreeSet<ObjectKey> {
    // A KFM route may animate either a skinned rig or a rigid GameObject
    // hierarchy. Anchor Animation components to roots which own an exact
    // selected MeshFilter/SkinnedMeshRenderer; renderer presence alone cannot
    // distinguish a valid rigid clip from an unrelated dependency clip.
    let mut selected_roots =
        collect_character_root_candidates(env, selected_meshes, transforms, selected_objects)
            .into_keys()
            .collect::<BTreeSet<_>>();
    for renderer in renderers {
        for seed in renderer
            .transform
            .into_iter()
            .chain(renderer.root_bone)
            .chain(renderer.bones.iter().copied())
        {
            if let Some(root) = character_root(seed, transforms) {
                selected_roots.insert(root);
            }
        }
    }
    if selected_roots.is_empty() {
        return BTreeSet::new();
    }
    let selected_game_objects = transforms
        .values()
        .filter(|node| {
            character_root(node.key, transforms).is_some_and(|root| selected_roots.contains(&root))
        })
        .filter_map(|node| node.game_object)
        .collect::<BTreeSet<_>>();
    let asset_indices = selected_meshes
        .iter()
        .map(|key| key.0)
        .chain(
            renderers
                .iter()
                .flat_map(|renderer| [renderer.key.0, renderer.mesh.0]),
        )
        .chain(selected_game_objects.iter().map(|key| key.0))
        .collect::<BTreeSet<_>>();
    let mut result = BTreeSet::new();
    let mut visit =
        |asset_index: usize, asset: &fusionforge::Asset, info: &fusionforge::ObjectInfo| {
            if asset.object_type_name(info) != "Animation" {
                return;
            }
            let Ok(body) = asset.read_object(asset_index, info) else {
                return;
            };
            let Some(game_object) = resolved_key(env, body.get("m_GameObject")) else {
                return;
            };
            // `selected_objects` is the exact preload scan boundary, not a
            // claim that every preloaded GameObject belongs to the selected
            // character. KFM closures legitimately contain neighboring scene
            // roots and their Animation components (Fusion Gunter includes a
            // building `Default Take`). Only components on the proven model
            // hierarchy may contribute clips.
            if !selected_game_objects.contains(&game_object) {
                return;
            }
            for value in fusionforge::value_array(body.get("m_Animations")) {
                if let Some(key) = resolved_key(env, Some(value)) {
                    if object_type(env, key).as_deref() == Some("AnimationClip") {
                        result.insert(key);
                    }
                }
            }
            if let Some(key) = resolved_key(env, body.get("m_Animation")) {
                if object_type(env, key).as_deref() == Some("AnimationClip") {
                    result.insert(key);
                }
            }
        };
    if let Some(selected_objects) = selected_objects {
        for &(asset_index, path_id) in selected_objects {
            let Some(asset) = env.assets.get(asset_index) else {
                continue;
            };
            let Some(info) = asset.objects.get(&path_id) else {
                continue;
            };
            visit(asset_index, asset, info);
        }
    } else {
        for asset_index in &asset_indices {
            let Some(asset) = env.assets.get(*asset_index) else {
                continue;
            };
            for info in asset.objects.values() {
                visit(*asset_index, asset, info);
            }
        }
    }
    result
}

pub(super) fn clip_sample_rate(body: &fusionforge::UnityValue) -> Option<f64> {
    body.get("m_SampleRate")
        .and_then(fusionforge::UnityValue::as_f64)
        .or_else(|| {
            body.get("m_FrameRate")
                .and_then(fusionforge::UnityValue::as_f64)
        })
        .filter(|value| value.is_finite() && *value > 0.0)
}

pub(super) fn build_skeleton_preview(
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    renderers: &[SkinnedRenderer],
    track_roots: &BTreeSet<String>,
) -> (JsonValue, BTreeMap<ObjectKey, String>) {
    let mut roots = BTreeSet::<ObjectKey>::new();
    for renderer in renderers {
        for key in renderer
            .root_bone
            .into_iter()
            .chain(renderer.bones.iter().copied())
        {
            let mut current = Some(key);
            while let Some(candidate) = current {
                let Some(node) = transforms.get(&candidate) else {
                    break;
                };
                if track_roots
                    .iter()
                    .any(|root| node.name.eq_ignore_ascii_case(root))
                {
                    roots.insert(candidate);
                    break;
                }
                current = node.parent;
            }
        }
    }
    if roots.is_empty() {
        for renderer in renderers {
            if let Some(root) = renderer.root_bone {
                roots.insert(root);
            } else if let Some(root) = common_ancestor(transforms, &renderer.bones) {
                roots.insert(root);
            }
        }
    }

    let included = transforms
        .keys()
        .copied()
        .filter(|key| ancestor_in_set(transforms, *key, &roots))
        .collect::<BTreeSet<_>>();
    let mut ordered = included.iter().copied().collect::<Vec<_>>();
    ordered.sort_by_key(|key| (depth_within(transforms, *key, &roots), *key));

    let mut paths = BTreeMap::<ObjectKey, String>::new();
    for key in &ordered {
        let Some(node) = transforms.get(key) else {
            continue;
        };
        let path = node
            .parent
            .and_then(|parent| paths.get(&parent))
            .map(|parent| format!("{parent}/{}", node.name))
            .unwrap_or_else(|| node.name.clone());
        paths.insert(*key, normalized_path(&path));
    }

    let joints = ordered
        .iter()
        .filter_map(|key| {
            let node = transforms.get(key)?;
            let path = paths.get(key)?;
            let parent = node.parent.and_then(|parent| paths.get(&parent)).cloned();
            let translation = [
                -node.translation[0],
                node.translation[1],
                node.translation[2],
            ];
            let rotation = normalize_quat([
                node.rotation[0],
                -node.rotation[1],
                -node.rotation[2],
                node.rotation[3],
            ]);
            Some(json!({
                "path": path,
                "parent": parent,
                "translation": translation,
                "rotation": rotation,
                "scale": node.scale,
                "sourceAssetIndex": key.0,
                "transformPathId": key.1,
            }))
        })
        .collect::<Vec<_>>();

    (
        json!({
            "source": "unity-transform-hierarchy",
            "space": "npc-root",
            "joints": joints,
        }),
        paths,
    )
}

pub(super) fn declared_clip_duration(body: &fusionforge::UnityValue) -> Option<f64> {
    body.get("m_StopTime")
        .and_then(fusionforge::UnityValue::as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .or_else(|| {
            let clip = body.get("m_MuscleClip")?;
            let stop = clip.get("m_StopTime")?.as_f64()?;
            let start = clip
                .get("m_StartTime")
                .and_then(fusionforge::UnityValue::as_f64)
                .unwrap_or(0.0);
            (stop.is_finite() && start.is_finite()).then_some((stop - start).max(0.0))
        })
}
