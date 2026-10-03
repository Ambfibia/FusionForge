use super::*;

#[test]
fn map_00_01_root_origin_golden_uses_shared_h_contract_without_facing_rotation() {
    // Serialized Map_00_01 root recovered by the native inspector.
    let unity_root = (0.0, 0.0, 512.0);
    let native_root = unity_to_native_vec3(unity_root);
    assert_eq!(native_root, (0.0, 0.0, 512.0));
    assert_eq!(
        native_coordinate_contract_json()["gameplayFacingRotationApplied"],
        serde_json::json!(false)
    );
    assert_eq!(
        native_coordinate_contract_json()["originPolicy"],
        serde_json::json!("source-trs-unchanged-no-auto-centering")
    );
}

#[test]
fn scale_classification_distinguishes_nonuniform_negative_and_singular_values() {
    assert!(approx_eq(1.0, 1.0, APPROX_EPSILON));
    let nonuniform = [0.5_f64, 1.5, 2.5];
    assert!(!approx_eq(nonuniform[0], nonuniform[1], APPROX_EPSILON));
    assert!(nonuniform.iter().all(|value| *value > 0.0));
    let reflected = [-1.0_f64, 1.0, 1.0];
    assert!(reflected.iter().any(|value| *value < 0.0));
    assert!(reflected.iter().product::<f64>().abs() > SINGULAR_EPSILON);
    let singular = [1.0_f64, 0.0, 1.0];
    assert!(singular.iter().product::<f64>().abs() <= SINGULAR_EPSILON);
}

#[test]
fn collider_contract_mirrors_center_but_never_size_or_scalar_dimensions() {
    let center = unity_to_native_vec3((2.0, -3.0, 4.0));
    let size = unity_to_native_scale((5.0, 6.0, 7.0));
    assert_eq!(center, (-2.0, -3.0, 4.0));
    assert_eq!(size, (5.0, 6.0, 7.0));
    let contract = ColliderCoordinateContract::default();
    assert_eq!(contract.size, "unchanged");
    assert_eq!(contract.scalar_dimensions, "radius-and-height-unchanged");
    assert!(!contract.gameplay_facing_rotation_applied);
}

#[test]
fn scene_asset_matching_is_exact_and_does_not_mix_neighbor_tiles() {
    assert!(scene_asset_belongs_to_map(
        "BuildPlayer-Map_00_01#0.assets",
        "Map_00_01"
    ));
    assert!(!scene_asset_belongs_to_map(
        "BuildPlayer-Map_00_010#0.assets",
        "Map_00_01"
    ));
    assert!(!scene_asset_belongs_to_map(
        "BuildPlayer-Map_00_02#0.assets",
        "Map_00_01"
    ));
}

#[test]
fn dependency_candidate_proof_prefers_exact_bundle_stem_and_keeps_ambiguity() {
    let direct = PathBuf::from("C:/build/Effects.resourceFile");
    let incidental = PathBuf::from("C:/build/Other.resourceFile");
    let catalog = HashMap::from([(
        normalize_bundle_name("Effects"),
        vec![incidental.clone(), direct.clone()],
    )]);
    assert_eq!(
        effective_archive_candidates("Effects", &catalog),
        vec![direct]
    );

    let first = PathBuf::from("C:/build/One.resourceFile");
    let second = PathBuf::from("C:/build/Two.resourceFile");
    let custom = normalize_bundle_name("CustomAssetBundle-deadbeef");
    let catalog = HashMap::from([(custom.clone(), vec![second.clone(), first.clone()])]);
    assert_eq!(
        effective_archive_candidates(&custom, &catalog),
        vec![first, second]
    );
}

#[test]
fn map_tile_origin_contract_uses_signed_decimal_and_h_mirror() {
    assert_eq!(
        expected_native_tile_horizontal_origin("Map_00_01"),
        Some([0.0, 512.0])
    );
    assert_eq!(
        expected_native_tile_horizontal_origin("Map_05_04"),
        Some([-2560.0, 2048.0])
    );
    assert_eq!(
        expected_native_tile_horizontal_origin("Map_01_11"),
        Some([-512.0, 5632.0])
    );
    assert_eq!(
        expected_native_tile_horizontal_origin("Map_-01_10"),
        Some([512.0, 5120.0])
    );
    assert_eq!(
        expected_native_tile_horizontal_origin("DongResources_00_01"),
        None
    );
}
