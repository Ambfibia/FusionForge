use super::*;

#[test]
fn classifier_uses_public_path_before_mixed_source_bundle_name() {
    let cfg = cfg();
    assert_eq!(
        classify_root("texture/npc_bloo.dds", "NpcTexture.resourceFile", &cfg),
        Family::Npc
    );
    assert_eq!(
        classify_root(
            "texture/nano_buttercup.dds",
            "FutureNano.resourceFile",
            &cfg
        ),
        Family::Nano
    );
    assert_eq!(
        classify_root(
            "texture/hat_chef.dds",
            "Retro_shared_part2.resourceFile",
            &cfg
        ),
        Family::Items
    );
    // Gender/head/face naming alone is not ownership. These routes are shared by
    // playable characters and HNPC wear, so TableData/graph semantics must decide.
    assert_eq!(
        classify_root("texture/f_emo_face_01.dds", "NpcTexture.resourceFile", &cfg),
        Family::Auto
    );
    assert_eq!(
        classify_root("texture/m_unknown.dds", "CharTexture.resourceFile", &cfg),
        Family::Auto
    );
    assert_eq!(
        classify_root("texture/head_hero.dds", "CharTexture.resourceFile", &cfg),
        Family::Items
    );
    assert_eq!(
        classify_root("texture/face_hero.dds", "CharTexture.resourceFile", &cfg),
        Family::Items
    );
    assert_eq!(
        classify_root(
            "texture/m_shirt_plumber_npc.dds",
            "NpcTexture.resourceFile",
            &cfg
        ),
        Family::Items
    );
    assert_eq!(
        classify_root("texture/f_unknown.dds", "CharTexture.resourceFile", &cfg),
        Family::Auto
    );
    assert_eq!(
        classify_root(
            "texture/road_sign.dds",
            "DongResources_05_04.resourceFile",
            &cfg
        ),
        Family::Dong("DongResources_05_04".to_string())
    );
    assert_eq!(
        classify_root(
            "texture/ex_cloud.dds",
            "World_shared_part3.resourceFile",
            &cfg
        ),
        Family::WorldShared
    );
}

#[test]
fn unreadable_legacy_objects_fail_closed_when_any_readable_path_references_them() {
    let unreadable = BTreeMap::from([
        ((7, 37), "broken material".to_string()),
        ((7, 38), "broken texture".to_string()),
    ]);
    assert!(referenced_unreadable_errors(&unreadable, &BTreeSet::new()).is_empty());
    assert_eq!(
        referenced_unreadable_errors(&unreadable, &BTreeSet::from([(7, 38)])),
        vec!["broken texture".to_string()]
    );
}

#[test]
fn route_rank_matches_literal_resource_locator_branches() {
    assert!(
        legacy_route_rank(
            "Prefabs/Particle/EffectScripts/es659.prefab",
            "Effects.resourceFile"
        ) < legacy_route_rank(
            "Prefabs/Particle/EffectScripts/es659.prefab",
            "Freearea_shared.resourceFile"
        )
    );
    assert!(
        legacy_route_rank("texture/shared.dds", "CharTexture.resourceFile")
            < legacy_route_rank("texture/shared.dds", "NpcTexture.resourceFile")
    );
    assert!(
        legacy_route_rank("nano/nano_foo.kfm", "FutureNano.resourceFile")
            < legacy_route_rank("nano/nano_foo.kfm", "Nano.resourceFile")
    );
}

#[test]
fn synthetic_root_estimate_includes_utf8_path_pointers_and_preserved_refs() {
    let first_ref = AssetRef {
        asset_path: "archive:/external.asset".to_string(),
        guid: [1; 16],
        type_id: 2,
        file_path: "external.asset".to_string(),
    };
    let second_ref = AssetRef {
        asset_path: String::new(),
        guid: [2; 16],
        type_id: 3,
        file_path: "system/library.asset".to_string(),
    };
    let root = Root {
        path: "texture/лягушка.dds".to_string(),
        normalized_path: "texture/лягушка.dds".to_string(),
        target: (0, 10),
        // The duplicate target must not be charged as another output preload pointer.
        preload_roots: vec![(0, 10), (0, 20), (0, 20)],
        preserved_preloads: vec![(first_ref.clone(), 7), (second_ref.clone(), 8)],
        source_asset: 0,
        source_assetbundle: 1,
        source_entry: UnityValue::Object(BTreeMap::new()),
        family: Family::Core,
        semantic_owner: SemanticOwnerHint::None,
        sections: BTreeSet::new(),
        output_part: None,
    };
    let path_bytes = ((4 + root.path.as_bytes().len() as u64) + 3) & !3;
    let ref_bytes = |reference: &AssetRef| {
        (((4 + reference.asset_path.as_bytes().len() as u64) + 3) & !3)
            + 16
            + 4
            + (((4 + reference.file_path.as_bytes().len() as u64) + 3) & !3)
    };
    // 1 container target pointer + 2 unique target/preload roots + 2 preserved preloads.
    let expected =
        path_bytes + 32 + 16 * (1 + 2 + 2) + ref_bytes(&first_ref) + ref_bytes(&second_ref);
    assert_eq!(synthetic_root_estimated_bytes(&root), expected);
    assert!(root.path.len() > root.path.chars().count());
}
