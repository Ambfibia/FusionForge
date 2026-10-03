use super::*;

#[test]
fn attach_staged_generated_icon_updates_manifest_and_tabledata_patch_atomically() {
    let temp = native_build_temp_dir("npc_generated_icon_attach").expect("temp dir");
    let project = &temp.root;
    let npc_dir = project.join("npcs").join("1__Test");
    let generated_dir = npc_dir.join("generated");
    let tabledata_dir = project.join("tabledata");
    fs::create_dir_all(&generated_dir).expect("generated icon dir");
    fs::create_dir_all(&tabledata_dir).expect("tabledata dir");

    let relative_png = "npcs/1__Test/generated/npcicon_01.png";
    image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 0]))
        .save(project.join(relative_png))
        .expect("write transparent icon PNG");

    let blueprint = NpcBlueprint {
        npc_id: 1,
        template_npc_id: Some(2),
        name: "Test".to_string(),
        internal_name: "npc_test".to_string(),
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
        generated_icon: None,
        profile: BTreeMap::new(),
        spawn_map: None,
        spawn_position: None,
        spawn_angle: None,
        spawn_json_id: None,
        notes: None,
    };
    let patch_path = tabledata_dir.join("1__Test.json");
    let patch = json!({
        "format": "fftools.tabledata-object.v1",
        "value": {
            "m_pNpcTable": {
                "m_pNpcData": [
                    {},
                    { "m_iTeam": 1, "m_iIcon1": 1 },
                    { "m_iTeam": 1, "m_iIcon1": 1 }
                ],
                "m_pNpcIconData": [
                    {},
                    { "m_iIconType": 4, "m_iIconNumber": 70 }
                ]
            }
        }
    });
    fs::write(
        &patch_path,
        serde_json::to_string_pretty(&patch).expect("serialize patch"),
    )
    .expect("write patch");

    let manifest_path = npc_dir.join("1__Test.npc-import.json");
    let manifest = json!({
        "format": "fftools.npc-import.v1",
        "blueprint": blueprint,
        "resources": {
            "iconTexture": {
                "bundle": "Icons.resourceFile",
                "asset": "icons/npcicon_70.png"
            }
        },
        "sourceBundles": ["Icons.resourceFile"],
        "tableDataPatch": {
            "file": "tabledata/1__Test.json",
            "copiedSections": ["m_pNpcData"]
        }
    });
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");

    let result = attach_staged_npc_generated_icon(
        project.to_string_lossy().to_string(),
        1,
        NpcGeneratedIcon {
            file: relative_png.to_string(),
            asset_path: "icons/npcicon_01.png".to_string(),
            template_asset_path: "icons/npcicon_70.png".to_string(),
            size: 64,
            camera: None,
        },
        None,
    )
    .expect("attach staged generated icon");
    assert_eq!(result.icon_type, 4);
    assert_eq!(result.icon_number, 1);
    assert_eq!(result.icon_row_index, 2);
    assert!(result.cloned_shared_row);
    assert_eq!(result.generated_icon.asset_path, "icons/npcicon_01.png");

    let updated_manifest: JsonValue = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("read updated manifest"),
    )
    .expect("parse updated manifest");
    assert_eq!(
        updated_manifest.pointer("/blueprint/iconAsset"),
        Some(&json!("icons/npcicon_01.png"))
    );
    assert_eq!(
        updated_manifest.pointer("/blueprint/generatedIcon/file"),
        Some(&json!(relative_png))
    );
    assert_eq!(
        updated_manifest.pointer("/resources/iconTexture/generated/size"),
        Some(&json!(64))
    );
    assert!(updated_manifest
        .pointer("/tableDataPatch/copiedSections")
        .and_then(JsonValue::as_array)
        .is_some_and(|sections| sections.contains(&json!("m_pNpcIconData"))));

    let updated_patch: JsonValue =
        serde_json::from_str(&fs::read_to_string(&patch_path).expect("read updated patch"))
            .expect("parse updated patch");
    let updated_body = json_to_unity_value(&updated_patch["value"], 0).expect("patch body");
    let updated_table = npc_table_from_body(&updated_body).expect("updated npc table");
    assert_eq!(
        unity_usize_field(&npc_table_array(updated_table, "m_pNpcData")[1], "m_iIcon1"),
        Some(2)
    );
    assert_eq!(
        unity_usize_field(&npc_table_array(updated_table, "m_pNpcData")[2], "m_iIcon1"),
        Some(1)
    );
    assert_eq!(
        npc_icon_row_type_and_number(&npc_table_array(updated_table, "m_pNpcIconData")[1]),
        Some((4, 70))
    );
    assert_eq!(
        npc_icon_row_type_and_number(&npc_table_array(updated_table, "m_pNpcIconData")[2]),
        Some((4, 1))
    );
}
