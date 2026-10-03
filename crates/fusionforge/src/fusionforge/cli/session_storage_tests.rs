use super::*;
#[test]
fn default_sessions_are_editor_owned_without_patch_project_files() {
    let path = cli_session_dir(Path::new("test.asset")).unwrap();
    assert!(path.starts_with(crate::repository_root().to_path_buf().join("work/sessions")));
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    fs::remove_dir(path).unwrap();
}
