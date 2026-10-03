use super::*;

#[test]
fn rejects_paths_that_can_escape_the_semantic_tree() {
    assert!(validate_relative("hat/hat_hero/hat_hero.glb").is_ok());
    assert!(validate_relative("../hat_hero.glb").is_err());
    assert!(validate_relative("hat\\hat_hero.glb").is_err());
    assert!(validate_relative("/hat/hat_hero.glb").is_err());
}

#[test]
fn maps_only_native_runtime_extensions() {
    assert_eq!(kind_for("part.glb").unwrap(), ProjectAssetKind::Model);
    assert_eq!(kind_for("part.png").unwrap(), ProjectAssetKind::Texture);
    assert_eq!(
        kind_for("part.publish.json").unwrap(),
        ProjectAssetKind::Data
    );
    assert!(kind_for("part.unity3d").is_err());
}
