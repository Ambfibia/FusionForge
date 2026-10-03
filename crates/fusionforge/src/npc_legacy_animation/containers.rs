use super::*;

pub(super) type ObjectKey = (usize, i64);

#[derive(Debug)]
pub(crate) struct UnityLegacyNpcPreview {
    pub model_hierarchy: JsonValue,
    pub skeleton: JsonValue,
    pub animations: Vec<JsonValue>,
    pub mesh_skins: BTreeMap<ObjectKey, JsonValue>,
    /// Exact renderer/Transform provenance for every selected mesh. Native
    /// publication uses this to keep logical model subtrees together instead
    /// of scattering every Mesh object into an unrelated standalone file.
    pub mesh_bindings: BTreeMap<ObjectKey, Vec<JsonValue>>,
    /// Unity `AssetBundle` preload ranges are sometimes shared by several
    /// character prefabs. Keep only meshes belonging to the character root
    /// selected by the requested KFM/NIF path so unrelated rigs cannot supply
    /// duplicate joint paths or rest transforms.
    pub selected_meshes: BTreeSet<ObjectKey>,
    pub warnings: Vec<String>,
}

/// Builds all optional animation additions for a bundle model preview.
///
/// `selected_meshes` must contain the actual Unity `Mesh` objects chosen by
/// the container graph.  Restricting renderer and clip discovery to those
/// assets avoids showing unrelated dependency-bundle animations as NPC clips.
pub(crate) fn build_unity_legacy_npc_preview(
    env: &fusionforge::UnityEnvironment,
    selected_meshes: &BTreeSet<ObjectKey>,
    preferred_paths: &BTreeSet<String>,
) -> UnityLegacyNpcPreview {
    build_unity_legacy_npc_preview_with_selection(env, selected_meshes, preferred_paths, None)
}

pub(crate) fn build_unity_legacy_npc_preview_from_selection(
    env: &fusionforge::UnityEnvironment,
    selected_meshes: &BTreeSet<ObjectKey>,
    preferred_paths: &BTreeSet<String>,
    selected_objects: &BTreeSet<ObjectKey>,
) -> UnityLegacyNpcPreview {
    build_unity_legacy_npc_preview_with_selection(
        env,
        selected_meshes,
        preferred_paths,
        Some(selected_objects),
    )
}

pub(super) fn build_unity_legacy_npc_preview_with_selection(
    env: &fusionforge::UnityEnvironment,
    selected_meshes: &BTreeSet<ObjectKey>,
    preferred_paths: &BTreeSet<String>,
    selected_objects: Option<&BTreeSet<ObjectKey>>,
) -> UnityLegacyNpcPreview {
    let transforms = collect_transforms(env, selected_objects);
    let candidate_mesh_count = selected_meshes.len();
    let preferred_roots = select_preferred_character_roots(
        env,
        selected_meshes,
        &transforms,
        preferred_paths,
        selected_objects,
    );
    let selected_meshes = meshes_for_character_roots(
        env,
        selected_meshes,
        &transforms,
        preferred_roots.as_ref(),
        selected_objects,
    );
    let renderers = collect_skinned_renderers(
        env,
        &selected_meshes,
        &transforms,
        preferred_roots.as_ref(),
        selected_objects,
    );
    if std::env::var_os("FFONE_PROFILE_NPC_LEGACY_SELECTION").is_some() {
        eprintln!(
            "legacy NPC selection profile: selectedObjects={} candidateMeshes={} transforms={} preferredRoots={} selectedMeshes={} renderers={} rendererTransforms={} rendererRoots={}",
            selected_objects.map_or(0, BTreeSet::len),
            candidate_mesh_count,
            transforms.len(),
            preferred_roots.as_ref().map_or(0, BTreeSet::len),
            selected_meshes.len(),
            renderers.len(),
            renderers.iter().filter(|renderer| renderer.transform.is_some()).count(),
            renderers
                .iter()
                .filter_map(|renderer| renderer.transform)
                .filter(|transform| character_root(*transform, &transforms).is_some())
                .count(),
        );
        for renderer in &renderers {
            let chain = renderer
                .transform
                .map(|start| {
                    let mut current = Some(start);
                    let mut visited = BTreeSet::new();
                    let mut values = Vec::new();
                    while let Some(key) = current {
                        if !visited.insert(key) {
                            values.push(format!("{}#{}(cycle)", key.0, key.1));
                            break;
                        }
                        let Some(node) = transforms.get(&key) else {
                            let selected =
                                selected_objects.is_none_or(|objects| objects.contains(&key));
                            let object_type = env
                                .assets
                                .get(key.0)
                                .and_then(|asset| {
                                    asset
                                        .objects
                                        .get(&key.1)
                                        .map(|info| asset.object_type_name(info))
                                })
                                .unwrap_or_else(|| "<absent>".to_string());
                            values.push(format!(
                                "{}#{}(missing-transform selected={selected} type={object_type})",
                                key.0, key.1
                            ));
                            break;
                        };
                        values.push(format!("{}#{}:{:?}", key.0, key.1, node.name));
                        current = node.parent;
                    }
                    values.join(" -> ")
                })
                .unwrap_or_else(|| "<none>".to_string());
            eprintln!(
                "legacy NPC renderer profile: renderer={}#{} mesh={}#{} chain={chain}",
                renderer.key.0, renderer.key.1, renderer.mesh.0, renderer.mesh.1,
            );
        }
    }
    let clip_keys = collect_clip_keys(
        env,
        &selected_meshes,
        &renderers,
        &transforms,
        selected_objects,
    );

    let mut raw_clips = Vec::<RawAnimationClip>::new();
    for (asset_index, path_id) in clip_keys {
        let Some(asset) = env.assets.get(asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(&path_id) else {
            continue;
        };
        let Ok(body) = asset.read_object(asset_index, info) else {
            continue;
        };
        raw_clips.push(RawAnimationClip {
            asset_name: asset.name.clone(),
            path_id,
            body,
        });
    }
    let time_recovery_catalog = build_curve_time_recovery_catalog(&raw_clips);
    let constant_curve_catalog = build_constant_curve_recovery_catalog(&raw_clips);

    let mut animations = Vec::new();
    let mut track_roots = BTreeSet::new();
    let mut warnings = Vec::new();
    let filtered_mesh_count = candidate_mesh_count.saturating_sub(selected_meshes.len());
    if filtered_mesh_count > 0 {
        warnings.push(format!(
            "Ignored {filtered_mesh_count} mesh(es) from unrelated character roots in the shared preload range."
        ));
    }
    for raw_clip in raw_clips {
        let (recovered_body, time_recoveries) =
            recover_non_strict_curve_times(&raw_clip, &time_recovery_catalog);
        let curve_recovery_plans =
            recover_conflicting_constant_curves(&raw_clip, &constant_curve_catalog);
        let decoded = decode_animation_clip_with_curve_recoveries(
            &raw_clip.asset_name,
            raw_clip.path_id,
            &recovered_body,
            time_recoveries,
            &curve_recovery_plans,
        );
        track_roots.extend(decoded.track_roots);
        warnings.extend(decoded.warnings);
        animations.push(decoded.preview);
    }
    animations.sort_by(|left, right| {
        left.get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .cmp(
                right
                    .get("name")
                    .and_then(JsonValue::as_str)
                    .unwrap_or_default(),
            )
    });
    let (skeleton, joint_paths) = build_skeleton_preview(&transforms, &renderers, &track_roots);
    let mut mesh_skins = BTreeMap::new();
    for renderer in &renderers {
        if mesh_skins.contains_key(&renderer.mesh) {
            continue;
        }
        let Some(asset) = env.assets.get(renderer.mesh.0) else {
            continue;
        };
        let Some(info) = asset.objects.get(&renderer.mesh.1) else {
            continue;
        };
        let Ok(mesh) = asset.read_object(renderer.mesh.0, info) else {
            continue;
        };
        match decode_mesh_skin(&mesh, renderer, &joint_paths) {
            Ok(skin) => {
                mesh_skins.insert(renderer.mesh, skin);
            }
            Err(error) => warnings.push(format!(
                "{}#{}: legacy NPC skin preview is unavailable: {error}",
                asset.name, renderer.mesh.1
            )),
        }
    }
    for (mesh, skin) in collect_rigid_mesh_skins(
        env,
        &selected_meshes,
        &transforms,
        &joint_paths,
        &mesh_skins,
        preferred_roots.as_ref(),
        selected_objects,
    ) {
        mesh_skins.entry(mesh).or_insert(skin);
    }
    let mesh_bindings = collect_mesh_bindings(
        env,
        &selected_meshes,
        &transforms,
        &renderers,
        selected_objects,
    );
    let model_hierarchy = build_model_hierarchy(
        env,
        &selected_meshes,
        &transforms,
        &renderers,
        selected_objects,
    );

    UnityLegacyNpcPreview {
        model_hierarchy,
        skeleton,
        animations,
        mesh_skins,
        mesh_bindings,
        selected_meshes,
        warnings,
    }
}

pub(super) fn parse_unity_matrix(value: &fusionforge::UnityValue) -> Option<Matrix4> {
    let object = value.as_object()?;
    let mut matrix = [[0.0_f64; 4]; 4];
    for row in 0..4 {
        for column in 0..4 {
            matrix[row][column] = object
                .get(&format!("e{row}{column}"))
                .and_then(fusionforge::UnityValue::as_f64)?;
        }
    }
    Some(matrix)
}

pub(super) fn read_object(
    env: &fusionforge::UnityEnvironment,
    key: ObjectKey,
) -> Option<fusionforge::UnityValue> {
    let asset = env.assets.get(key.0)?;
    let info = asset.objects.get(&key.1)?;
    asset.read_object(key.0, info).ok()
}

pub(super) fn object_type(env: &fusionforge::UnityEnvironment, key: ObjectKey) -> Option<String> {
    let asset = env.assets.get(key.0)?;
    let info = asset.objects.get(&key.1)?;
    Some(asset.object_type_name(info))
}

pub(super) fn unity_vec3(value: Option<&fusionforge::UnityValue>, fallback: Vec3) -> Vec3 {
    let Some(value) = value else {
        return fallback;
    };
    let Some(x) = value.get("x").and_then(fusionforge::UnityValue::as_f64) else {
        return fallback;
    };
    let Some(y) = value.get("y").and_then(fusionforge::UnityValue::as_f64) else {
        return fallback;
    };
    let Some(z) = value.get("z").and_then(fusionforge::UnityValue::as_f64) else {
        return fallback;
    };
    [x, y, z]
}

pub(super) fn unity_quat(value: Option<&fusionforge::UnityValue>, fallback: Quat) -> Quat {
    let Some(value) = value else {
        return fallback;
    };
    let Some(x) = value.get("x").and_then(fusionforge::UnityValue::as_f64) else {
        return fallback;
    };
    let Some(y) = value.get("y").and_then(fusionforge::UnityValue::as_f64) else {
        return fallback;
    };
    let Some(z) = value.get("z").and_then(fusionforge::UnityValue::as_f64) else {
        return fallback;
    };
    let Some(w) = value.get("w").and_then(fusionforge::UnityValue::as_f64) else {
        return fallback;
    };
    [x, y, z, w]
}
