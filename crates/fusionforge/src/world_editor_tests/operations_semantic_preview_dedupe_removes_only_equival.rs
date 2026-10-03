use super::*;

#[test]
fn exact_native_coordinate_contract_forbids_recenter_and_rescale() {
    let contract = native_coordinate_contract_json();
    assert_eq!(
        contract["unitScale"],
        json!("1-unity-unit-equals-1-bevy-unit")
    );
    assert_eq!(
        contract["originPolicy"],
        json!("source-root-trs-unchanged-no-auto-centering")
    );
    assert_eq!(contract["autoCentered"], json!(false));
    assert_eq!(contract["autoScaled"], json!(false));
    assert_eq!(contract["position"], json!("[-unity.x,unity.y,unity.z]"));
    assert_eq!(
        contract["inverseBindMatrix"],
        json!("H*unity*H where H=diag(-1,1,1,1)")
    );
}

#[test]
fn semantic_preview_dedupe_removes_only_equivalent_preload_copies() {
    let material = |id: &str, texture: &str| {
        json!({
            "id": id,
            "name": "npc_agentk9-main-link_a.dds",
            "shaderName": "SkinnedToonShading_blendSrcalphaInvsrcalpha",
            "color": "#808080",
            "colorAlpha": 1.0,
            "textureDataUrl": texture,
        })
    };
    let materials = BTreeMap::from([
        (
            "material-a".to_string(),
            material("material-a", "data:image/png;base64,AAAA"),
        ),
        (
            "material-b".to_string(),
            material("material-b", "data:image/png;base64,AAAA"),
        ),
        (
            "material-c".to_string(),
            material("material-c", "data:image/png;base64,BBBB"),
        ),
    ]);
    let mesh = |id: &str, path_id: i64, material_id: &str, renderer_path_id: i64| {
        json!({
            "id": id,
            "sourceAsset": "Character_Agent_K9",
            "pathId": path_id,
            "name": "npc_wilddog",
            "kind": "mesh",
            "positions": [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            "normals": [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            "uvs": [0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            "indices": [0, 1, 2],
            "groups": [{"start": 0, "count": 3, "materialIndex": 0}],
            "materialId": material_id,
            "materialIds": [material_id],
            "material": {"id": material_id, "name": "npc_agentk9-main-link_a.dds", "color": "#808080"},
            "skin": {
                "source": "unity-skinned-mesh-renderer",
                "rendererAssetIndex": 1,
                "rendererPathId": renderer_path_id,
                "rendererTransformPathId": renderer_path_id + 1,
                "jointPaths": ["Bip01"],
                "boneIndices": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                "weights": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
                "inverseBindMatrices": [[[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]],
            }
        })
    };
    let mut meshes = vec![
        mesh("Character_Agent_K9:74", 74, "material-a", 56),
        mesh("Character_Agent_K9:165", 165, "material-b", 147),
        mesh("Character_Agent_K9:254", 254, "material-c", 236),
    ];
    let clip = |asset: &str, path_id: i64| {
        json!({
            "asset": asset,
            "pathId": path_id,
            "name": "stand1",
            "duration": 1.0,
            "animationData": {"translations": [], "rotations": [], "scales": []},
        })
    };
    let mut animations = vec![clip("Character_Agent_K9", 64), clip("copy", 155)];
    let mut skeleton = json!({
        "joints": [
            {
                "path": "Bip01",
                "parent": null,
                "translation": [0.0, 0.0, 0.0],
                "rotation": [0.0, 0.0, 0.0, 1.0],
                "scale": [1.0, 1.0, 1.0],
                "sourceAssetIndex": 1,
                "transformPathId": 80,
            },
            {
                "path": "Bip01",
                "parent": null,
                "translation": [0.0, 0.0, 0.0],
                "rotation": [0.0, 0.0, 0.0, 1.0],
                "scale": [1.0, 1.0, 1.0],
                "sourceAssetIndex": 1,
                "transformPathId": 171,
            }
        ]
    });
    let mut warnings = Vec::new();

    let dedup = deduplicate_preview_semantic_copies(
        &mut meshes,
        &materials,
        &mut animations,
        &mut skeleton,
        &mut warnings,
    );

    assert_eq!(
        dedup,
        PreviewSemanticDedup {
            meshes: 1,
            animations: 1,
            joints: 1
        }
    );
    assert_eq!(meshes.len(), 2, "different texture material must survive");
    assert_eq!(animations.len(), 1);
    assert_eq!(skeleton["joints"].as_array().map(Vec::len), Some(1));
    assert!(warnings.is_empty());
}

#[test]
fn live_staged_fusion_finn_preview_resolves_sibling_materials_when_available() {
    let project = default_repo_root()
        .join("builds")
        .join("retrobution-20260613.ffclient");
    let bundle = project
        .join("npcs")
        .join("3462__Fusion_Finn")
        .join("assets")
        .join("TrainingGrounds");
    if !bundle.is_dir() {
        eprintln!("skipping live staged Fusion Finn preview: staged assets not found");
        return;
    }

    let preview = preview_bundle_container_model(
        bundle.to_string_lossy().to_string(),
        Some(project.to_string_lossy().to_string()),
        vec!["mob/mob_fusionfinn.kfm".to_string()],
    )
    .expect("staged Fusion Finn preview");
    let materials = preview["materials"]
        .as_object()
        .expect("Fusion Finn materials");
    assert!(
        !materials.is_empty(),
        "staged sibling asset directories must resolve Fusion Finn materials"
    );
    assert!(
        materials.values().any(|material| material
            .get("textureDataUrl")
            .and_then(JsonValue::as_str)
            .is_some_and(|value| value.starts_with("data:image/png;base64,"))),
        "Fusion Finn material should bind its staged sibling texture"
    );
    assert!(
        preview["meshes"]
            .as_array()
            .is_some_and(|meshes| meshes.iter().any(|mesh| mesh
                .get("materialId")
                .and_then(JsonValue::as_str)
                .is_some_and(|id| materials.contains_key(id)))),
        "Fusion Finn mesh should reference a resolved material"
    );
}

#[test]
fn live_staged_agent_k9_preview_collapses_preload_copies_when_available() {
    let project = default_repo_root()
        .join("builds")
        .join("retrobution-20260613.ffclient");
    let bundle = project
        .join("npcs")
        .join("3459__Agent_K9")
        .join("assets")
        .join("Character_Agent_K9");
    if !bundle.is_dir() {
        eprintln!("skipping live staged Agent K9 preview: staged assets not found");
        return;
    }

    let preview = preview_bundle_container_model(
        bundle.to_string_lossy().to_string(),
        Some(project.to_string_lossy().to_string()),
        vec!["mob/npc_agentk9.kfm".to_string()],
    )
    .expect("staged Agent K9 preview");
    let meshes = preview["meshes"].as_array().expect("Agent K9 meshes");
    assert_eq!(
        meshes.len(),
        1,
        "four equivalent preload meshes must collapse"
    );
    let animations = preview["animations"]
        .as_array()
        .expect("Agent K9 animations");
    let animation_names = animations
        .iter()
        .filter_map(|animation| animation.get("name").and_then(JsonValue::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(animations.len(), animation_names.len());
    assert_eq!(animation_names.len(), 3);
    let joints = preview
        .pointer("/skeleton/joints")
        .and_then(JsonValue::as_array)
        .expect("Agent K9 joints");
    let joint_paths = joints
        .iter()
        .filter_map(|joint| joint.get("path").and_then(JsonValue::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(joints.len(), joint_paths.len());
    assert_eq!(joint_paths.len(), 35);
    crate::npc_animation::NpcAnimationSampler::from_preview(&preview)
        .expect("deduplicated Agent K9 preview should be sampleable");
}

#[test]
fn migrates_world_document_json_to_v2_defaults() {
    let mut document = json!({
        "schemaVersion": 1,
        "tileId": "Map_05_04",
        "terrain": {}
    });

    migrate_world_document_json(&mut document);

    assert_eq!(document["schemaVersion"], json!(2));
    assert!(document["objects"].as_array().is_some());
    assert!(document["colliders"].as_array().is_some());
    assert!(document["waterVolumes"].as_array().is_some());
    assert!(document["previewMeshes"].as_array().is_some());
    assert!(document["previewMaterials"].as_object().is_some());
    assert!(document["previewTerrains"].as_array().is_some());
}

#[test]
fn generated_icon_binding_clones_shared_row_and_allocates_a_free_number() {
    let mut shared_table = json_to_unity_value(
        &json!({
            "m_pNpcData": [
                { "m_iIcon1": 1 },
                { "m_iIcon1": 1 }
            ],
            "m_pNpcIconData": [
                {},
                { "m_iIconType": 4, "m_iIconNumber": 70 }
            ]
        }),
        0,
    )
    .expect("shared icon table");

    let binding = bind_unique_generated_npc_icon_row(&mut shared_table, 0, 4, 2226)
        .expect("bind generated icon");
    assert_eq!(binding.row_index, 2);
    assert_eq!(binding.icon_number, 2226);
    assert!(binding.cloned_shared_row);
    assert_eq!(
        unity_usize_field(&npc_table_array(&shared_table, "m_pNpcData")[0], "m_iIcon1"),
        Some(2)
    );
    assert_eq!(
        unity_usize_field(&npc_table_array(&shared_table, "m_pNpcData")[1], "m_iIcon1"),
        Some(1)
    );
    assert_eq!(
        npc_icon_row_type_and_number(&npc_table_array(&shared_table, "m_pNpcIconData")[1]),
        Some((4, 70))
    );
    assert_eq!(
        npc_icon_row_type_and_number(&npc_table_array(&shared_table, "m_pNpcIconData")[2]),
        Some((4, 2226))
    );

    let mut colliding_table = json_to_unity_value(
        &json!({
            "m_pNpcData": [
                { "m_iIcon1": 1 }
            ],
            "m_pNpcIconData": [
                {},
                { "m_iIconType": 4, "m_iIconNumber": 70 },
                { "m_iIconType": 4, "m_iIconNumber": 2226 }
            ]
        }),
        0,
    )
    .expect("colliding icon table");
    let binding = bind_unique_generated_npc_icon_row(&mut colliding_table, 0, 4, 2226)
        .expect("allocate next icon number");
    assert_eq!(binding.row_index, 1);
    assert_eq!(binding.icon_number, 2227);
    assert!(!binding.cloned_shared_row);
    assert_eq!(
        npc_icon_row_type_and_number(&npc_table_array(&colliding_table, "m_pNpcIconData")[2]),
        Some((4, 2226))
    );
}

#[test]
fn blueprint_generated_icon_does_not_retarget_other_npcs_sharing_the_template_row() {
    let mut target_table = json_to_unity_value(
        &json!({
            "m_pNpcData": [
                { "m_iTeam": 1, "m_iMesh": 0, "m_iIcon1": 1 },
                { "m_iTeam": 1, "m_iMesh": 0, "m_iIcon1": 1 }
            ],
            "m_pNpcMeshData": [{}],
            "m_pNpcIconData": [
                {},
                { "m_iIconType": 4, "m_iIconNumber": 70 }
            ]
        }),
        0,
    )
    .expect("target table");
    let blueprint = NpcBlueprint {
        npc_id: 0,
        template_npc_id: None,
        name: "Generated icon NPC".to_string(),
        internal_name: "npc_generated_icon".to_string(),
        comment: None,
        comment1: None,
        greeting_name: None,
        greeting_comment: None,
        greeting_comment1: None,
        greeting_key: None,
        authoring_model_path: None,
        model_bundle: None,
        model_asset: None,
        texture_bundle: None,
        texture_asset: None,
        icon_bundle: Some("Icons.resourceFile".to_string()),
        icon_asset: Some("icons/npcicon_70.png".to_string()),
        audio_source: None,
        audio_prefix: None,
        animation_set: None,
        generated_icon: Some(NpcGeneratedIcon {
            file: "npcs/0__Generated/generated/npcicon_07.png".to_string(),
            asset_path: "icons/npcicon_07.png".to_string(),
            template_asset_path: "icons/npcicon_70.png".to_string(),
            size: 64,
            camera: None,
        }),
        profile: BTreeMap::new(),
        spawn_map: None,
        spawn_position: None,
        spawn_angle: None,
        spawn_json_id: None,
        notes: None,
    };

    assert!(apply_blueprint_npc_assets(&mut target_table, 0, &blueprint)
        .expect("apply generated icon blueprint"));
    assert_eq!(
        unity_usize_field(&npc_table_array(&target_table, "m_pNpcData")[0], "m_iIcon1"),
        Some(2)
    );
    assert_eq!(
        unity_usize_field(&npc_table_array(&target_table, "m_pNpcData")[1], "m_iIcon1"),
        Some(1)
    );
    assert_eq!(
        npc_icon_row_type_and_number(&npc_table_array(&target_table, "m_pNpcIconData")[1]),
        Some((4, 70))
    );
    assert_eq!(
        npc_icon_row_type_and_number(&npc_table_array(&target_table, "m_pNpcIconData")[2]),
        Some((4, 7))
    );
}

#[test]
fn configured_npc_icon_fallback_reuses_existing_donor_row() {
    let mut body = json_to_unity_value(
        &json!({
            "m_pNpcTable": {
                "m_pNpcData": [
                    {},
                    { "m_iIcon1": 1 },
                    { "m_iIcon1": 2 }
                ],
                "m_pNpcIconData": [
                    {},
                    { "m_iIconType": 4, "m_iIconNumber": 1 },
                    { "m_iIconType": 4, "m_iIconNumber": 3147 }
                ]
            }
        }),
        0,
    )
    .expect("npc table body");
    let config = json!({
        "NpcIconFallbacks": [{ "NpcId": 1, "FallbackNpcId": 2 }]
    });

    assert_eq!(
        apply_configured_npc_icon_fallbacks(&mut body, &config).expect("apply fallback"),
        1
    );
    let table = npc_table_from_body(&body).expect("npc table");
    assert_eq!(
        unity_usize_field(&npc_table_array(table, "m_pNpcData")[1], "m_iIcon1"),
        Some(2)
    );
    assert_eq!(
        npc_icon_row_type_and_number(&npc_table_array(table, "m_pNpcIconData")[2]),
        Some((4, 3147))
    );
}

pub(super) fn test_joint(
    node_index: usize,
    name: &str,
    parent_node_index: Option<usize>,
    children_node_indices: Vec<usize>,
) -> CleanGltfJoint {
    CleanGltfJoint {
        node_index,
        name: name.to_string(),
        parent_node_index,
        children_node_indices,
        translation: (0.0, 0.0, 0.0),
        rotation: (0.0, 0.0, 0.0, 1.0),
        scale: (1.0, 1.0, 1.0),
    }
}

pub(super) fn pair_values(
    value: &fusionforge::UnityValue,
) -> (&fusionforge::UnityValue, &fusionforge::UnityValue) {
    match value {
        fusionforge::UnityValue::Pair(left, right) => (&**left, &**right),
        other => panic!("expected pair, got {other:?}"),
    }
}

#[test]
fn rewrite_npc_output_pointers_preserves_unresolved_external_refs() {
    let mut asset = test_clean_asset();
    asset.objects.insert(
        100,
        fusionforge::ObjectInfo {
            path_id: 100,
            data_offset: 0,
            size: 0,
            type_id: 1,
            class_id: 1,
        },
    );
    asset.asset_refs.push(fusionforge::AssetRef {
        asset_path: String::new(),
        guid: [0; 16],
        type_id: 0,
        file_path: "sharedassets0.assets".to_string(),
    });

    let env = fusionforge::UnityEnvironment::from_assets(vec![asset]);
    let selected = BTreeSet::from([(0usize, 100i64)]);
    let externalized = BTreeMap::new();
    let external_ref_indices = BTreeMap::new();
    let explicit_external_ref_indices = BTreeMap::new();
    let output_path_ids = BTreeMap::from([((0usize, 100i64), 16i64)]);
    let mut value = unity_external_pointer(1, 100);

    rewrite_npc_output_pointers(
        &mut value,
        &env,
        0,
        &selected,
        &externalized,
        &external_ref_indices,
        &explicit_external_ref_indices,
        &output_path_ids,
    );

    assert_eq!(
        value.as_pointer(),
        Some(&fusionforge::Pointer {
            source_asset: 0,
            file_id: 1,
            path_id: 100,
        })
    );
}

pub(super) fn joint_globals_for_test(joints: &[CleanGltfJoint]) -> BTreeMap<usize, [[f64; 4]; 4]> {
    let mut globals = BTreeMap::<usize, [[f64; 4]; 4]>::new();
    let mut pending = joints
        .iter()
        .map(|joint| joint.node_index)
        .collect::<BTreeSet<_>>();
    while let Some(node_index) = pending.iter().copied().find(|node_index| {
        joints
            .iter()
            .find(|joint| joint.node_index == *node_index)
            .and_then(|joint| joint.parent_node_index)
            .is_none_or(|parent| !pending.contains(&parent))
    }) {
        let joint = joints
            .iter()
            .find(|joint| joint.node_index == node_index)
            .unwrap();
        let local = clean_trs_matrix4(joint.translation, joint.rotation, joint.scale);
        let global = if let Some(parent_node_index) = joint.parent_node_index {
            clean_multiply_matrix4(globals[&parent_node_index], local)
        } else {
            local
        };
        globals.insert(node_index, global);
        pending.remove(&node_index);
    }
    globals
}

#[test]
fn collapse_secondary_iktarget_roots_under_primary_reparents_otto_roots() {
    let repo_root = crate::repository_root().parent().unwrap();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let raw_joints = read_clean_gltf_joints(&path).unwrap();
    let mut joints = legacy_safe_clean_gltf_joints(&raw_joints);
    let globals_before = joint_globals_for_test(&joints);

    let root_names_before = joints
        .iter()
        .filter(|joint| joint.parent_node_index.is_none())
        .map(|joint| joint.name.clone())
        .collect::<Vec<_>>();
    assert!(root_names_before.len() > 1);
    assert!(root_names_before
        .iter()
        .any(|name| name == "Bip01 NonAccum"));
    assert!(root_names_before
        .iter()
        .any(|name| name.contains("IKTarget")));

    collapse_secondary_iktarget_roots_under_primary(&mut joints);

    let root_names_after = joints
        .iter()
        .filter(|joint| joint.parent_node_index.is_none())
        .map(|joint| joint.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(root_names_after, vec!["Bip01 NonAccum".to_string()]);

    let nonaccum_node_index = joints
        .iter()
        .find(|joint| joint.name == "Bip01 NonAccum")
        .map(|joint| joint.node_index)
        .unwrap();
    for joint in joints
        .iter()
        .filter(|joint| joint.name.contains("IKTarget"))
    {
        assert_eq!(joint.parent_node_index, Some(nonaccum_node_index));
    }

    let globals_after = joint_globals_for_test(&joints);
    let mut max_error = 0.0_f64;
    for joint in &joints {
        let before = globals_before[&joint.node_index];
        let after = globals_after[&joint.node_index];
        for row in 0..4 {
            for col in 0..4 {
                max_error = max_error.max((before[row][col] - after[row][col]).abs());
            }
        }
    }
    assert!(
        max_error < 1e-5,
        "otto reparent max global error was {max_error}"
    );
}

#[test]
fn hoist_nonaccum_rest_transform_to_wrapper_matches_legacy_shape() {
    let repo_root = crate::repository_root().parent().unwrap();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let raw_joints = read_clean_gltf_joints(&path).unwrap();
    let mut joints = legacy_safe_clean_gltf_joints(&raw_joints);
    collapse_secondary_iktarget_roots_under_primary(&mut joints);

    let nonaccum_before = joints
        .iter()
        .find(|joint| joint.parent_node_index.is_none() && joint.name == "Bip01 NonAccum")
        .unwrap()
        .clone();

    let (wrapper_translation, wrapper_rotation, wrapper_scale) =
        hoist_nonaccum_rest_transform_to_wrapper(&mut joints);

    assert_eq!(wrapper_translation, nonaccum_before.translation);
    assert_eq!(wrapper_rotation, nonaccum_before.rotation);
    assert_eq!(wrapper_scale, nonaccum_before.scale);

    let nonaccum_after = joints
        .iter()
        .find(|joint| joint.parent_node_index.is_none() && joint.name == "Bip01 NonAccum")
        .unwrap();
    assert_eq!(nonaccum_after.translation, (0.0, 0.0, 0.0));
    assert_eq!(nonaccum_after.rotation, (0.0, 0.0, 0.0, 1.0));
    assert_eq!(nonaccum_after.scale, (1.0, 1.0, 1.0));
}

#[test]
fn npc_otto_weighted_palette_excludes_nonaccum_and_iktargets() {
    let repo_root = crate::repository_root().parent().unwrap();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let joints = legacy_safe_clean_gltf_joints(&read_clean_gltf_joints(&path).unwrap());
    let mesh = fusionforge::modding::ImportedMesh::from_model_path(&path, None).unwrap();
    let weighted_joint_indices = mesh.used_joint_indices();
    let palette_joint_indices =
        clean_gltf_palette_joint_indices(&joints, &weighted_joint_indices);
    let names = palette_joint_indices
        .iter()
        .filter_map(|index| joints.get(*index))
        .map(|joint| joint.name.as_str())
        .collect::<Vec<_>>();

    assert!(!names.contains(&"Bip01 NonAccum"));
    assert!(!names.contains(&"Bip01 L Calf_IKTarget"));
    assert!(!names.contains(&"Bip01 R Calf_IKTarget"));
    assert!(!names.contains(&"Bip01 L Calf_IKTarget_001"));
    assert!(!names.contains(&"Bip01 R Calf_IKTarget_001"));
}

#[test]
fn legacy_bip01_wrapper_adds_root_curves_to_imported_clips() {
    let joints = vec![test_joint(1, "Bip01 NonAccum", None, vec![])];
    let mut clips = vec![fusionforge::modding::ImportedAnimationClip {
        name: "idle".to_string(),
        sample_rate: 30.0,
        duration: 1.0,
        translations: vec![fusionforge::modding::ImportedVec3Curve {
            path: "Bip01/Bip01 NonAccum".to_string(),
            keys: vec![fusionforge::modding::ImportedVec3Key {
                time: 0.0,
                value: (0.0, 0.0, 0.0),
            }],
        }],
        rotations: vec![fusionforge::modding::ImportedQuatCurve {
            path: "Bip01/Bip01 NonAccum".to_string(),
            keys: vec![fusionforge::modding::ImportedQuatKey {
                time: 0.0,
                value: (0.0, 0.0, 0.0, 1.0),
            }],
        }],
        scales: Vec::new(),
    }];

    assert!(clean_gltf_needs_bip01_wrapper(&joints));
    ensure_legacy_bip01_root_curves(&mut clips);

    assert!(missing_clean_gltf_animation_paths(&joints, &clips).is_empty());
    assert!(clips[0]
        .translations
        .iter()
        .any(|curve| curve.path == "Bip01"));
    assert!(clips[0].rotations.iter().any(|curve| curve.path == "Bip01"));
    assert!(clips[0].scales.iter().any(|curve| curve.path == "Bip01"));
}

#[test]
fn npc_internal_alias_entries_are_not_exported_or_saved() {
    assert!(!should_export_unity_field_path(
        "m_pNpcTable.m_pNpcStringData[730].m_strComment2"
    ));

    let mut document = json!({
        "entries": [
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pNpcTable.m_pNpcStringData[730].m_strComment2",
                "source": "Computress",
                "translation": "Компьютер"
            },
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pNpcTable.m_pNpcStringData[730].m_strName",
                "source": "Computress",
                "translation": "Компьютер"
            }
        ]
    });

    normalize_translation_document(&mut document);

    let entries = document["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0]["fieldPath"],
        json!("m_pNpcTable.m_pNpcStringData[730].m_strName")
    );
}

#[test]
fn npc_barker_entries_are_exported_and_saved() {
    assert!(should_export_unity_field_path(
        "m_pNpcTable.m_pNpcBarkerData[100].m_strComment2"
    ));

    let mut document = json!({
        "entries": [
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pNpcTable.m_pNpcBarkerData[100].m_strComment2",
                "source": "Dexlabs welcomes you to Genius Grove.",
                "translation": "Лаборатории Декстера приветствует вас в Роща гениев."
            },
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pNpcTable.m_pNpcStringData[730].m_strName",
                "source": "Computress",
                "translation": "Компьютер"
            }
        ]
    });

    normalize_translation_document(&mut document);

    let entries = document["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0]["fieldPath"],
        json!("m_pNpcTable.m_pNpcBarkerData[100].m_strComment2")
    );
    assert_eq!(
        entries[1]["fieldPath"],
        json!("m_pNpcTable.m_pNpcStringData[730].m_strName")
    );
}

#[test]
fn tabledata_display_fields_are_exported() {
    for field_path in [
        "m_pNpcTable.m_pNpcStringData[730].m_strName",
        "m_pNpcTable.m_pNpcStringData[730].m_strComment",
        "m_pNpcTable.m_pNpcStringData[730].m_strComment1",
        "m_pNameTable.m_pFirstName[1].m_pstrNameString",
        "m_pNameTable.m_pMiddleName[1].m_pstrNameString",
        "m_pNameTable.m_pLastName[1].m_pstrNameString",
        "m_pTransportationTable.m_pBroomstickString[1].m_pstrLocationName",
        "m_pTransportationTable.m_pBroomstickString[1].m_pstrLocationInfo",
        "m_pTransportationTable.m_pTransportationWarpString[1].m_pstrLocationName",
        "m_pTransportationTable.m_pTransportationWarpString[1].m_pstrLocationInfo",
        "m_pHelpTable.m_pHelpString[1].m_strName",
        "m_pHelpTable.m_pHelpString[1].m_strComment",
        "m_pHelpTable.m_pHelpPageString[1].m_strName",
        "m_pFirstUseTable.m_pFirstUseString[1].m_strName",
        "m_pFirstUseTable.m_pFirstUseString[1].m_strComment",
        "m_pRulesTable.m_pRulesString[1].m_strName",
        "m_pNpcTable.m_pNpcServiceData[1].m_strService",
    ] {
        assert!(
            should_export_unity_field_path(field_path),
            "{field_path} should be exported"
        );
    }
}
