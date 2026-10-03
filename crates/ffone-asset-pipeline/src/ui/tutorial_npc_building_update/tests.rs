use super::*;
use tempfile::tempdir;

#[test]
fn transactional_pair_rolls_back_both_files_after_glb_failure() {
    let temp = tempdir().unwrap();
    let installed = temp.path().join("installed.glb");
    let manifest = temp.path().join("manifest.json");
    let staged = temp.path().join("next.glb");
    let staged_manifest = temp.path().join("next.json");
    let backup = temp.path().join("backup.glb");
    let backup_manifest = temp.path().join("backup.json");
    fs::write(&installed, b"old-glb").unwrap();
    fs::write(&manifest, b"old-manifest").unwrap();
    fs::write(&staged, b"new-glb").unwrap();
    fs::write(&staged_manifest, b"new-manifest").unwrap();

    let error = commit_pair(
        &installed,
        &manifest,
        &staged,
        &staged_manifest,
        &backup,
        &backup_manifest,
        b"new-glb",
        b"new-manifest",
        true,
    )
    .unwrap_err();
    assert!(error.to_string().contains("injected failure"));
    assert_eq!(fs::read(installed).unwrap(), b"old-glb");
    assert_eq!(fs::read(manifest).unwrap(), b"old-manifest");
}

#[test]
fn stale_transaction_is_fail_closed() {
    let temp = tempdir().unwrap();
    fs::create_dir(temp.path().join(format!("{TRANSACTION_PREFIX}stale"))).unwrap();
    assert!(reject_stale_transactions(temp.path()).is_err());
}
