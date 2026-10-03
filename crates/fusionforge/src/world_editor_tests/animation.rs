use super::*;

#[test]
fn infer_authoring_npc_animation_set_from_template_uses_template_model_bundle() {
    let temp = native_build_temp_dir("npc_animation_set").expect("temp dir");
    let cache_dir = temp.root.join("cache");
    fs::create_dir_all(&cache_dir).expect("cache dir");
    let bundle_index_path = cache_dir.join("bundle-index.json");
    let index = ClientFileIndex {
        source_dir: temp.root.to_string_lossy().to_string(),
        project_dir: temp.root.to_string_lossy().to_string(),
        bundles: vec![ClientBundleFile {
            path: r"D:\FusionFall\Source\DongResources_00_15.resourceFile".to_string(),
            name: "DongResources_00_15.resourceFile".to_string(),
            extension: ".resourceFile".to_string(),
            size: 1,
            modified_ms: None,
            cache_dir: None,
            extracted_files: Vec::new(),
            assets: vec![ClientAssetSummary {
                name: "CustomAssetBundle-DongResources".to_string(),
                object_count: 3,
                type_counts: BTreeMap::from([
                    ("Mesh".to_string(), 1),
                    ("GameObject".to_string(), 1),
                ]),
                container_paths: vec!["mob/npc_otto.kfm".to_string()],
            }],
            errors: Vec::new(),
        }],
        unity_files: Vec::new(),
        fonts: Vec::new(),
        audio_clips: Vec::new(),
        cache_index_path: String::new(),
        translation_index_path: String::new(),
        translation_count: 0,
    };
    fs::write(
        &bundle_index_path,
        serde_json::to_string_pretty(&index).expect("serialize bundle index"),
    )
    .expect("write bundle index");

    let npc_table = json_to_unity_value(
        &json!({
            "m_pNpcData": [
                {},
                {
                    "m_iMesh": 1
                }
            ],
            "m_pNpcStringData": [
                {},
                {
                    "m_strName": "Otto"
                }
            ],
            "m_pNpcMeshData": [
                {},
                {
                    "m_pstrMMeshModelString": "npc_otto"
                }
            ],
            "m_pNpcIconData": []
        }),
        0,
    )
    .expect("npc table");
    let mut blueprint = NpcBlueprint {
        npc_id: 3463,
        template_npc_id: Some(1),
        name: "Otto".to_string(),
        internal_name: "npc_otto".to_string(),
        comment: None,
        comment1: None,
        greeting_name: None,
        greeting_comment: None,
        greeting_comment1: None,
        greeting_key: None,
        authoring_model_path: Some(r"D:\Models\otto.glb".to_string()),
        model_bundle: Some(r"D:\Models\otto.glb".to_string()),
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
    let mut warnings = Vec::new();

    infer_authoring_npc_animation_set_from_template(
        &temp.root,
        &npc_table,
        1,
        &mut blueprint,
        &mut warnings,
    );

    assert_eq!(
        blueprint.animation_set.as_deref(),
        Some(r"D:\FusionFall\Source\DongResources_00_15.resourceFile")
    );
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("Inherited legacy animationSet")));
}

#[test]
fn clean_gltf_missing_animation_paths_catches_sanitized_alias_loss() {
    let joints = vec![
        test_joint(1, "Bip01 NonAccum", None, vec![2]),
        test_joint(2, "Bip01 Head", Some(1), vec![3]),
        test_joint(3, "Bip01 Head.002", Some(2), vec![]),
        test_joint(4, "Bip01 L Calf-IKTarget", None, vec![]),
    ];
    let clips = vec![fusionforge::modding::ImportedAnimationClip {
        name: "idle".to_string(),
        sample_rate: 30.0,
        duration: 1.0,
        translations: vec![fusionforge::modding::ImportedVec3Curve {
            path: "Bip01/Bip01 L Calf-IKTarget".to_string(),
            keys: vec![fusionforge::modding::ImportedVec3Key {
                time: 0.0,
                value: (0.0, 0.0, 0.0),
            }],
        }],
        rotations: vec![fusionforge::modding::ImportedQuatCurve {
            path: "Bip01/Bip01 NonAccum/Bip01 Head/Bip01 Head.002".to_string(),
            keys: vec![fusionforge::modding::ImportedQuatKey {
                time: 0.0,
                value: (0.0, 0.0, 0.0, 1.0),
            }],
        }],
        scales: Vec::new(),
    }];

    assert!(missing_clean_gltf_animation_paths(&joints, &clips).is_empty());

    let sanitized = sanitize_clean_gltf_joints_for_runtime(&joints);
    let missing_after_sanitize = missing_clean_gltf_animation_paths(&sanitized, &clips);
    assert_eq!(
        missing_after_sanitize,
        vec![
            "Bip01/Bip01 L Calf-IKTarget".to_string(),
            "Bip01/Bip01 NonAccum/Bip01 Head/Bip01 Head.002".to_string(),
        ]
    );
}

#[test]
fn otto_imported_animation_paths_match_collapsed_runtime_joints() {
    let repo_root = crate::repository_root().parent().unwrap();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let raw_joints = read_clean_gltf_joints(&path).unwrap();
    let runtime_joints = legacy_safe_clean_gltf_joints(&raw_joints);
    let mut clips = fusionforge::modding::ImportedAnimationClip::from_gltf_path(&path, 30.0)
        .expect("otto clips");
    remap_imported_clip_paths_for_runtime(&mut clips, &raw_joints, &runtime_joints)
        .expect("remap otto clip paths");
    ensure_legacy_bip01_root_curves(&mut clips);
    let missing = missing_clean_gltf_animation_paths(&runtime_joints, &clips);
    assert!(
        missing.is_empty(),
        "otto animation paths missing from runtime joints: {missing:?}"
    );
}

#[test]
fn npc_otto_bundle_bone_indices_fit_renderer_palette() {
    let repo_root = crate::repository_root().parent().unwrap();
    let dump_path = repo_root.join("work").join("otto_after_render_dedupe.json");
    let renderer = dump_object_value(&dump_path, "SkinnedMeshRenderer");
    let mesh = dump_object_value(&dump_path, "Mesh");

    let palette_len = renderer
        .get("m_Bones")
        .and_then(fusionforge::UnityValue::as_array)
        .map(|values| values.len())
        .unwrap_or(0);
    let bone_indices = fusionforge::read_packed_bits(
        mesh.get("m_CompressedMesh")
            .and_then(|value| value.get("m_BoneIndices"))
            .expect("bone indices"),
    );
    let max_bone_index = bone_indices.iter().copied().max().unwrap_or(0) as usize;

    assert!(palette_len > 0, "otto renderer palette was empty");
    assert!(
        max_bone_index < palette_len,
        "otto bundle max bone index {max_bone_index} exceeded palette len {palette_len}"
    );
}

#[test]
fn remap_imported_clip_paths_for_runtime_rewrites_unsafe_joint_tokens() {
    let raw_joints = vec![
        test_joint(1, "Bip01 NonAccum", None, vec![2]),
        test_joint(2, "Bip01 Head.002", Some(1), vec![3]),
        test_joint(3, "Bip01 L Calf-IKTarget.001", Some(2), vec![]),
    ];
    let runtime_joints = legacy_safe_clean_gltf_joints(&raw_joints);
    let mut clips = vec![fusionforge::modding::ImportedAnimationClip {
        name: "test".to_string(),
        sample_rate: 30.0,
        duration: 1.0 / 30.0,
        translations: vec![fusionforge::modding::ImportedVec3Curve {
            path: "Bip01/Bip01 NonAccum/Bip01 Head.002/Bip01 L Calf-IKTarget.001".to_string(),
            keys: vec![],
        }],
        rotations: vec![],
        scales: vec![],
    }];

    remap_imported_clip_paths_for_runtime(&mut clips, &raw_joints, &runtime_joints)
        .expect("remap clip paths");

    assert_eq!(
        clips[0].translations[0].path,
        "Bip01/Bip01 NonAccum/Bip01 Head_002/Bip01 L Calf_IKTarget_001"
    );
}

#[test]
fn normalize_clean_gltf_npc_clip_names_renames_single_clip_to_stand1() {
    let mut clips = vec![fusionforge::modding::ImportedAnimationClip {
        name: "Bip01Action.001".to_string(),
        sample_rate: 30.0,
        duration: 1.0,
        translations: Vec::new(),
        rotations: Vec::new(),
        scales: Vec::new(),
    }];

    normalize_clean_gltf_npc_clip_names(&mut clips);

    assert_eq!(clips[0].name, "stand1");
}

#[test]
fn normalize_clean_gltf_npc_clip_names_keeps_fallback_name() {
    let mut clips = vec![fusionforge::modding::ImportedAnimationClip {
        name: "nif-default".to_string(),
        sample_rate: 30.0,
        duration: 0.0,
        translations: Vec::new(),
        rotations: Vec::new(),
        scales: Vec::new(),
    }];

    normalize_clean_gltf_npc_clip_names(&mut clips);

    assert_eq!(clips[0].name, "nif-default");
}

#[test]
fn expand_single_idle_clip_aliases_clones_stand_variants() {
    let base = fusionforge::modding::ImportedAnimationClip {
        name: "stand1".to_string(),
        sample_rate: 30.0,
        duration: 1.25,
        translations: vec![fusionforge::modding::ImportedVec3Curve {
            path: "Bip01".to_string(),
            keys: vec![
                fusionforge::modding::ImportedVec3Key {
                    time: 0.0,
                    value: (0.0, 0.0, 0.0),
                },
                fusionforge::modding::ImportedVec3Key {
                    time: 1.25,
                    value: (0.0, 0.0, 0.0),
                },
            ],
        }],
        rotations: Vec::new(),
        scales: Vec::new(),
    };
    let mut clips = vec![base.clone()];

    expand_single_idle_clip_aliases(&mut clips);

    assert_eq!(clips.len(), 3);
    assert_eq!(clips[0].name, "stand1");
    assert_eq!(clips[1].name, "stand2");
    assert_eq!(clips[2].name, "stand3");
    assert_eq!(clips[1].duration, base.duration);
    assert_eq!(clips[2].translations.len(), base.translations.len());
    assert_eq!(clips[2].translations[0].path, base.translations[0].path);
    assert_eq!(
        clips[2].translations[0].keys.len(),
        base.translations[0].keys.len()
    );
}

#[test]
fn npc_otto_imported_single_clip_expands_to_legacy_idle_aliases() {
    let repo_root = crate::repository_root().parent().unwrap();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let mut clips = fusionforge::modding::ImportedAnimationClip::from_gltf_path(&path, 30.0)
        .expect("otto clips");

    normalize_clean_gltf_npc_clip_names(&mut clips);
    expand_single_idle_clip_aliases(&mut clips);

    let names = clips
        .iter()
        .map(|clip| clip.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["stand1", "stand2", "stand3"]);
}
