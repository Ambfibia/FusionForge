use super::*;

#[test]
fn test_ser_paths_are_readable_true_names_without_hash_suffixes() {
    for spec in PARTS {
        assert!(
            spec.semantic_glb
                .starts_with("characters/player/male/test_ser/")
        );
        assert!(!spec.semantic_glb.contains("--"));
        let route_name = spec
            .route
            .rsplit('/')
            .next()
            .unwrap()
            .trim_end_matches(".nif");
        assert!(spec.semantic_glb.ends_with(&format!("/{route_name}.glb")));
    }
}

#[test]
fn coordinate_conversion_is_exact_h_conjugation_without_scale_or_centering() {
    assert_eq!(native_translation([2.0, 3.0, -4.0]), [-2.0, 3.0, -4.0]);
    assert_eq!(
        native_rotation([0.5, -0.5, 0.5, 0.5]).unwrap(),
        [0.5, 0.5, -0.5, 0.5]
    );
    let contract = exact_native_coordinate_contract();
    assert!(!contract.auto_centered);
    assert!(!contract.auto_scaled);
    assert_eq!(contract.unit_scale, "1-unity-unit-equals-1-bevy-unit");
}

#[test]
fn only_exact_unity_game_object_alias_warning_is_classified() {
    let route = "wear/m_face_001_type01.nif";
    assert!(classified_unity_alias_warning(
        route,
        "wear/m_face_001_type01.nif: could not extract NIF bytes"
    ));
    assert!(classified_unity_alias_warning(
        route,
        "wear/m_face_001_type01.nif: NIF parse failed: binrw error"
    ));
    assert!(!classified_unity_alias_warning(
        route,
        "wear/m_face_001_type01.nif: skinning missing"
    ));
    assert!(!classified_unity_alias_warning(
        route,
        "wear/other.nif: could not extract NIF bytes"
    ));
}

#[test]
fn selection_binds_creation_row_one_equipment() {
    let selection = test_ser_selection();
    assert_eq!(selection.gender, 1);
    assert_eq!(selection.face_style, 5);
    assert_eq!(selection.hair_style, 23);
    assert_eq!(
        selection
            .equipment
            .iter()
            .map(|item| (item.item_type, item.item_id))
            .collect::<Vec<_>>(),
        [(0, 1), (1, 30), (2, 30), (3, 30)]
    );
}
