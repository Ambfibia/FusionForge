use super::*;

#[test]
fn legacy_audio_catalog_requires_a_verified_runtime_registry() {
    let (_temp, project_root) = fixture();
    add_fixture_asset(
        &project_root,
        "audio/catalog.json",
        br#"{"schema":"legacy-audio"}"#,
    );

    let blocked = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap();
    assert!(!blocked.apply_ready);
    assert!(
        blocked
            .blockers
            .iter()
            .any(|blocker| blocker.contains(AUDIO_RUNTIME_REGISTRY))
    );

    add_fixture_asset(
        &project_root,
        AUDIO_RUNTIME_REGISTRY,
        br#"{"schema":"runtime-audio"}"#,
    );
    let ready = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap();
    assert!(ready.apply_ready);
    assert!(
        ready
            .archived
            .iter()
            .any(|entry| entry.source_path == "audio/catalog.json")
    );
}
