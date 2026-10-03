use super::super::*;

pub(in super::super) fn sanitize_clean_gltf_joints_for_runtime(joints: &[CleanGltfJoint]) -> Vec<CleanGltfJoint> {
    let names = joints
        .iter()
        .map(|joint| joint.name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut keep_nodes = BTreeSet::<usize>::new();
    for joint in joints {
        let runtime_name = clean_gltf_runtime_bone_name(&joint.name);
        let is_alias = !runtime_name.eq_ignore_ascii_case(&joint.name)
            && names.contains(&runtime_name.to_ascii_lowercase());
        if !is_alias {
            keep_nodes.insert(joint.node_index);
        }
    }

    let joint_by_node = joints
        .iter()
        .map(|joint| (joint.node_index, joint))
        .collect::<BTreeMap<_, _>>();
    let nearest_kept_parent = |joint: &CleanGltfJoint| {
        let mut parent = joint.parent_node_index;
        while let Some(parent_node) = parent {
            if keep_nodes.contains(&parent_node) {
                return Some(parent_node);
            }
            parent = joint_by_node
                .get(&parent_node)
                .and_then(|parent_joint| parent_joint.parent_node_index);
        }
        None
    };
    let kept_parent_by_node = joints
        .iter()
        .filter(|joint| keep_nodes.contains(&joint.node_index))
        .map(|joint| (joint.node_index, nearest_kept_parent(joint)))
        .collect::<BTreeMap<_, _>>();

    joints
        .iter()
        .filter(|joint| keep_nodes.contains(&joint.node_index))
        .map(|joint| {
            let children_node_indices = kept_parent_by_node
                .iter()
                .filter_map(|(node, parent)| {
                    if *parent == Some(joint.node_index) {
                        Some(*node)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            CleanGltfJoint {
                node_index: joint.node_index,
                name: clean_gltf_runtime_bone_name(&joint.name),
                parent_node_index: kept_parent_by_node
                    .get(&joint.node_index)
                    .copied()
                    .flatten(),
                children_node_indices,
                translation: joint.translation,
                rotation: joint.rotation,
                scale: joint.scale,
            }
        })
        .collect()
}

pub(in super::super) fn clean_gltf_needs_bip01_wrapper(joints: &[CleanGltfJoint]) -> bool {
    !joints
        .iter()
        .any(|joint| joint.name.eq_ignore_ascii_case("Bip01"))
        && joints
            .iter()
            .any(|joint| joint.parent_node_index.is_none() && joint.name == "Bip01 NonAccum")
}

pub(in super::super) fn clean_gltf_runtime_joint_paths_by_node(joints: &[CleanGltfJoint]) -> BTreeMap<usize, String> {
    fn build_path(
        node_index: usize,
        joint_by_node: &BTreeMap<usize, &CleanGltfJoint>,
        cache: &mut BTreeMap<usize, String>,
        needs_bip01_wrapper: bool,
    ) -> String {
        if let Some(path) = cache.get(&node_index) {
            return path.clone();
        }
        let joint = joint_by_node[&node_index];
        let path = if let Some(parent_node_index) = joint.parent_node_index {
            let parent_path =
                build_path(parent_node_index, joint_by_node, cache, needs_bip01_wrapper);
            format!("{parent_path}/{}", joint.name)
        } else if needs_bip01_wrapper {
            format!("Bip01/{}", joint.name)
        } else {
            joint.name.clone()
        };
        cache.insert(node_index, path.clone());
        path
    }

    let joint_by_node = joints
        .iter()
        .map(|joint| (joint.node_index, joint))
        .collect::<BTreeMap<_, _>>();
    let mut cache = BTreeMap::<usize, String>::new();
    let needs_bip01_wrapper = clean_gltf_needs_bip01_wrapper(joints);
    let mut paths = joints
        .iter()
        .map(|joint| {
            (
                joint.node_index,
                build_path(
                    joint.node_index,
                    &joint_by_node,
                    &mut cache,
                    needs_bip01_wrapper,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if needs_bip01_wrapper && !paths.values().any(|path| path == "Bip01") {
        paths.insert(usize::MAX, "Bip01".to_string());
    }
    paths
}

pub(in super::super) fn clean_gltf_runtime_joint_paths(joints: &[CleanGltfJoint]) -> BTreeSet<String> {
    clean_gltf_runtime_joint_paths_by_node(joints)
        .into_values()
        .collect()
}

pub(in super::super) fn clean_gltf_root_gameobject_name(internal_name: &str, _static_mesh: bool) -> String {
    internal_name.to_string()
}

pub(in super::super) fn clean_mesh_filter(
    asset: &fusionforge::Asset,
    game_object_path_id: i64,
    mesh_path_id: i64,
) -> Result<fusionforge::UnityValue, String> {
    let mut value = asset.empty_object_value_for_class(33)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "m_GameObject".to_string(),
            unity_local_pointer(0, game_object_path_id),
        );
        object.insert("m_Mesh".to_string(), unity_local_pointer(0, mesh_path_id));
    }
    Ok(value)
}

pub(in super::super) fn clean_mesh_renderer(
    asset: &fusionforge::Asset,
    game_object_path_id: i64,
    material_path_id: i64,
) -> Result<fusionforge::UnityValue, String> {
    let mut value = asset.empty_object_value_for_class(23)?;
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
    }
    Ok(value)
}

pub(in super::super) fn preserve_legacy_mesh_compressed_defaults(
    mesh_value: &mut fusionforge::UnityValue,
    template_mesh_value: &fusionforge::UnityValue,
) {
    let Some(mesh_object) = mesh_value.as_object_mut() else {
        return;
    };
    let Some(compressed_mesh) = mesh_object.get_mut("m_CompressedMesh") else {
        return;
    };
    let Some(template_compressed_mesh) = template_mesh_value
        .as_object()
        .and_then(|object| object.get("m_CompressedMesh"))
    else {
        return;
    };

    for field in ["m_BindPoses", "m_Tangents", "m_TangentSigns"] {
        let should_copy = compressed_mesh
            .as_object()
            .and_then(|object| object.get(field))
            .and_then(fusionforge::UnityValue::as_object)
            .is_some_and(|object| {
                object
                    .get("m_NumItems")
                    .and_then(fusionforge::UnityValue::as_i64)
                    .unwrap_or_default()
                    == 0
                    && object
                        .get("m_BitSize")
                        .and_then(fusionforge::UnityValue::as_i64)
                        .unwrap_or_default()
                        == 0
            });
        if should_copy {
            maybe_copy_object_field(compressed_mesh, template_compressed_mesh, field);
        }
    }
}
