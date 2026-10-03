use super::*;

#[test]
fn npc_otto_bind_poses_match_runtime_joint_globals() {
    let repo_root = crate::repository_root().parent().unwrap();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let mesh = ImportedMesh::from_model_path(&path, None).unwrap();
    assert_eq!(mesh.joint_names.len(), mesh.bind_poses.len());

    let (document, _, _) = gltf::import(&path).unwrap();
    let skin = document.skins().next().unwrap();
    let joints = skin.joints().collect::<Vec<_>>();
    assert_eq!(joints.len(), mesh.bind_poses.len());

    let mut parent_by_child = BTreeMap::<usize, usize>::new();
    for node in document.nodes() {
        for child in node.children() {
            parent_by_child.insert(child.index(), node.index());
        }
    }

    let local_by_node = document
        .nodes()
        .map(|node| {
            let (translation, rotation, scale) = node.transform().decomposed();
            (
                node.index(),
                trs_matrix4(
                    gltf_vec3_to_fusionfall(translation[0], translation[1], translation[2]),
                    gltf_quat_to_fusionfall(rotation[0], rotation[1], rotation[2], rotation[3]),
                    gltf_scale_to_fusionfall(scale[0], scale[1], scale[2]),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let joint_indices = joints
        .iter()
        .map(|joint| joint.index())
        .collect::<BTreeSet<_>>();
    let mut non_joint_parent_count = 0usize;
    let mut globals_by_node = BTreeMap::<usize, [[f64; 4]; 4]>::new();
    for joint in &joints {
        let mut chain = Vec::<usize>::new();
        let mut current = Some(joint.index());
        while let Some(node_index) = current {
            chain.push(node_index);
            current = parent_by_child.get(&node_index).copied();
        }
        chain.reverse();
        let mut global = [[0.0_f64; 4]; 4];
        global[0][0] = 1.0;
        global[1][1] = 1.0;
        global[2][2] = 1.0;
        global[3][3] = 1.0;
        for (depth, node_index) in chain.iter().copied().enumerate() {
            if depth + 1 != chain.len() && !joint_indices.contains(&node_index) {
                non_joint_parent_count += 1;
            }
            global = multiply_matrix4(global, *local_by_node.get(&node_index).unwrap());
        }
        globals_by_node.insert(joint.index(), global);
    }

    let max_error = joints
        .iter()
        .zip(&mesh.bind_poses)
        .map(|(joint, bind_pose)| {
            identity_error(multiply_matrix4(
                *globals_by_node.get(&joint.index()).unwrap(),
                imported_matrix4(bind_pose),
            ))
        })
        .fold(0.0_f64, f64::max);
    assert!(
        max_error < 1e-3,
        "npc_otto max global*bind error was {max_error}, non_joint_parent_count={non_joint_parent_count}"
    );
}
