use super::*;

#[test]
fn stale_transaction_artifact_fails_before_any_write() {
    let (_temp, project_root) = fixture();
    let stale = project_root.join("assets/game/.clean-runtime-metadata-backup-stale");
    fs::create_dir(&stale).unwrap();
    let error = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap_err()
    .to_string();
    assert!(error.contains("stale clean-runtime-metadata transaction artifact"));
    assert!(
        project_root
            .join("assets/game/icons/catalog.json")
            .is_file()
    );
}

#[test]
fn isolated_stale_revision_stage_fails_before_any_write() {
    let (_temp, project_root) = fixture();
    let archive_parent =
        project_root.join("../FusionForge/work/ffone/migration-archive/retrobution-test");
    fs::create_dir_all(&archive_parent).unwrap();
    let stale = archive_parent.join(".conversion-metadata-revision-stage-stale");
    fs::create_dir(&stale).unwrap();
    let error = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap_err()
    .to_string();
    assert!(error.contains("stale clean-runtime-metadata transaction artifact"));
    assert!(
        project_root
            .join("assets/game/icons/catalog.json")
            .is_file()
    );
    assert!(stale.is_dir());
}
