use super::*;

#[test]
fn h_mirror_preserves_units_origin_magnitude_and_scale() {
    let unity_origin = (7.25, -3.0, 512.0);
    let unity_scale = (0.75, 1.5, 2.25);

    assert_eq!(unity_to_native_vec3(unity_origin), (-7.25, -3.0, 512.0));
    assert_eq!(unity_to_native_scale(unity_scale), unity_scale);
    assert_eq!(
        unity_origin.0 * unity_origin.0
            + unity_origin.1 * unity_origin.1
            + unity_origin.2 * unity_origin.2,
        {
            let native = unity_to_native_vec3(unity_origin);
            native.0 * native.0 + native.1 * native.1 + native.2 * native.2
        }
    );
}

#[test]
fn world_contract_explicitly_forbids_facing_rotation_recenter_and_rescale() {
    let contract = native_coordinate_contract_json();
    assert_eq!(contract["basis"], json!("H=diag(-1,1,1)"));
    assert_eq!(contract["gameplayFacingRotationApplied"], json!(false));
    assert_eq!(
        contract["originPolicy"],
        json!("source-trs-unchanged-no-auto-centering")
    );
    assert_eq!(
        contract["unitScale"],
        json!("1-unity-unit-equals-1-bevy-unit")
    );
}
