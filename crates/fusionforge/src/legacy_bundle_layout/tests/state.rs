use super::*;

#[test]
fn exact_runtime_names_are_kept_for_dong_and_compat_shells() {
    let cfg = cfg();
    assert!(COMPAT_BUNDLE_NAMES.contains(&"NpcTexture.resourceFile"));
    assert_eq!(
        output_part_name(&Family::Dong("DongResources_05_04".to_string()), 0, 1, &cfg),
        "DongResources_05_04.resourceFile"
    );
    assert_eq!(
        output_part_name(
            &Family::Compat("Tutorial.resourceFile".to_string()),
            0,
            1,
            &cfg
        ),
        "Tutorial.resourceFile"
    );
    assert_eq!(
        output_part_name(
            &Family::Compat("NpcTexture.resourceFile".to_string()),
            0,
            1,
            &cfg
        ),
        "NpcTexture.resourceFile"
    );
    assert_eq!(
        output_part_name(&Family::Npc, 1, 3, &cfg),
        "NPC_Pack_002.resourceFile"
    );
    assert_eq!(
        output_part_name(&Family::Hnpc, 0, 2, &cfg),
        "HNPC_Pack_001.resourceFile"
    );
}

#[test]
fn custom_npc_policy_and_complete_phase_normalization_are_runtime_safe() {
    let cfg = config(&serde_json::json!({
        "PreloadNpcBundles": false,
        "LoadNpcBundlesInWorld": true,
        "NpcBundleManifestSections": ["m_FreeZone", "m_PaidZone"],
        "BundleLayout": { "Enabled": true }
    }));
    let expected_world = BTreeSet::from(["m_FreeZone".to_string(), "m_PaidZone".to_string()]);
    assert_eq!(custom_root_sections(&Family::Npc, &cfg), expected_world);
    assert_eq!(custom_root_sections(&Family::Hnpc, &cfg), expected_world);
    assert_eq!(custom_root_sections(&Family::Icons, &cfg), expected_world);

    let complete = BTreeSet::from([
        "m_FreeZoneComplete".to_string(),
        "m_PaidZoneComplete".to_string(),
    ]);
    assert_eq!(
        runtime_sections_for_family(&Family::Npc, &complete),
        expected_world
    );
    assert_eq!(
        runtime_sections_for_family(
            &Family::Dong("DongResources_01_01".to_string()),
            &complete
        ),
        complete
    );
    assert_eq!(
        lifecycle_safe_family(Family::Nano, &expected_world),
        Family::Nano
    );

    let creation = BTreeSet::from(["m_CharacterCreation".to_string()]);
    let both_character_entries = BTreeSet::from([
        "m_CharacterCreation".to_string(),
        "m_CharacterSelection".to_string(),
    ]);
    assert_eq!(
        runtime_sections_for_root(&Family::Items, &creation),
        both_character_entries
    );
    assert_eq!(
        runtime_sections_for_root(&Family::Hnpc, &creation),
        BTreeSet::from([
            "m_CharacterCreation".to_string(),
            "m_CharacterSelection".to_string(),
        ])
    );
    assert_eq!(runtime_sections_for_root(&Family::Npc, &creation), creation);
}
