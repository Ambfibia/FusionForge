use super::*;

#[test]
fn npc_icon_runtime_paths_use_tabledata_prefixes_and_two_digit_suffixes() {
    let cases = [
        (0, "wpnicon"),
        (1, "nanoicon"),
        (2, "skillicon"),
        (3, "cosicon"),
        (4, "npcicon"),
        (5, "nanoready"),
        (6, "questitemicon"),
        (7, "generalitemicon"),
        (8, "mobicon"),
        (9, "fusionicon"),
        (10, "hnpcicon"),
        (11, "transport"),
        (12, "vehicle"),
    ];

    for (icon_type, prefix) in cases {
        let path = npc_icon_runtime_asset_path(icon_type, 7).expect("known icon type");
        assert_eq!(path, format!("icons/{prefix}_07.png"));
        let parsed = parse_npc_icon_asset_path(&path).expect("parse generated icon path");
        assert_eq!(parsed.prefix, prefix);
        assert_eq!(parsed.number, 7);
        assert_eq!(npc_icon_type_for_prefix(&parsed.prefix), Some(icon_type));
    }

    assert_eq!(
        npc_icon_runtime_asset_path(4, 141).as_deref(),
        Some("icons/npcicon_141.png")
    );
    assert_eq!(
        npc_icon_runtime_asset_path(1, 1).as_deref(),
        Some("icons/nanoicon_01.png")
    );
    assert_eq!(
        npc_icon_runtime_asset_path(3, 3).as_deref(),
        Some("icons/cosicon_03.png")
    );
    assert!(npc_icon_runtime_asset_path(13, 1).is_none());
    assert!(npc_icon_runtime_asset_path(4, -1).is_none());
}

#[test]
fn npc_otto_remapped_skin_indices_fit_runtime_palette() {
    let repo_root = crate::repository_root().parent().unwrap();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let raw_joints = read_clean_gltf_joints(&path).unwrap();
    let joints = legacy_safe_clean_gltf_joints(&raw_joints);
    let mut mesh = fusionforge::modding::ImportedMesh::from_model_path(&path, None).unwrap();
    let weighted_joint_indices = mesh.used_joint_indices();
    let palette_joint_indices =
        clean_gltf_palette_joint_indices(&joints, &weighted_joint_indices);
    let target_bones = palette_joint_indices
        .iter()
        .filter_map(|index| joints.get(*index))
        .map(|joint| joint.name.clone())
        .collect::<Vec<_>>();
    let warnings = mesh.remap_skin_to_bones(&target_bones);

    assert!(
        warnings.is_empty(),
        "otto remap warnings were not empty: {warnings:?}"
    );
    assert_eq!(mesh.bind_poses.len(), target_bones.len());

    let max_bone_index = mesh
        .skin
        .iter()
        .flat_map(|skin| skin.bone_indices)
        .max()
        .unwrap_or(0);
    assert!(
        max_bone_index >= 0,
        "otto remapped max bone index was negative: {max_bone_index}"
    );
    assert!(
        (max_bone_index as usize) < target_bones.len(),
        "otto remapped max bone index {max_bone_index} exceeded palette len {}",
        target_bones.len()
    );
}
