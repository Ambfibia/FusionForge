use super::*;

#[test]
fn conflicting_duplicate_curve_preserves_serialized_last_write_resolution() {
    let (clip, hierarchy) = duplicate_translation_regression_fixture(true);
    assert!(clip.get("decodeWarnings").is_none());
    let canonical = &clip["animationData"]["translations"][0];
    assert_eq!(canonical["sourceIndex"], json!(1));
    let displaced = &clip["animationData"]["duplicateTrsBindings"][0];
    assert_eq!(displaced["sourceIndex"], json!(0));
    assert_eq!(displaced["relation"], json!("serializedLastWriteWins"));
    assert_eq!(displaced["canonicalSourceIndex"], json!(1));
    assert_ne!(displaced["keys"], canonical["keys"]);
    assert_eq!(
        displaced["resolutionProof"]["rule"],
        json!("later-serialized-binding-overwrites-earlier")
    );
    validate_exact_animation_source(&hierarchy, &[clip])
        .expect("serialized overwrite remains lossless and source ordered");
}
