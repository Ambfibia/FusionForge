use super::*;

#[test]
fn split_parts_keeps_character_bundles_atomic_and_bounded() {
    let make = |name: &str, size: u64| SourceBundle {
        path: PathBuf::from(name),
        name: name.to_string(),
        size,
    };
    let parts = split_parts(
        vec![make("a", 7), make("b", 6), make("c", 4), make("d", 3)],
        10,
        12,
    );
    assert_eq!(parts.len(), 2);
    assert!(parts
        .iter()
        .all(|part| part.iter().map(|bundle| bundle.size).sum::<u64>() <= 10));
    assert_eq!(parts.iter().map(Vec::len).sum::<usize>(), 4);
}

#[test]
fn character_bundle_filter_does_not_capture_legacy_global_bundles() {
    assert!(is_standalone_character_bundle("Character_Rex.resourceFile"));
    assert!(!is_standalone_character_bundle(
        "CharacterCreation.resourceFile"
    ));
    assert!(!is_standalone_character_bundle(
        "CharacterSelection.resourceFile"
    ));
}
