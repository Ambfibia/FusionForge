use super::*;
#[test]
fn check_runs_conflicts_and_leaves_missing_output_root_absent() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("native");
    let files = vec![("ui/menu.json".into(), b"native".to_vec())];
    check(&root, &files).unwrap();
    assert!(!root.exists());
    install(&root, &files).unwrap();
    check(&root, &files).unwrap();
    assert!(check(&root, &[("ui/menu.json".into(), b"different".to_vec())]).is_err());
    check_with_permission(
        &root,
        &[("ui/menu.json".into(), b"different".to_vec())],
        true,
    )
    .unwrap();
    assert_eq!(fs::read(root.join("ui/menu.json")).unwrap(), b"native");
}
#[test]
fn conflicts_fail_before_any_output_and_replay_is_identical() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("keep"), b"user").unwrap();
    let files = vec![
        ("new".into(), b"native".to_vec()),
        ("keep".into(), b"converted".to_vec()),
    ];
    assert!(install(temp.path(), &files).is_err());
    assert!(!temp.path().join("new").exists());
    assert_eq!(fs::read(temp.path().join("keep")).unwrap(), b"user");
    install(temp.path(), &files[..1]).unwrap();
    install(temp.path(), &files[..1]).unwrap();
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
    for bad in ["../escape", "file:stream", "CON.txt", "trailing."] {
        assert!(
            install(temp.path(), &[(bad.into(), vec![])]).is_err(),
            "{bad}"
        );
    }
}
