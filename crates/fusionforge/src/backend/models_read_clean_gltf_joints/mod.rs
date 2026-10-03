use super::super::*;

pub(in super::super) fn preview_renderable_mesh_keys(env: &fusionforge::UnityEnvironment) -> BTreeSet<(usize, i64)> {
    let mut mesh_filters = BTreeMap::<(usize, i64), (usize, i64)>::new();
    let mut mesh_renderer_game_objects = BTreeSet::<(usize, i64)>::new();
    let mut renderable = BTreeSet::<(usize, i64)>::new();

    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            let object_type = asset.object_type_name(info);
            if !matches!(
                object_type.as_str(),
                "MeshFilter" | "MeshRenderer" | "SkinnedMeshRenderer"
            ) {
                continue;
            }
            let Ok(body) = asset.read_object(asset_index, info) else {
                continue;
            };
            let enabled = body
                .get("m_Enabled")
                .and_then(fusionforge::UnityValue::as_i64)
                .map(|value| value != 0)
                .unwrap_or(true);
            if !enabled {
                continue;
            }
            let game_object = body
                .get("m_GameObject")
                .and_then(fusionforge::UnityValue::as_pointer)
                .and_then(|pointer| env.resolve_pointer(pointer).ok())
                .map(|key| (key.asset, key.path_id));

            match object_type.as_str() {
                "MeshFilter" => {
                    if let (Some(game_object), Some(mesh)) = (
                        game_object,
                        body.get("m_Mesh")
                            .and_then(fusionforge::UnityValue::as_pointer)
                            .and_then(|pointer| env.resolve_pointer(pointer).ok())
                            .map(|key| (key.asset, key.path_id)),
                    ) {
                        mesh_filters.insert(game_object, mesh);
                    }
                }
                "MeshRenderer" => {
                    if let Some(game_object) = game_object {
                        mesh_renderer_game_objects.insert(game_object);
                    }
                }
                "SkinnedMeshRenderer" => {
                    if let Some(mesh) = body
                        .get("m_Mesh")
                        .and_then(fusionforge::UnityValue::as_pointer)
                        .and_then(|pointer| env.resolve_pointer(pointer).ok())
                        .map(|key| (key.asset, key.path_id))
                    {
                        renderable.insert(mesh);
                    }
                }
                _ => {}
            }
        }
    }

    for game_object in mesh_renderer_game_objects {
        if let Some(mesh) = mesh_filters.get(&game_object) {
            renderable.insert(*mesh);
        }
    }
    renderable
}

pub(in super::super) fn preview_renderable_mesh_keys_from_selection(
    env: &fusionforge::UnityEnvironment,
    selected: &BTreeSet<(usize, i64)>,
) -> BTreeSet<(usize, i64)> {
    let mut mesh_filters = BTreeMap::<(usize, i64), (usize, i64)>::new();
    let mut mesh_renderer_game_objects = BTreeSet::<(usize, i64)>::new();
    let mut renderable = BTreeSet::<(usize, i64)>::new();
    for &(asset_index, path_id) in selected {
        let Some(asset) = env.assets.get(asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(&path_id) else {
            continue;
        };
        let object_type = asset.object_type_name(info);
        if !matches!(
            object_type.as_str(),
            "MeshFilter" | "MeshRenderer" | "SkinnedMeshRenderer"
        ) {
            continue;
        }
        let Ok(body) = asset.read_object(asset_index, info) else {
            continue;
        };
        let enabled = body
            .get("m_Enabled")
            .and_then(fusionforge::UnityValue::as_i64)
            .map(|value| value != 0)
            .unwrap_or(true);
        if !enabled {
            continue;
        }
        let game_object = body
            .get("m_GameObject")
            .and_then(fusionforge::UnityValue::as_pointer)
            .and_then(|pointer| env.resolve_pointer(pointer).ok())
            .map(|key| (key.asset, key.path_id));
        match object_type.as_str() {
            "MeshFilter" => {
                if let (Some(game_object), Some(mesh)) = (
                    game_object,
                    body.get("m_Mesh")
                        .and_then(fusionforge::UnityValue::as_pointer)
                        .and_then(|pointer| env.resolve_pointer(pointer).ok())
                        .map(|key| (key.asset, key.path_id)),
                ) {
                    mesh_filters.insert(game_object, mesh);
                }
            }
            "MeshRenderer" => {
                if let Some(game_object) = game_object {
                    mesh_renderer_game_objects.insert(game_object);
                }
            }
            "SkinnedMeshRenderer" => {
                if let Some(mesh) = body
                    .get("m_Mesh")
                    .and_then(fusionforge::UnityValue::as_pointer)
                    .and_then(|pointer| env.resolve_pointer(pointer).ok())
                    .map(|key| (key.asset, key.path_id))
                {
                    renderable.insert(mesh);
                }
            }
            _ => {}
        }
    }
    for game_object in mesh_renderer_game_objects {
        if let Some(mesh) = mesh_filters.get(&game_object) {
            renderable.insert(*mesh);
        }
    }
    renderable
}

pub(in super::super) fn is_authoring_model_path(value: &str) -> bool {
    matches!(
        Path::new(value)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "glb" | "gltf" | "obj"
    )
}

pub(in super::super) fn npc_blueprint_uses_authoring_model_bundle(blueprint: &NpcBlueprint) -> bool {
    blueprint
        .authoring_model_path
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        || blueprint
            .model_bundle
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| {
                value
                    .replace('\\', "/")
                    .to_ascii_lowercase()
                    .ends_with("__authoring.resourcefile")
            })
}

pub(in super::super) fn npc_model_paths_from_table(
    npc_table: &fusionforge::UnityValue,
    npc_id: usize,
) -> BTreeSet<String> {
    let mut hints = NpcAssetHints::default();
    collect_npc_asset_hints_from_table(npc_table, npc_id, &mut hints);
    hints.model_paths
}

pub(in super::super) fn find_npc_template_model_bundle(
    project: &Path,
    template_model_paths: &BTreeSet<String>,
) -> Result<PathBuf, String> {
    let index_path = project.join("cache").join("bundle-index.json");
    if !index_path.is_file() {
        return Err(format!(
            "{} is missing; load/index the target client before converting an authoring model",
            index_path.display()
        ));
    }
    let index = serde_json::from_str::<ClientFileIndex>(
        &fs::read_to_string(&index_path)
            .map_err(|err| format!("{}: {err}", index_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", index_path.display()))?;
    let mut best = None::<(i32, String)>;
    for bundle in &index.bundles {
        let paths = bundle
            .assets
            .iter()
            .flat_map(|asset| asset.container_paths.iter())
            .map(|path| normalized_asset_path(path))
            .collect::<BTreeSet<_>>();
        let exact_matches = paths
            .iter()
            .filter(|path| {
                template_model_paths
                    .iter()
                    .any(|expected| path == &expected || path.ends_with(&format!("/{expected}")))
            })
            .count() as i32;
        if exact_matches == 0 {
            continue;
        }
        let has_mesh = bundle
            .assets
            .iter()
            .any(|asset| asset.type_counts.get("Mesh").copied().unwrap_or_default() > 0);
        let has_game_object = bundle.assets.iter().any(|asset| {
            asset
                .type_counts
                .get("GameObject")
                .copied()
                .unwrap_or_default()
                > 0
        });
        let score = exact_matches * 1000
            + if has_mesh { 100 } else { 0 }
            + if has_game_object { 20 } else { 0 };
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, bundle.path.clone()));
        }
    }
    best.map(|(_, path)| PathBuf::from(path)).ok_or_else(|| {
        format!(
            "Could not find a template NPC resource bundle for model path(s): {}",
            template_model_paths
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

pub(in super::super) fn model_container_path_matches(path: &str, wanted: &BTreeSet<String>) -> bool {
    let lower = normalized_asset_path(path);
    if wanted.is_empty() {
        return lower.starts_with("mob/");
    }
    wanted
        .iter()
        .any(|expected| lower == *expected || lower.ends_with(&format!("/{expected}")))
}

pub(in super::super) fn target_model_path_for_template_path(template_path: &str, target_model_path: &str) -> String {
    let template_ext = Path::new(template_path.replace('\\', "/").as_str())
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let target_ext = Path::new(target_model_path.replace('\\', "/").as_str())
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let Some(template_ext) = template_ext else {
        return target_model_path.replace('\\', "/");
    };
    if target_ext.as_deref() == Some(template_ext.as_str()) {
        return target_model_path.replace('\\', "/");
    }
    let normalized = target_model_path.replace('\\', "/");
    if let Some((base, _)) = normalized.rsplit_once('.') {
        format!("{base}.{template_ext}")
    } else {
        format!("{normalized}.{template_ext}")
    }
}

pub(in super::super) fn set_gameobject_static_mesh_components(
    value: &mut fusionforge::UnityValue,
    asset_index: usize,
    transform_path_id: i64,
    mesh_filter_path_id: i64,
    mesh_renderer_path_id: i64,
) {
    if let Some(object) = value.as_object_mut() {
        object.insert("m_IsActive".to_string(), fusionforge::UnityValue::Int(1));
        object.insert(
            "m_Component".to_string(),
            fusionforge::UnityValue::Array(vec![
                local_component_pair(4, asset_index, transform_path_id),
                local_component_pair(33, asset_index, mesh_filter_path_id),
                local_component_pair(23, asset_index, mesh_renderer_path_id),
            ]),
        );
    }
}

pub(in super::super) fn renderer_mesh_key(
    env: &fusionforge::UnityEnvironment,
    value: &fusionforge::UnityValue,
) -> Option<(usize, i64)> {
    let pointer = value.get("m_Mesh")?.as_pointer()?;
    let key = env.resolve_pointer(pointer).ok()?;
    Some((key.asset, key.path_id))
}

pub(in super::super) fn retarget_mesh_filter(
    value: &mut fusionforge::UnityValue,
    asset_index: usize,
    gameobject_path_id: i64,
    mesh_path_id: i64,
) {
    set_local_pointer_field(value, "m_GameObject", asset_index, gameobject_path_id);
    set_local_pointer_field(value, "m_Mesh", asset_index, mesh_path_id);
}

pub(in super::super) fn retarget_mesh_renderer(
    value: &mut fusionforge::UnityValue,
    asset_index: usize,
    gameobject_path_id: i64,
    material_path_id: i64,
) {
    if let Some(object) = value.as_object_mut() {
        object.insert("m_Enabled".to_string(), fusionforge::UnityValue::Bool(true));
        object.insert(
            "m_GameObject".to_string(),
            unity_local_pointer(asset_index, gameobject_path_id),
        );
        object.insert(
            "m_Materials".to_string(),
            fusionforge::UnityValue::Array(vec![unity_local_pointer(
                asset_index,
                material_path_id,
            )]),
        );
        object.insert(
            "m_UpdateWhenOffscreen".to_string(),
            fusionforge::UnityValue::Bool(true),
        );
    }
}

pub(in super::super) fn retarget_skinned_mesh_renderer(
    value: &mut fusionforge::UnityValue,
    asset_index: usize,
    mesh_path_id: i64,
    material_path_id: i64,
) {
    if let Some(object) = value.as_object_mut() {
        object.insert("m_Enabled".to_string(), fusionforge::UnityValue::Bool(true));
        object.insert(
            "m_Mesh".to_string(),
            unity_local_pointer(asset_index, mesh_path_id),
        );
        object.insert(
            "m_Materials".to_string(),
            fusionforge::UnityValue::Array(vec![unity_local_pointer(
                asset_index,
                material_path_id,
            )]),
        );
        object.insert(
            "m_UpdateWhenOffscreen".to_string(),
            fusionforge::UnityValue::Bool(true),
        );
    }
}

pub(in super::super) fn mesh_bind_poses_from_value(
    value: &fusionforge::UnityValue,
) -> Vec<fusionforge::modding::ImportedMatrix4x4> {
    let Some(bind_poses) = value
        .get("m_BindPose")
        .and_then(fusionforge::UnityValue::as_array)
    else {
        return Vec::new();
    };

    bind_poses
        .iter()
        .filter_map(|entry| {
            let matrix = entry.as_object()?;
            let mut values = [0.0; 16];
            for (index, key) in [
                "e00", "e01", "e02", "e03", "e10", "e11", "e12", "e13", "e20", "e21", "e22", "e23",
                "e30", "e31", "e32", "e33",
            ]
            .iter()
            .enumerate()
            {
                values[index] = matrix.get(*key).and_then(fusionforge::UnityValue::as_f64)?;
            }
            Some(fusionforge::modding::ImportedMatrix4x4 { values })
        })
        .collect()
}

pub(in super::super) fn reset_imported_mesh_transform(value: &mut fusionforge::UnityValue) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.insert(
        "m_LocalPosition".to_string(),
        fusionforge::UnityValue::Object(BTreeMap::from([
            ("x".to_string(), fusionforge::UnityValue::Float(0.0)),
            ("y".to_string(), fusionforge::UnityValue::Float(0.0)),
            ("z".to_string(), fusionforge::UnityValue::Float(0.0)),
        ])),
    );
    object.insert(
        "m_LocalRotation".to_string(),
        fusionforge::UnityValue::Object(BTreeMap::from([
            ("x".to_string(), fusionforge::UnityValue::Float(0.0)),
            ("y".to_string(), fusionforge::UnityValue::Float(0.0)),
            ("z".to_string(), fusionforge::UnityValue::Float(1.0)),
            ("w".to_string(), fusionforge::UnityValue::Float(0.0)),
        ])),
    );
    object.insert(
        "m_LocalScale".to_string(),
        fusionforge::UnityValue::Object(BTreeMap::from([
            ("x".to_string(), fusionforge::UnityValue::Float(1.0)),
            ("y".to_string(), fusionforge::UnityValue::Float(1.0)),
            ("z".to_string(), fusionforge::UnityValue::Float(1.0)),
        ])),
    );
}

pub(in super::super) fn npc_converted_authoring_model_path(project: &Path, blueprint: &NpcBlueprint) -> PathBuf {
    let bundle_name = npc_blueprint_bundle_name(blueprint)
        .trim_end_matches(".resourceFile")
        .trim_end_matches(".resourcefile")
        .to_string();
    npc_build_work_root(project, blueprint)
        .join("generated")
        .join(format!(
            "{}__authoring.resourceFile",
            safe_segment(&bundle_name)
        ))
}

#[derive(Debug, Clone)]
pub(in super::super) struct CleanGltfJoint {
    pub(in super::super) node_index: usize,
    pub(in super::super) name: String,
    pub(in super::super) parent_node_index: Option<usize>,
    pub(in super::super) children_node_indices: Vec<usize>,
    pub(in super::super) translation: (f64, f64, f64),
    pub(in super::super) rotation: (f64, f64, f64, f64),
    pub(in super::super) scale: (f64, f64, f64),
}

pub(in super::super) fn gltf_clean_scale(value: [f32; 3]) -> (f64, f64, f64) {
    (
        f64::from(value[0]),
        f64::from(value[2]),
        f64::from(value[1]),
    )
}

pub(in super::super) fn gltf_clean_rotation(value: [f32; 4]) -> (f64, f64, f64, f64) {
    (
        -f64::from(value[0]),
        f64::from(value[2]),
        f64::from(value[1]),
        f64::from(value[3]),
    )
}

pub(in super::super) fn read_clean_gltf_joints(path: &Path) -> Result<Vec<CleanGltfJoint>, String> {
    let (document, _, _) =
        gltf::import(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let skin = document
        .skins()
        .next()
        .ok_or_else(|| format!("{} has no GLB skin", path.display()))?;
    let joint_indices = skin
        .joints()
        .map(|node| node.index())
        .collect::<BTreeSet<_>>();
    let mut parent_by_child = BTreeMap::<usize, usize>::new();
    for node in document.nodes() {
        for child in node.children() {
            parent_by_child.insert(child.index(), node.index());
        }
    }
    let nodes_by_index = document
        .nodes()
        .map(|node| (node.index(), node))
        .collect::<BTreeMap<_, _>>();
    let mut collapsed_parent_by_joint = BTreeMap::<usize, Option<usize>>::new();
    let mut collapsed_local_by_joint =
        BTreeMap::<usize, ((f64, f64, f64), (f64, f64, f64, f64), (f64, f64, f64))>::new();
    for node in skin.joints() {
        let mut chain = vec![node.index()];
        let mut current = node.index();
        let parent_joint_index = loop {
            let Some(parent_index) = parent_by_child.get(&current).copied() else {
                break None;
            };
            if joint_indices.contains(&parent_index) {
                break Some(parent_index);
            }
            chain.push(parent_index);
            current = parent_index;
        };
        chain.reverse();
        let mut translation = (0.0, 0.0, 0.0);
        let mut rotation = (0.0, 0.0, 0.0, 1.0);
        let mut scale = (1.0, 1.0, 1.0);
        for node_index in chain {
            let (local_t, local_r, local_s) = nodes_by_index[&node_index].transform().decomposed();
            let local_translation = gltf_clean_translation(local_t);
            let local_rotation = gltf_clean_rotation(local_r);
            let local_scale = gltf_clean_scale(local_s);
            translation = clean_add_vec3(
                translation,
                clean_rotate_vec3(rotation, clean_mul_vec3(scale, local_translation)),
            );
            rotation = clean_quat_mul(rotation, local_rotation);
            scale = clean_mul_vec3(scale, local_scale);
        }
        collapsed_parent_by_joint.insert(node.index(), parent_joint_index);
        collapsed_local_by_joint.insert(node.index(), (translation, rotation, scale));
    }
    let mut children_by_parent = BTreeMap::<usize, Vec<usize>>::new();
    for (child, parent) in &collapsed_parent_by_joint {
        if let Some(parent) = parent {
            children_by_parent.entry(*parent).or_default().push(*child);
        }
    }
    let mut result = Vec::new();
    for node in skin.joints() {
        let (translation, rotation, scale) = collapsed_local_by_joint[&node.index()];
        let children_node_indices = children_by_parent
            .get(&node.index())
            .cloned()
            .unwrap_or_default();
        let parent_node_index = collapsed_parent_by_joint[&node.index()];
        result.push(CleanGltfJoint {
            node_index: node.index(),
            name: node
                .name()
                .map(str::to_string)
                .unwrap_or_else(|| format!("node_{}", node.index())),
            parent_node_index,
            children_node_indices,
            translation,
            rotation,
            scale,
        });
    }
    Ok(result)
}

pub(in super::super) fn legacy_skinned_mesh_basis_rotation() -> (f64, f64, f64, f64) {
    (
        -std::f64::consts::FRAC_1_SQRT_2,
        0.0,
        0.0,
        std::f64::consts::FRAC_1_SQRT_2,
    )
}

pub(in super::super) fn read_clean_gltf_mesh_node_transform(
    path: &Path,
) -> Result<((f64, f64, f64), (f64, f64, f64, f64), (f64, f64, f64)), String> {
    let (document, _, _) =
        gltf::import(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let node = document
        .nodes()
        .find(|node| node.mesh().is_some())
        .ok_or_else(|| format!("{} has no GLB mesh node", path.display()))?;
    let mut parent_by_child = BTreeMap::<usize, usize>::new();
    for parent in document.nodes() {
        for child in parent.children() {
            parent_by_child.insert(child.index(), parent.index());
        }
    }

    let mut chain = vec![node.index()];
    let mut current = node.index();
    while let Some(parent) = parent_by_child.get(&current).copied() {
        chain.push(parent);
        current = parent;
    }
    chain.reverse();

    let nodes_by_index = document
        .nodes()
        .map(|node| (node.index(), node))
        .collect::<BTreeMap<_, _>>();
    let mut translation = (0.0, 0.0, 0.0);
    let mut rotation = (0.0, 0.0, 0.0, 1.0);
    let mut scale = (1.0, 1.0, 1.0);
    for node_index in chain {
        let (local_t, local_r, local_s) = nodes_by_index[&node_index].transform().decomposed();
        let local_translation = gltf_clean_translation(local_t);
        let local_rotation = gltf_clean_rotation(local_r);
        let local_scale = gltf_clean_scale(local_s);
        translation = clean_add_vec3(
            translation,
            clean_rotate_vec3(rotation, clean_mul_vec3(scale, local_translation)),
        );
        rotation = clean_quat_mul(rotation, local_rotation);
        scale = clean_mul_vec3(scale, local_scale);
    }

    Ok((translation, rotation, scale))
}

pub(in super::super) fn legacy_safe_clean_gltf_joints(joints: &[CleanGltfJoint]) -> Vec<CleanGltfJoint> {
    let mut seen = BTreeMap::<String, usize>::new();
    joints
        .iter()
        .map(|joint| {
            let mut runtime_name = clean_gltf_legacy_safe_bone_name(&joint.name);
            if runtime_name.is_empty() {
                runtime_name = format!("node_{}", joint.node_index);
            }
            let key = runtime_name.to_ascii_lowercase();
            let suffix = seen.entry(key).or_insert(0);
            if *suffix > 0 {
                runtime_name = format!("{runtime_name}_{}", *suffix + 1);
            }
            *suffix += 1;
            CleanGltfJoint {
                node_index: joint.node_index,
                name: runtime_name,
                parent_node_index: joint.parent_node_index,
                children_node_indices: joint.children_node_indices.clone(),
                translation: joint.translation,
                rotation: joint.rotation,
                scale: joint.scale,
            }
        })
        .collect()
}

pub(in super::super) fn clean_gltf_legacy_palette_sort_key(name: &str) -> (u16, String, String) {
    let normalized = clean_gltf_legacy_safe_bone_name(name);
    let short = normalized
        .strip_prefix("Bip01 ")
        .unwrap_or(&normalized)
        .to_string();
    let lower = short.to_ascii_lowercase();
    let (group, detail) = if lower == "head" {
        (0, String::new())
    } else if let Some(suffix) = lower.strip_prefix("head_") {
        (1, suffix.to_string())
    } else if let Some(suffix) = lower.strip_prefix("l ") {
        (
            10 + clean_gltf_side_bone_group_rank(suffix),
            suffix.to_string(),
        )
    } else if lower == "neck" {
        (30, String::new())
    } else if lower == "pelvis" {
        (31, String::new())
    } else if let Some(suffix) = lower.strip_prefix("r ") {
        (
            40 + clean_gltf_side_bone_group_rank(suffix),
            suffix.to_string(),
        )
    } else if lower == "spine" {
        (70, String::new())
    } else if let Some(suffix) = lower.strip_prefix("spine") {
        (71, suffix.to_string())
    } else if lower == "nonaccum" {
        (90, String::new())
    } else if lower.contains("iktarget") {
        (95, lower.clone())
    } else {
        (80, lower.clone())
    };
    (group, detail, normalized)
}

pub(in super::super) fn clean_gltf_palette_joint_indices(
    joints: &[CleanGltfJoint],
    weighted_joint_indices: &BTreeSet<usize>,
) -> Vec<usize> {
    if weighted_joint_indices.is_empty() {
        return Vec::new();
    }
    let joint_by_node = joints
        .iter()
        .enumerate()
        .map(|(index, joint)| (joint.node_index, (index, joint)))
        .collect::<BTreeMap<_, _>>();
    let mut keep = BTreeSet::<usize>::new();
    for &joint_index in weighted_joint_indices {
        let Some(joint) = joints.get(joint_index) else {
            continue;
        };
        let mut current = Some(joint.node_index);
        while let Some(node_index) = current {
            let Some((ancestor_index, ancestor_joint)) = joint_by_node.get(&node_index) else {
                break;
            };
            keep.insert(*ancestor_index);
            current = ancestor_joint.parent_node_index;
        }
    }
    let mut ordered = keep.into_iter().collect::<Vec<_>>();
    ordered.retain(|index| {
        let Some(joint) = joints.get(*index) else {
            return false;
        };
        let normalized = clean_gltf_legacy_safe_bone_name(&joint.name).to_ascii_lowercase();
        if weighted_joint_indices.contains(index) {
            return true;
        }
        normalized != "bip01 nonaccum" && normalized != "bip01 footsteps"
    });
    ordered.sort_by(|left, right| {
        clean_gltf_legacy_palette_sort_key(&joints[*left].name)
            .cmp(&clean_gltf_legacy_palette_sort_key(&joints[*right].name))
            .then_with(|| left.cmp(right))
    });
    ordered
}
