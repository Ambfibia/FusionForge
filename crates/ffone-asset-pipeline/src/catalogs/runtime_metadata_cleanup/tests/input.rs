use super::*;

#[test]
fn dry_run_is_read_only_and_reports_world_blocker_separately() {
    let (_temp, project_root) = fixture();
    let options = CleanRuntimeMetadataOptions::new(&project_root, "retrobution-test");
    let report = clean_runtime_metadata(&options).unwrap();
    assert_eq!(report.mode, CleanRuntimeMetadataMode::DryRun);
    assert!(report.apply_ready);
    assert_eq!(report.counts.archived_files, 6);
    assert_eq!(report.counts.removed_manifest_entries, 6);
    assert_eq!(report.counts.runtime_registry_files, 0);
    assert_eq!(report.counts.deferred_world_files, 1);
    assert!(report.world_migration_required);
    assert!(
        project_root
            .join("assets/game/icons/catalog.json")
            .is_file()
    );
    assert!(
        !project_root
            .join("../FusionForge/work/ffone/migration-archive/retrobution-test/conversion-metadata")
            .exists()
    );
}
