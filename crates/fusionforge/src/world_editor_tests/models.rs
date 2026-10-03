use super::*;

#[test]
fn exact_unrelated_mesh_warning_becomes_typed_selection_proof() {
    let mut source = json!({
        "meshes": [{"name": "selected-a"}, {"name": "selected-b"}],
        "warnings": [
            "Ignored 4 mesh(es) from unrelated character roots in the shared preload range."
        ]
    });

    let proof = resolve_exact_unrelated_mesh_filter_warning(&mut source)
        .expect("character-root filtering is positive ownership evidence");
    assert_eq!(proof["excludedCandidateMeshes"], json!(4));
    assert_eq!(proof["selectedMeshes"], json!(2));
    assert_eq!(source["warnings"], json!([]));
    assert_eq!(
        source["exactMeshSelectionProof"]["warningDisposition"],
        json!("resolved-as-positive-character-root-ownership-proof")
    );
}

#[test]
fn exact_nif_warning_is_suppressed_only_for_the_pointer_identical_serialized_root() {
    let route = "wear/hat_apacheshield.nif";
    let mut source = json!({
        "exactContainerTargets": [{
            "proof": "directAssetBundleContainerPointer",
            "exactRoute": route,
            "containerAssetName": "CustomAssetBundle-ce09",
            "assetIndex": 3,
            "assetName": "CustomAssetBundle-ce09",
            "pathId": 44,
            "objectType": "GameObject"
        }],
        "modelHierarchy": {
            "roots": [{
                "name": "hat_apacheshield",
                "sourceAssetIndex": 3,
                "transformPathId": 45,
                "gameObjectPathId": 44
            }]
        },
        "warnings": [
            "wear/hat_apacheshield.nif: NIF parse failed: binrw error",
            "unrelated warning remains fail-closed"
        ]
    });

    let evidence = suppress_exact_nif_parse_warning_for_serialized_game_object(
        &mut source,
        &normalized_asset_path(route),
        &[route.to_string()],
    )
    .expect("typed serialized GameObject evidence");

    assert_eq!(
        evidence["proof"],
        json!("directAssetBundleContainerPointerMatchesOnlyHierarchyRoot")
    );
    assert_eq!(evidence["gameObjectPathId"], json!(44));
    assert_eq!(
        source["warnings"],
        json!(["unrelated warning remains fail-closed"])
    );
    assert_eq!(
        source["exactSerializedGameObjectClosure"]["warningDisposition"],
        json!(
            "legacy-nif-byte-parse-is-non-authoritative-for-direct-serialized-game-object-route"
        )
    );
}

#[test]
fn exact_nif_byte_parse_is_skipped_for_the_pointer_identical_serialized_root() {
    let route = "wear/hat_apacheshield.nif";
    let mut source = json!({
        "exactContainerTargets": [{
            "proof": "directAssetBundleContainerPointer",
            "exactRoute": route,
            "containerAssetName": "CustomAssetBundle-ce09",
            "assetIndex": 3,
            "assetName": "CustomAssetBundle-ce09",
            "pathId": 44,
            "objectType": "GameObject"
        }],
        "modelHierarchy": {
            "roots": [{
                "name": "hat_apacheshield",
                "sourceAssetIndex": 3,
                "transformPathId": 45,
                "gameObjectPathId": 44
            }]
        },
        "warnings": []
    });

    let evidence = suppress_exact_nif_parse_warning_for_serialized_game_object(
        &mut source,
        &normalized_asset_path(route),
        &[route.to_string()],
    )
    .expect("typed serialized GameObject evidence");

    assert_eq!(
        evidence["byteParseDisposition"],
        json!("skipped-because-direct-serialized-game-object-is-authoritative")
    );
    assert!(evidence["suppressedWarning"].is_null());
    assert_eq!(source["warnings"], json!([]));
    assert_eq!(
        source["exactSerializedGameObjectClosure"]["proof"],
        json!("directAssetBundleContainerPointerMatchesOnlyHierarchyRoot")
    );
}

#[test]
fn exact_nif_warning_stays_blocking_when_pointer_and_hierarchy_root_differ() {
    let route = "wear/hat_apacheshield.nif";
    let warning = "wear/hat_apacheshield.nif: NIF parse failed: binrw error";
    let mut source = json!({
        "exactContainerTargets": [{
            "proof": "directAssetBundleContainerPointer",
            "exactRoute": route,
            "containerAssetName": "CustomAssetBundle-ce09",
            "assetIndex": 3,
            "assetName": "CustomAssetBundle-ce09",
            "pathId": 44,
            "objectType": "GameObject"
        }],
        "modelHierarchy": {
            "roots": [{
                "name": "different_root",
                "sourceAssetIndex": 3,
                "transformPathId": 46,
                "gameObjectPathId": 99
            }]
        },
        "warnings": [warning]
    });

    assert!(suppress_exact_nif_parse_warning_for_serialized_game_object(
        &mut source,
        &normalized_asset_path(route),
        &[route.to_string()],
    )
    .is_none());
    assert_eq!(source["warnings"], json!([warning]));
    assert!(source.get("exactSerializedGameObjectClosure").is_none());
}

pub(super) fn live_fusion_model_preview(model_path: &str) -> Option<JsonValue> {
    let project = default_repo_root()
        .join("builds")
        .join("retrobution-20260613.ffclient");
    let index_path = project.join("cache").join("bundle-index.json");
    if !index_path.is_file() {
        eprintln!("skipping live Fusion model preview test: bundle index not found");
        return None;
    }
    let index = serde_json::from_str::<ClientFileIndex>(
        &fs::read_to_string(&index_path).expect("read bundle index"),
    )
    .expect("parse bundle index");
    let wanted = normalized_asset_path(model_path);
    let wanted_nif = wanted.trim_end_matches(".kfm").to_string() + ".nif";
    let mut candidates = index
        .bundles
        .iter()
        .filter(|bundle| {
            bundle.assets.iter().any(|asset| {
                asset.container_paths.iter().any(|path| {
                    let normalized = normalized_asset_path(path);
                    normalized == wanted || normalized == wanted_nif
                })
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        eprintln!("skipping live Fusion model preview test: model bundle not indexed");
        return None;
    }
    candidates.sort_by_key(|bundle| {
        let name = bundle.name.to_ascii_lowercase();
        let score = if name.contains("dongresources") {
            0
        } else if name.contains("character") {
            1
        } else {
            2
        };
        (score, name)
    });
    let bundle = candidates.remove(0);
    Some(
        preview_bundle_container_model(
            bundle.path.clone(),
            Some(project.to_string_lossy().to_string()),
            vec![model_path.to_string()],
        )
        .expect("build Fusion model preview"),
    )
}

#[test]
fn legacy_manifest_authoring_model_path_parses_existing_warning() {
    let manifest = json!({
        "warnings": [
            "Converted authoring model D:\\CodexProject\\FusionFallProject\\Models\\npc_otto.glb into a clean Unity resourceFile D:\\CodexProject\\FusionFallProject\\builds\\foo.ffclient\\npcs\\generated\\Character_Otto__authoring.resourceFile with generated Unity objects and AssetBundle container entries mob/npc_otto.kfm; skin is remapped to the generated Unity runtime bone palette."
        ]
    });

    let path = legacy_manifest_authoring_model_path(&manifest).expect("authoring model path");

    assert_eq!(
        path,
        PathBuf::from(r"D:\CodexProject\FusionFallProject\Models\npc_otto.glb")
    );
}

#[test]
fn authoring_model_path_from_manifest_prefers_blueprint_field() {
    let blueprint = NpcBlueprint {
        npc_id: 3463,
        template_npc_id: Some(1180),
        name: "Otto".to_string(),
        internal_name: "npc_otto".to_string(),
        comment: None,
        comment1: None,
        greeting_name: None,
        greeting_comment: None,
        greeting_comment1: None,
        greeting_key: None,
        authoring_model_path: Some(r"D:\Models\otto.glb".to_string()),
        model_bundle: None,
        model_asset: Some("mob/npc_otto.kfm".to_string()),
        texture_bundle: None,
        texture_asset: None,
        icon_bundle: None,
        icon_asset: None,
        audio_source: None,
        audio_prefix: None,
        animation_set: None,
        generated_icon: None,
        profile: BTreeMap::new(),
        spawn_map: None,
        spawn_position: None,
        spawn_angle: None,
        spawn_json_id: None,
        notes: None,
    };
    let manifest = json!({
        "warnings": [
            "Converted authoring model D:\\Legacy\\otto.glb into a clean Unity resourceFile D:\\Legacy\\Character_Otto__authoring.resourceFile with generated Unity objects and AssetBundle container entries mob/npc_otto.kfm; skin is remapped to the generated Unity runtime bone palette."
        ]
    });

    let path = authoring_model_path_from_manifest(Path::new("."), &blueprint, &manifest)
        .expect("authoring path");

    assert_eq!(path, PathBuf::from(r"D:\Models\otto.glb"));
}

#[test]
fn clean_skinned_renderer_keeps_bindposes_on_mesh_only() {
    let asset = test_clean_asset();
    let bind_poses = vec![fusionforge::modding::ImportedMatrix4x4 {
        values: [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ],
    }];

    let renderer =
        clean_skinned_renderer(&asset, 2, 6, 8, &[10, 11], &bind_poses).expect("renderer");
    let object = renderer.as_object().expect("renderer object");

    assert_eq!(
        object
            .get("m_BindPose")
            .and_then(fusionforge::UnityValue::as_array)
            .map(|values| values.len()),
        Some(0)
    );
    assert_eq!(
        object
            .get("m_Bones")
            .and_then(fusionforge::UnityValue::as_array)
            .map(|values| values.len()),
        Some(2)
    );
}

#[test]
fn clean_gltf_runtime_paths_preserve_alias_and_ik_nodes() {
    let joints = vec![
        test_joint(1, "Bip01 NonAccum", None, vec![2]),
        test_joint(2, "Bip01 Head", Some(1), vec![3]),
        test_joint(3, "Bip01 Head.002", Some(2), vec![]),
        test_joint(4, "Bip01 L Calf-IKTarget", None, vec![]),
    ];

    let paths = clean_gltf_runtime_joint_paths(&joints);

    assert!(paths.contains("Bip01/Bip01 NonAccum"));
    assert!(paths.contains("Bip01/Bip01 NonAccum/Bip01 Head"));
    assert!(paths.contains("Bip01/Bip01 NonAccum/Bip01 Head/Bip01 Head.002"));
    assert!(paths.contains("Bip01/Bip01 L Calf-IKTarget"));
}

#[test]
fn legacy_safe_clean_gltf_joints_preserve_unique_nodes() {
    let joints = vec![
        test_joint(1, "Bip01 Head", None, vec![2]),
        test_joint(2, "Bip01 Head.002", Some(1), vec![3]),
        test_joint(3, "Bip01 L Calf-IKTarget.001", Some(2), vec![]),
    ];

    let runtime = legacy_safe_clean_gltf_joints(&joints);
    let names = runtime
        .iter()
        .map(|joint| joint.name.clone())
        .collect::<Vec<_>>();

    assert_eq!(names[0], "Bip01 Head");
    assert_eq!(names[1], "Bip01 Head_002");
    assert_eq!(names[2], "Bip01 L Calf_IKTarget_001");
}

#[test]
fn legacy_safe_clean_gltf_joints_keeps_alias_skin_nodes() {
    let joints = vec![
        test_joint(1, "Bip01 Head", None, vec![2, 3]),
        test_joint(2, "Bip01 Head.002", Some(1), vec![]),
        test_joint(3, "Bip01 L Calf-IKTarget", Some(1), vec![4]),
        test_joint(4, "Bip01 L Calf-IKTarget.001", Some(3), vec![]),
    ];

    let runtime = legacy_safe_clean_gltf_joints(&joints);
    let names = runtime
        .iter()
        .map(|joint| joint.name.clone())
        .collect::<Vec<_>>();

    assert_eq!(runtime.len(), 4);
    assert_eq!(
        names,
        vec![
            "Bip01 Head".to_string(),
            "Bip01 Head_002".to_string(),
            "Bip01 L Calf_IKTarget".to_string(),
            "Bip01 L Calf_IKTarget_001".to_string(),
        ]
    );
}

#[test]
fn clean_gltf_palette_joint_indices_keeps_ancestor_chain() {
    let joints = vec![
        test_joint(1, "Bip01 Pelvis", None, vec![2, 5]),
        test_joint(2, "Bip01 Spine", Some(1), vec![3]),
        test_joint(3, "Bip01 Neck", Some(2), vec![4]),
        test_joint(4, "Bip01 Head", Some(3), vec![]),
        test_joint(5, "Bip01 L Clavicle", Some(1), vec![6]),
        test_joint(6, "Bip01 L UpperArm", Some(5), vec![]),
    ];

    let selected = clean_gltf_palette_joint_indices(&joints, &BTreeSet::from([3usize, 5usize]));

    assert_eq!(selected, vec![3, 4, 5, 2, 0, 1]);
}

#[test]
fn clean_gltf_palette_joint_indices_uses_legacy_like_order() {
    let joints = vec![
        test_joint(1, "Bip01 Pelvis", None, vec![]),
        test_joint(2, "Bip01 Spine", None, vec![]),
        test_joint(3, "Bip01 Head", None, vec![]),
        test_joint(4, "Bip01 R Hand", None, vec![]),
        test_joint(5, "Bip01 L Hand", None, vec![]),
    ];

    let selected = clean_gltf_palette_joint_indices(
        &joints,
        &BTreeSet::from([0usize, 1usize, 2usize, 3usize, 4usize]),
    );

    assert_eq!(selected, vec![2, 4, 0, 3, 1]);
}

#[test]
fn clean_gltf_palette_joint_indices_skips_nonaccum_helper_root() {
    let joints = vec![
        test_joint(1, "Bip01 NonAccum", None, vec![2]),
        test_joint(2, "Bip01 Pelvis", Some(1), vec![3]),
        test_joint(3, "Bip01 Spine", Some(2), vec![4]),
        test_joint(4, "Bip01 Head", Some(3), vec![]),
    ];

    let selected = clean_gltf_palette_joint_indices(&joints, &BTreeSet::from([3usize]));

    assert_eq!(selected, vec![3, 1, 2]);
}

#[test]
fn legacy_skinned_mesh_basis_rotation_matches_reference_quaternion() {
    let rotation = legacy_skinned_mesh_basis_rotation();
    assert!((rotation.0 + 0.7071067811865476).abs() < 1.0e-12);
    assert_eq!(rotation.1, 0.0);
    assert_eq!(rotation.2, 0.0);
    assert!((rotation.3 - 0.7071067811865476).abs() < 1.0e-12);
}

#[test]
fn read_clean_gltf_joints_collapses_otto_non_joint_ancestors() {
    let repo_root = crate::repository_root().parent().unwrap();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let joints = read_clean_gltf_joints(&path).unwrap();
    let mesh = fusionforge::modding::ImportedMesh::from_model_path(&path, None).unwrap();

    assert_eq!(joints.len(), mesh.bind_poses.len());
    let joint_slot_by_node = joints
        .iter()
        .enumerate()
        .map(|(slot, joint)| (joint.node_index, slot))
        .collect::<BTreeMap<_, _>>();
    let multiply = |left: [[f64; 4]; 4], right: [[f64; 4]; 4]| {
        let mut out = [[0.0_f64; 4]; 4];
        for row in 0..4 {
            for col in 0..4 {
                out[row][col] = (0..4)
                    .map(|index| left[row][index] * right[index][col])
                    .sum();
            }
        }
        out
    };

    let mut global_rotation = vec![(0.0, 0.0, 0.0, 1.0); joints.len()];
    let mut global_translation = vec![(0.0, 0.0, 0.0); joints.len()];
    let mut global_scale = vec![(1.0, 1.0, 1.0); joints.len()];
    let mut max_error = 0.0_f64;
    for (slot, joint) in joints.iter().enumerate() {
        if let Some(parent_slot) = joint
            .parent_node_index
            .and_then(|parent| joint_slot_by_node.get(&parent))
            .copied()
        {
            global_translation[slot] = clean_add_vec3(
                global_translation[parent_slot],
                clean_rotate_vec3(
                    global_rotation[parent_slot],
                    clean_mul_vec3(global_scale[parent_slot], joint.translation),
                ),
            );
            global_rotation[slot] =
                clean_quat_mul(global_rotation[parent_slot], joint.rotation);
            global_scale[slot] = clean_mul_vec3(global_scale[parent_slot], joint.scale);
        } else {
            global_translation[slot] = joint.translation;
            global_rotation[slot] = joint.rotation;
            global_scale[slot] = joint.scale;
        }

        let (x, y, z, w) = global_rotation[slot];
        let xx = x * x;
        let yy = y * y;
        let zz = z * z;
        let xy = x * y;
        let xz = x * z;
        let yz = y * z;
        let wx = w * x;
        let wy = w * y;
        let wz = w * z;
        let rotation = [
            [1.0 - 2.0 * (yy + zz), 2.0 * (xy - wz), 2.0 * (xz + wy), 0.0],
            [2.0 * (xy + wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz - wx), 0.0],
            [2.0 * (xz - wy), 2.0 * (yz + wx), 1.0 - 2.0 * (xx + yy), 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let scale = [
            [global_scale[slot].0, 0.0, 0.0, 0.0],
            [0.0, global_scale[slot].1, 0.0, 0.0],
            [0.0, 0.0, global_scale[slot].2, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let mut global_matrix = multiply(rotation, scale);
        global_matrix[0][3] = global_translation[slot].0;
        global_matrix[1][3] = global_translation[slot].1;
        global_matrix[2][3] = global_translation[slot].2;

        let bind_pose = &mesh.bind_poses[slot].values;
        let bind_matrix = [
            [bind_pose[0], bind_pose[1], bind_pose[2], bind_pose[3]],
            [bind_pose[4], bind_pose[5], bind_pose[6], bind_pose[7]],
            [bind_pose[8], bind_pose[9], bind_pose[10], bind_pose[11]],
            [bind_pose[12], bind_pose[13], bind_pose[14], bind_pose[15]],
        ];
        let composed = multiply(global_matrix, bind_matrix);
        let mut error = 0.0_f64;
        for row in 0..4 {
            for col in 0..4 {
                let expected = if row == col { 1.0 } else { 0.0 };
                error += (composed[row][col] - expected).abs();
            }
        }
        max_error = max_error.max(error);
    }

    assert!(
        max_error < 1e-3,
        "otto collapsed joint max error was {max_error}"
    );
}

#[test]
fn clean_gltf_root_gameobject_name_uses_internal_name_for_skinned_npc() {
    assert_eq!(
        clean_gltf_root_gameobject_name("npc_otto", false),
        "npc_otto"
    );
    assert_eq!(
        clean_gltf_root_gameobject_name("npc_agentsix", false),
        "npc_agentsix"
    );
}
