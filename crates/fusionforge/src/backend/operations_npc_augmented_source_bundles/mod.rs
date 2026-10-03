use super::super::*;

pub(in super::super) fn collapse_secondary_iktarget_roots_under_primary(joints: &mut [CleanGltfJoint]) {
    let root_indices = joints
        .iter()
        .enumerate()
        .filter_map(|(index, joint)| joint.parent_node_index.is_none().then_some(index))
        .collect::<Vec<_>>();
    if root_indices.len() <= 1 {
        return;
    }
    let Some(primary_index) = root_indices
        .iter()
        .copied()
        .find(|index| joints[*index].name == "Bip01 NonAccum")
        .or_else(|| root_indices.first().copied())
    else {
        return;
    };

    let mut globals = vec![[[0.0_f64; 4]; 4]; joints.len()];
    let mut pending = (0..joints.len()).collect::<BTreeSet<_>>();
    while let Some(index) = pending.iter().copied().find(|index| {
        joints[*index]
            .parent_node_index
            .and_then(|parent_node| {
                joints
                    .iter()
                    .position(|joint| joint.node_index == parent_node)
            })
            .is_none_or(|parent_index| !pending.contains(&parent_index))
    }) {
        let joint = &joints[index];
        let local = clean_trs_matrix4(joint.translation, joint.rotation, joint.scale);
        let global = if let Some(parent_node_index) = joint.parent_node_index {
            let parent_index = joints
                .iter()
                .position(|candidate| candidate.node_index == parent_node_index)
                .expect("missing parent joint");
            clean_multiply_matrix4(globals[parent_index], local)
        } else {
            local
        };
        globals[index] = global;
        pending.remove(&index);
    }

    let primary_global = globals[primary_index];
    let primary_node_index = joints[primary_index].node_index;
    for index in root_indices {
        if index == primary_index {
            continue;
        }
        if !joints[index].name.contains("IKTarget") {
            continue;
        }
        let relative =
            clean_multiply_matrix4(clean_invert_affine_matrix4(primary_global), globals[index]);
        let (translation, rotation, scale) = clean_decompose_affine_matrix4(relative);
        joints[index].parent_node_index = Some(primary_node_index);
        joints[index].translation = translation;
        joints[index].rotation = rotation;
        joints[index].scale = scale;
    }

    let mut children_by_parent = BTreeMap::<usize, Vec<usize>>::new();
    for joint in joints.iter() {
        if let Some(parent_node_index) = joint.parent_node_index {
            children_by_parent
                .entry(parent_node_index)
                .or_default()
                .push(joint.node_index);
        }
    }
    for joint in joints.iter_mut() {
        joint.children_node_indices = children_by_parent
            .get(&joint.node_index)
            .cloned()
            .unwrap_or_default();
    }
}

pub(in super::super) fn hoist_nonaccum_rest_transform_to_wrapper(
    joints: &mut [CleanGltfJoint],
) -> ((f64, f64, f64), (f64, f64, f64, f64), (f64, f64, f64)) {
    let Some(nonaccum_joint) = joints
        .iter_mut()
        .find(|joint| joint.parent_node_index.is_none() && joint.name == "Bip01 NonAccum")
    else {
        return ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 1.0), (1.0, 1.0, 1.0));
    };

    let wrapper_translation = nonaccum_joint.translation;
    let wrapper_rotation = nonaccum_joint.rotation;
    let wrapper_scale = nonaccum_joint.scale;
    nonaccum_joint.translation = (0.0, 0.0, 0.0);
    nonaccum_joint.rotation = (0.0, 0.0, 0.0, 1.0);
    nonaccum_joint.scale = (1.0, 1.0, 1.0);
    (wrapper_translation, wrapper_rotation, wrapper_scale)
}

/// Constant rest/animated rotation of the synthetic "Bip01" wrapper above the
/// GLB joint roots. Maps the converted GLB bind space onto the legacy NPC
/// prefab orientation (+Y up, game-facing). This quaternion matches the
/// "Bip01" root node rotation of the original world NPC prefabs (for example
/// npc_max in DongResources): a -90° X basis rotation composed with a +90°
/// yaw. A plain -90° X rotation left the NPC standing sideways in-game and
/// adding 180° yaw faced it backwards.
pub(in super::super) fn legacy_bip01_wrapper_rotation() -> (f64, f64, f64, f64) {
    (-0.5, 0.5, 0.5, 0.5)
}

pub(in super::super) fn legacy_curve_times(sample_rate: f64) -> [f64; 2] {
    let sample_rate = if sample_rate.is_finite() && sample_rate > 0.0 {
        sample_rate
    } else {
        30.0
    };
    [0.0, 1.0 / sample_rate]
}

pub(in super::super) fn ensure_legacy_bip01_root_curves(clips: &mut [fusionforge::modding::ImportedAnimationClip]) {
    // The wrapper's animated pose must match its rest pose (the constant
    // bind-space -> prefab-space rotation); writing identity here would lay
    // the whole skeleton flat again as soon as a clip plays.
    let wrapper_rotation = legacy_bip01_wrapper_rotation();
    for clip in clips {
        let [first_time, second_time] = legacy_curve_times(clip.sample_rate);
        if !clip.translations.iter().any(|curve| curve.path == "Bip01") {
            clip.translations
                .push(fusionforge::modding::ImportedVec3Curve {
                    path: "Bip01".to_string(),
                    keys: vec![
                        fusionforge::modding::ImportedVec3Key {
                            time: first_time,
                            value: (0.0, 0.0, 0.0),
                        },
                        fusionforge::modding::ImportedVec3Key {
                            time: second_time,
                            value: (0.0, 0.0, 0.0),
                        },
                    ],
                });
        }
        if !clip.rotations.iter().any(|curve| curve.path == "Bip01") {
            clip.rotations
                .push(fusionforge::modding::ImportedQuatCurve {
                    path: "Bip01".to_string(),
                    keys: vec![
                        fusionforge::modding::ImportedQuatKey {
                            time: first_time,
                            value: wrapper_rotation,
                        },
                        fusionforge::modding::ImportedQuatKey {
                            time: second_time,
                            value: wrapper_rotation,
                        },
                    ],
                });
        }
        if !clip.scales.iter().any(|curve| curve.path == "Bip01") {
            clip.scales.push(fusionforge::modding::ImportedVec3Curve {
                path: "Bip01".to_string(),
                keys: vec![
                    fusionforge::modding::ImportedVec3Key {
                        time: first_time,
                        value: (1.0, 1.0, 1.0),
                    },
                    fusionforge::modding::ImportedVec3Key {
                        time: second_time,
                        value: (1.0, 1.0, 1.0),
                    },
                ],
            });
        }
    }
}

pub(in super::super) fn rgb_to_565(r: u8, g: u8, b: u8) -> u16 {
    (((r as u16 >> 3) & 0x1f) << 11) | (((g as u16 >> 2) & 0x3f) << 5) | ((b as u16 >> 3) & 0x1f)
}

pub(in super::super) fn rgb_from_565(value: u16) -> [u8; 3] {
    let r = ((value >> 11) & 0x1f) as u8;
    let g = ((value >> 5) & 0x3f) as u8;
    let b = (value & 0x1f) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}

pub(in super::super) fn color_distance_sq(a: [u8; 3], b: [u8; 3]) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}

pub(in super::super) fn clean_transform(
    asset: &fusionforge::Asset,
    game_object_path_id: i64,
    father_path_id: Option<i64>,
    children_path_ids: &[i64],
    translation: (f64, f64, f64),
    rotation: (f64, f64, f64, f64),
    scale: (f64, f64, f64),
) -> Result<fusionforge::UnityValue, String> {
    let mut value = asset.empty_object_value_for_class(4)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "m_GameObject".to_string(),
            unity_local_pointer(0, game_object_path_id),
        );
        object.insert(
            "m_Father".to_string(),
            father_path_id
                .map(|path_id| unity_local_pointer(0, path_id))
                .unwrap_or_else(unity_null_pointer),
        );
        object.insert(
            "m_Children".to_string(),
            fusionforge::UnityValue::Array(
                children_path_ids
                    .iter()
                    .map(|path_id| unity_local_pointer(0, *path_id))
                    .collect(),
            ),
        );
        object.insert(
            "m_LocalPosition".to_string(),
            unity_vec3(translation.0, translation.1, translation.2),
        );
        object.insert(
            "m_LocalRotation".to_string(),
            unity_quat(rotation.0, rotation.1, rotation.2, rotation.3),
        );
        object.insert(
            "m_LocalScale".to_string(),
            unity_vec3(scale.0, scale.1, scale.2),
        );
    }
    Ok(value)
}

pub(in super::super) fn clean_skinned_renderer(
    asset: &fusionforge::Asset,
    game_object_path_id: i64,
    mesh_path_id: i64,
    material_path_id: i64,
    bone_transform_path_ids: &[i64],
    _bind_poses: &[fusionforge::modding::ImportedMatrix4x4],
) -> Result<fusionforge::UnityValue, String> {
    let mut value = asset.empty_object_value_for_class(137)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "m_GameObject".to_string(),
            unity_local_pointer(0, game_object_path_id),
        );
        object.insert("m_Enabled".to_string(), fusionforge::UnityValue::Bool(true));
        object.insert(
            "m_CastShadows".to_string(),
            fusionforge::UnityValue::Bool(true),
        );
        object.insert(
            "m_ReceiveShadows".to_string(),
            fusionforge::UnityValue::Bool(true),
        );
        object.insert(
            "m_LightmapIndex".to_string(),
            fusionforge::UnityValue::Int(255),
        );
        object.insert(
            "m_LightmapTilingOffset".to_string(),
            unity_quat(1.0, 1.0, 0.0, 0.0),
        );
        object.insert(
            "m_Materials".to_string(),
            fusionforge::UnityValue::Array(vec![unity_local_pointer(0, material_path_id)]),
        );
        object.insert("m_Mesh".to_string(), unity_local_pointer(0, mesh_path_id));
        object.insert(
            "m_Bones".to_string(),
            fusionforge::UnityValue::Array(
                bone_transform_path_ids
                    .iter()
                    .map(|path_id| unity_local_pointer(0, *path_id))
                    .collect(),
            ),
        );
        object.insert(
            "m_BindPose".to_string(),
            fusionforge::UnityValue::Array(Vec::new()),
        );
        object.insert("m_Quality".to_string(), fusionforge::UnityValue::Int(0));
        object.insert(
            "m_SkinNormals".to_string(),
            fusionforge::UnityValue::Bool(true),
        );
        object.insert(
            "m_UpdateWhenOffscreen".to_string(),
            fusionforge::UnityValue::Bool(false),
        );
    }
    Ok(value)
}

#[allow(dead_code)]
pub(in super::super) fn leading_external_preloads_for_assetbundle(
    asset: &fusionforge::Asset,
) -> Result<Vec<fusionforge::Pointer>, String> {
    let Some(info) = asset.objects.values().find(|info| info.class_id == 142) else {
        return Ok(Vec::new());
    };
    let value = asset.read_object(0, info)?;
    let mut result = Vec::new();
    for preload in fusionforge::value_array(value.get("m_PreloadTable")) {
        let Some(pointer) = preload.as_pointer() else {
            break;
        };
        if pointer.file_id == 0 {
            break;
        }
        result.push(pointer.clone());
    }
    Ok(result)
}

pub(in super::super) fn materialize_npc_authoring_source_assets(
    project: &Path,
    blueprint: &NpcBlueprint,
    authoring_model: &Path,
) -> Result<(PathBuf, String), String> {
    let source_name = authoring_model
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("npc_authoring");
    let target_root = npc_stage_root(project, blueprint).join("source");
    fs::create_dir_all(&target_root).map_err(|err| format!("{}: {err}", target_root.display()))?;
    let target_model = target_root.join(
        authoring_model
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("model.glb"),
    );
    copy_file_if_different(authoring_model, &target_model)?;
    if let Some(source_dir) = authoring_model.parent() {
        for extension in [
            "png", "jpg", "jpeg", "dds", "tga", "bmp", "tif", "tiff", "webp",
        ] {
            let candidate = source_dir.join(format!("{source_name}.{extension}"));
            if !candidate.is_file() {
                continue;
            }
            let target = target_root.join(
                candidate
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default(),
            );
            copy_file_if_different(&candidate, &target)?;
        }
    }
    let relative = path_relative_to_project(project, &target_model).ok_or_else(|| {
        format!(
            "{} is outside project {}",
            target_model.display(),
            project.display()
        )
    })?;
    Ok((target_model, relative))
}

pub(in super::super) fn imported_npc_character_name(object_name: &str) -> String {
    let base = object_name
        .strip_prefix("npc_")
        .or_else(|| object_name.strip_prefix("NPC_"))
        .unwrap_or(object_name);
    let parts = base
        .split(|ch: char| ch == '_' || ch == '-' || ch.is_ascii_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut output = String::new();
            output.extend(first.to_uppercase());
            output.push_str(&chars.as_str().to_ascii_lowercase());
            output
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        object_name.to_string()
    } else {
        parts.join(" ")
    }
}

pub(in super::super) fn standalone_npc_schema_candidates(
    project: &Path,
    bundle_name: &str,
    model_path: &str,
) -> Vec<(PathBuf, BTreeSet<String>)> {
    vec![
        (
            project.join("npcs").join("schemas").join(bundle_name),
            BTreeSet::from([model_path.to_string()]),
        ),
        (
            project
                .parent()
                .unwrap_or(project)
                .join("FusionFallClient-ru")
                .join(bundle_name),
            BTreeSet::from([model_path.to_string()]),
        ),
        (
            default_repo_root()
                .join("builds")
                .join("FusionFallClient-ru")
                .join(bundle_name),
            BTreeSet::from([model_path.to_string()]),
        ),
    ]
}

pub(in super::super) fn infer_authoring_npc_icon_from_template(
    project: &Path,
    source_table: &fusionforge::UnityValue,
    template_npc_id: usize,
    blueprint: &mut NpcBlueprint,
    warnings: &mut Vec<String>,
) {
    if blueprint.generated_icon.is_some()
        || blueprint
            .icon_asset
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        || blueprint
            .icon_bundle
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    {
        return;
    }

    let template_icon_paths = npc_icon_paths_from_table(source_table, template_npc_id);
    let Some(template_icon_path) = template_icon_paths.iter().next().cloned() else {
        warnings.push(format!(
            "Could not infer icon bundle for authoring NPC '{}': template NPC {} has no icon paths in TableData.",
            blueprint.name, template_npc_id
        ));
        return;
    };

    match find_npc_template_icon_bundle(project, &template_icon_paths) {
        Ok(bundle_path) => {
            blueprint.icon_asset = Some(template_icon_path.clone());
            blueprint.icon_bundle = Some(bundle_path.to_string_lossy().to_string());
            warnings.push(format!(
                "Inherited legacy icon {} from template NPC {} for authoring model {}.",
                template_icon_path, template_npc_id, blueprint.name
            ));
            warnings.push(format!(
                "Inherited legacy icon bundle {} from template NPC {} for authoring model {}.",
                bundle_path.display(),
                template_npc_id,
                blueprint.name
            ));
        }
        Err(err) => warnings.push(format!(
            "Could not infer icon bundle for authoring NPC '{}': {}",
            blueprint.name, err
        )),
    }
}

pub(in super::super) fn npc_augmented_source_bundles(
    project: &Path,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
    hints: &NpcAssetHints,
) -> Vec<PathBuf> {
    let uses_authoring = npc_blueprint_uses_authoring_model_bundle(blueprint);
    let mut bundles = Vec::new();
    let mut seen = BTreeSet::new();
    let stage_root = npc_stage_root(project, blueprint);
    let is_staged_self_path = |path: &Path| path.starts_with(&stage_root);
    for path in npc_blueprint_source_paths(project, blueprint) {
        if is_npc_build_asset_source_path(project, blueprint, &path) {
            add_unique_npc_source_bundle(&mut bundles, &mut seen, path);
            continue;
        }
        if is_staged_self_path(&path) {
            add_unique_npc_source_bundle(
                &mut bundles,
                &mut seen,
                recovered_or_original_npc_source_path(project, blueprint, context, path),
            );
            continue;
        }
        add_unique_npc_source_bundle(&mut bundles, &mut seen, path);
    }
    for path in &context.source_paths {
        if is_npc_build_asset_source_path(project, blueprint, path) {
            add_unique_npc_source_bundle(&mut bundles, &mut seen, path.clone());
            continue;
        }
        if is_staged_self_path(path) {
            add_unique_npc_source_bundle(
                &mut bundles,
                &mut seen,
                recovered_or_original_npc_source_path(project, blueprint, context, path.clone()),
            );
            continue;
        }
        add_unique_npc_source_bundle(&mut bundles, &mut seen, path.clone());
    }
    if context.source_paths.iter().any(|path| {
        !is_staged_self_path(path) || is_npc_build_asset_source_path(project, blueprint, path)
    }) {
        return bundles;
    }

    let strict_tokens = hints
        .tokens
        .iter()
        .filter(|token| token.contains('_') || token.starts_with("npc") || token.starts_with("mob"))
        .cloned()
        .collect::<BTreeSet<_>>();
    if hints.exact_paths.is_empty() && strict_tokens.is_empty() {
        return bundles;
    }

    if let Some(index) = load_npc_source_index(project, context) {
        let initial_bundle_keys = bundles
            .iter()
            .map(|path| path.to_string_lossy().to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        let mut covered_exact_paths = BTreeSet::<String>::new();
        for bundle in &index.bundles {
            let bundle_key = PathBuf::from(&bundle.path)
                .to_string_lossy()
                .to_ascii_lowercase();
            if !initial_bundle_keys.contains(&bundle_key) {
                continue;
            }
            for path in bundle
                .assets
                .iter()
                .flat_map(|asset| asset.container_paths.iter())
            {
                let lower = normalized_asset_path(path);
                if hints
                    .exact_paths
                    .iter()
                    .any(|exact| lower == *exact || lower.ends_with(&format!("/{exact}")))
                {
                    covered_exact_paths.insert(lower);
                }
            }
        }

        for bundle in &index.bundles {
            if uses_authoring && is_npc_shared_dependency_bundle(&bundle.name) {
                add_unique_npc_source_bundle(&mut bundles, &mut seen, PathBuf::from(&bundle.path));
            }
        }

        for bundle in index.bundles {
            let exact_matches = bundle
                .assets
                .iter()
                .flat_map(|asset| asset.container_paths.iter())
                .map(|path| normalized_asset_path(path))
                .filter(|path| {
                    hints
                        .exact_paths
                        .iter()
                        .any(|exact| path == exact || path.ends_with(&format!("/{exact}")))
                })
                .collect::<BTreeSet<_>>();
            let matched_by_tokens = hints.exact_paths.is_empty()
                && bundle.assets.iter().any(|asset| {
                    asset
                        .container_paths
                        .iter()
                        .any(|path| npc_hint_container_path_matches(path, &strict_tokens, hints))
                });
            if !exact_matches.is_empty() || matched_by_tokens {
                covered_exact_paths.extend(exact_matches);
                add_unique_npc_source_bundle(&mut bundles, &mut seen, PathBuf::from(bundle.path));
            }
        }
    }

    if uses_authoring {
        if let Some(source_dir) = context
            .source_table_data_bundle
            .as_ref()
            .and_then(|bundle| bundle.parent())
        {
            for shared_name in [
                "CharacterCreation.resourceFile",
                "CharacterSelection.resourceFile",
                "TrainingGrounds.resourceFile",
            ] {
                let shared_path = source_dir.join(shared_name);
                if shared_path.exists() {
                    add_unique_npc_source_bundle(&mut bundles, &mut seen, shared_path);
                }
            }
        }
    }
    bundles
}

pub(in super::super) fn npc_token_stopword(token: &str) -> bool {
    matches!(
        token,
        "copy"
            | "agent"
            | "npc"
            | "mob"
            | "the"
            | "and"
            | "null"
            | "texture"
            | "material"
            | "materials"
            | "maintex"
    )
}

pub(in super::super) fn npc_token_variants(value: &str) -> Vec<String> {
    let normalized = value.replace('\\', "/").to_ascii_lowercase();
    let without_extension = normalized
        .rsplit_once('.')
        .filter(|(_, extension)| extension.chars().all(|ch| ch.is_ascii_alphanumeric()))
        .map(|(base, _)| base.to_string())
        .unwrap_or(normalized);
    let basename = without_extension
        .rsplit('/')
        .next()
        .unwrap_or(&without_extension)
        .to_string();
    let mut base_parts = vec![
        without_extension,
        basename.clone(),
        basename
            .strip_prefix("npc_")
            .unwrap_or(&basename)
            .to_string(),
        basename
            .strip_prefix("mob_")
            .unwrap_or(&basename)
            .to_string(),
    ];
    let collapsed_parts = base_parts
        .iter()
        .map(|part| {
            part.chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        ch
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
                .trim_matches('_')
                .to_string()
        })
        .collect::<Vec<_>>();
    let compact_parts = base_parts
        .iter()
        .map(|part| {
            part.chars()
                .filter(|ch| ch.is_ascii_alphanumeric())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    let compact_basename = basename
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    if (3..4).contains(&compact_basename.len()) {
        base_parts.push(format!("npc_{compact_basename}"));
        base_parts.push(format!("mob_{compact_basename}"));
    }

    base_parts
        .into_iter()
        .chain(collapsed_parts)
        .chain(compact_parts)
        .flat_map(|part| {
            part.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .map(str::trim)
                .filter(|token| token.len() >= 3 || token.contains('_'))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|token| !npc_token_stopword(token))
        .collect()
}

pub(in super::super) fn add_npc_token_hint(hints: &mut NpcAssetHints, value: &str) {
    for token in npc_token_variants(value) {
        hints.tokens.insert(token);
    }
}

pub(in super::super) fn collapsed_ascii_token(value: &str, keep_underscore: bool) -> String {
    let mut token = String::new();
    let mut last_was_separator = false;
    for ch in value.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || (keep_underscore && ch == '_') {
            token.push(ch);
            last_was_separator = false;
        } else if keep_underscore && !last_was_separator {
            token.push('_');
            last_was_separator = true;
        }
    }
    token.trim_matches('_').to_string()
}

pub(in super::super) fn retarget_imported_npc_blueprint_paths_for_build(
    blueprint: &NpcBlueprint,
    hints: &NpcAssetHints,
) -> NpcBlueprint {
    let mut blueprint = blueprint.clone();
    if !npc_blueprint_uses_authoring_model_bundle(&blueprint) {
        if let Some(model_path) = blueprint.model_asset.as_deref() {
            blueprint.model_asset =
                Some(npc_imported_model_target_path(blueprint.npc_id, model_path));
        }
        if let Some(texture_path) = blueprint.texture_asset.as_deref() {
            blueprint.texture_asset = Some(npc_imported_texture_target_path(
                blueprint.npc_id,
                texture_path,
            ));
        }
    }
    if blueprint.generated_icon.is_none() {
        if let Some(icon_path) = retargeted_npc_icon_asset_path(&blueprint, hints) {
            blueprint.icon_asset = Some(icon_path);
        }
    }
    blueprint
}

pub(in super::super) fn metadata_preload_range(
    metadata: &fusionforge::UnityValue,
    preload_len: usize,
) -> (usize, usize) {
    let start = metadata
        .get("preloadIndex")
        .and_then(fusionforge::UnityValue::as_i64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
        .min(preload_len);
    let size = metadata
        .get("preloadSize")
        .and_then(fusionforge::UnityValue::as_i64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0);
    let end = start.saturating_add(size).min(preload_len);
    (start, end)
}
