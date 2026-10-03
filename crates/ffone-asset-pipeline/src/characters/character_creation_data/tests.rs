use super::*;

#[test]
fn strips_only_native_collision_suffix() {
    assert_eq!(
        strip_collision_suffix("cosicon_195--38f5bae0d58146fc"),
        "cosicon_195"
    );
    assert_eq!(
        strip_collision_suffix("model--not-a-hash"),
        "model--not-a-hash"
    );
}

#[test]
fn legacy_true_name_normalizes_route_and_null() {
    assert_eq!(true_name(r"Wear\m_shirt_light.nif"), "m_shirt_light");
    assert_eq!(true_name("null"), "");
    assert_eq!(true_name(""), "");
}

#[test]
fn retobution_historical_mesh_alias_is_explicit() {
    assert_eq!(
        model_alias(AvatarItemCategory::Back, "back_angelwing"),
        Some("back_angelswing")
    );
}

#[test]
fn pink_lady_texture_aliases_are_exact_and_not_fuzzy() {
    assert_eq!(
        texture_alias("f_shoes_pink lady_a"),
        Some("f_shoes_pink_lady_a")
    );
    assert_eq!(
        texture_alias("f_shoes_pink lady_b"),
        Some("f_shoes_pink_lady_b")
    );
    assert_eq!(texture_alias("f_shoes_pinklady_a"), None);
    assert_eq!(texture_alias("f_shoes_pink lady_a_"), None);
    assert_eq!(texture_alias("pink lady_a"), None);
}

#[test]
fn openfusion_0104_constraints_are_exact() {
    let constraints = CharacterAppearanceConstraints {
        gender_codes: vec![1, 2],
        body_codes: (0..=2).collect(),
        height_codes: (0..=4).collect(),
        skin_color_codes: (1..=12).collect(),
        hair_color_codes: (1..=18).collect(),
        eye_color_codes: (1..=5).collect(),
    };
    assert_eq!(constraints.gender_codes, [1, 2]);
    assert_eq!(constraints.body_codes, [0, 1, 2]);
    assert_eq!(constraints.height_codes, [0, 1, 2, 3, 4]);
    assert_eq!(constraints.skin_color_codes.len(), 12);
    assert_eq!(constraints.hair_color_codes.len(), 18);
    assert_eq!(constraints.eye_color_codes, [1, 2, 3, 4, 5]);
}

#[test]
fn primary_equipment_texture_evidence_is_exact_and_keeps_screenshot_sentinels() {
    let document: EquipmentTextureSourceMetadataDocument =
        serde_json::from_str(EQUIPMENT_TEXTURE_SOURCE_METADATA_JSON)
            .expect("parse checked player-equipment texture evidence");
    validate_equipment_texture_source_metadata(&document)
        .expect("validate checked player-equipment texture evidence");
    assert_eq!(
        document
            .entries
            .iter()
            .filter(|entry| entry.route_repair.is_some())
            .count(),
        7
    );
    assert_eq!(
        document
            .entries
            .iter()
            .filter(|entry| entry.true_name_repair.is_some())
            .count(),
        130
    );
    for (true_name, path_id, native_path) in [
        (
            "m_halmet_football",
            437,
            "characters/player/items/hat/helmat_football/textures/m_halmet_football.png",
        ),
        (
            "back_octibackpack",
            941,
            "characters/player/items/back/back_octibackpack/textures/back_octibackpack.png",
        ),
        (
            "bazooka_toybazooka",
            150,
            "characters/player/items/weapon/bazooka_toybazooka/textures/bazooka_toybazooka.png",
        ),
    ] {
        let entry = document
            .entries
            .iter()
            .find(|entry| entry.true_name == true_name)
            .unwrap_or_else(|| panic!("missing equipment sentinel {true_name}"));
        assert_eq!(entry.texture.path_id, path_id);
        assert_eq!(entry.native_asset.path, native_path);
        assert_eq!(entry.source.source_alias, "primary");
    }
}
