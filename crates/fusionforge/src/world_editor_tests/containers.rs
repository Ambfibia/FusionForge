use super::*;

#[test]
fn staged_preview_collects_sibling_source_bundle_directories() {
    let temp = native_build_temp_dir("npc_preview_siblings").expect("temp dir");
    let project = temp.path();
    let assets = project.join("npcs/3462__Fusion_Finn/assets");
    let model_dir = assets.join("TrainingGrounds");
    let texture_dir = assets.join("Character_Texture_npc2");
    let icon_dir = assets.join("Icons");
    fs::create_dir_all(&model_dir).expect("model dir");
    fs::create_dir_all(&texture_dir).expect("texture dir");
    fs::create_dir_all(&icon_dir).expect("icon dir");
    fs::write(model_dir.join("TrainingGrounds"), b"model").expect("model asset");
    fs::write(texture_dir.join("Character_Texture_npc2"), b"texture").expect("texture asset");
    fs::write(icon_dir.join("Icons"), b"icons").expect("icon asset");
    fs::write(
        assets.join("_manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "sourceBundles": [
                "npcs/3462__Fusion_Finn/assets/TrainingGrounds",
                "npcs/3462__Fusion_Finn/assets/Character_Texture_npc2"
            ],
            "assetDirs": [{
                "path": "npcs/3462__Fusion_Finn/assets/Icons",
                "files": ["npcs/3462__Fusion_Finn/assets/Icons/Icons"]
            }]
        }))
        .expect("manifest json"),
    )
    .expect("manifest");

    let dirs = staged_preview_sibling_asset_dirs(project, &model_dir)
        .into_iter()
        .collect::<BTreeSet<_>>();

    assert!(dirs.contains(&model_dir));
    assert!(dirs.contains(&texture_dir));
    assert!(dirs.contains(&icon_dir));
}

#[test]
fn bundle_plan_keeps_existing_tile_resource_and_stages_imports() {
    let document = json!({
        "schemaVersion": 2,
        "tileId": "Map_05_04",
        "sourceMapBundle": "Map_05_04.unity3d",
        "sourceResourceBundle": "DongResources_05_04.resourceFile",
        "objects": [
            {
                "name": "Existing crate",
                "source": {
                    "kind": "model",
                    "bundle": "DongResources_05_04.resourceFile",
                    "uri": "props/crate"
                }
            },
            {
                "name": "Imported statue",
                "source": {
                    "kind": "externalModel",
                    "uri": "assets/models/statue.glb"
                }
            }
        ]
    });

    let plan = analyze_world_bundle_plan_value(&document);
    let decisions = plan["decisions"].as_array().expect("decisions");

    assert!(decisions.iter().any(|decision| {
        decision["asset"] == json!("Existing crate")
            && decision["targetBundleClass"] == json!("tileResource")
            && decision["action"] == json!("referenceExisting")
    }));
    assert!(decisions.iter().any(|decision| {
        decision["asset"] == json!("Imported statue")
            && decision["targetBundleClass"] == json!("newBundle")
            && decision["action"] == json!("stageTemplateImport")
    }));
}

#[test]
fn infer_authoring_npc_icon_from_template_uses_template_icon_bundle() {
    let temp = native_build_temp_dir("npc_icon_bundle").expect("temp dir");
    let cache_dir = temp.root.join("cache");
    fs::create_dir_all(&cache_dir).expect("cache dir");
    let bundle_index_path = cache_dir.join("bundle-index.json");
    let index = ClientFileIndex {
        source_dir: temp.root.to_string_lossy().to_string(),
        project_dir: temp.root.to_string_lossy().to_string(),
        bundles: vec![ClientBundleFile {
            path: r"D:\FusionFall\Source\Icons.resourceFile".to_string(),
            name: "Icons.resourceFile".to_string(),
            extension: ".resourceFile".to_string(),
            size: 1,
            modified_ms: None,
            cache_dir: None,
            extracted_files: Vec::new(),
            assets: vec![ClientAssetSummary {
                name: "Icons".to_string(),
                object_count: 3,
                type_counts: BTreeMap::from([
                    ("Texture2D".to_string(), 1),
                    ("MeshFilter".to_string(), 1),
                    ("GameObject".to_string(), 1),
                ]),
                container_paths: vec!["icons/npcicon_141.png".to_string()],
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
                    "m_iIcon1": 1
                }
            ],
            "m_pNpcIconData": [
                {},
                {
                    "m_iIconType": 4,
                    "m_iIconNumber": 141
                }
            ]
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

    infer_authoring_npc_icon_from_template(
        &temp.root,
        &npc_table,
        1,
        &mut blueprint,
        &mut warnings,
    );

    assert_eq!(
        blueprint.icon_asset.as_deref(),
        Some("icons/npcicon_141.png")
    );
    assert_eq!(
        blueprint.icon_bundle.as_deref(),
        Some(r"D:\FusionFall\Source\Icons.resourceFile")
    );
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("Inherited legacy icon")));
}

pub(super) fn dump_object_value(dump_path: &Path, object_type: &str) -> fusionforge::UnityValue {
    let dump =
        serde_json::from_str::<JsonValue>(&fs::read_to_string(dump_path).expect("read dump"))
            .expect("parse dump");
    let item = dump
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("type").and_then(JsonValue::as_str) == Some(object_type))
        })
        .unwrap_or_else(|| panic!("missing {object_type} in {}", dump_path.display()));
    json_to_unity_value(item.get("value").expect("dump value"), 0).expect("unity dump value")
}
