use super::*;

#[test]
fn apply_blueprint_npc_assets_preserves_icon_type_and_uses_unique_icon_number() {
    let mut target_table = json_to_unity_value(
        &json!({
            "m_pNpcData": [
                {
                    "m_iTeam": 1,
                    "m_iMesh": 0,
                    "m_iIcon1": 0
                }
            ],
            "m_pNpcMeshData": [
                {}
            ],
            "m_pNpcIconData": [
                {
                    "m_iIconType": 4,
                    "m_iIconNumber": 0
                }
            ]
        }),
        0,
    )
    .expect("target table");
    let blueprint = NpcBlueprint {
        npc_id: 3463,
        template_npc_id: None,
        name: "Otto".to_string(),
        internal_name: "npc_otto".to_string(),
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
        icon_bundle: Some(r"D:\FusionFall\Source\Icons.resourceFile".to_string()),
        icon_asset: Some("icons/mobicon_141.png".to_string()),
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

    let changed = apply_blueprint_npc_assets(&mut target_table, 0, &blueprint).expect("apply");
    assert!(changed);
    let icon_row = npc_table_array(&target_table, "m_pNpcIconData")
        .first()
        .expect("icon row");
    assert_eq!(
        icon_row
            .get("m_iIconType")
            .and_then(fusionforge::UnityValue::as_i64),
        Some(8)
    );
    assert_eq!(
        icon_row
            .get("m_iIconNumber")
            .and_then(fusionforge::UnityValue::as_i64),
        Some(3463)
    );
}
