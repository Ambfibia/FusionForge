use super::*;

#[test]
fn exact_animation_validation_rejects_empty_sampleable_target_collision() {
    let (mut clip, hierarchy) = simon_empty_binding_regression_fixture();
    clip["animationData"]["emptyTrsBindings"][0]["path"] = json!("Bip01/J1");

    let error = validate_exact_animation_source(&hierarchy, &[clip]).unwrap_err();
    assert!(
        error.contains("both empty and sampleable translation bindings"),
        "{error}"
    );
}
