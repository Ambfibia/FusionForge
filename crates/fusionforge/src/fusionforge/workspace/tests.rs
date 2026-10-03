use super::*;

#[test]
fn conversion_cases_are_fresh_and_editor_owned() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("work/cases");
    assert!(create_fresh_case(&root, &temp.path().join("native-client")).is_err());
    assert!(!temp.path().join("native-client").exists());
    assert!(create_fresh_case(&root, &root).is_err());
    assert!(create_fresh_case(&root, &root.join(".ffclienteditor/case")).is_err());
    assert!(create_fresh_case(&root, &root.join("../outside")).is_err());
    let case = create_fresh_case(&root, &root.join("example")).unwrap();
    std::fs::write(case.join("source.json"), "keep").unwrap();
    assert!(create_fresh_case(&root, &case).is_err());
    assert_eq!(
        std::fs::read_to_string(case.join("source.json")).unwrap(),
        "keep"
    );
}

#[test]
fn rejects_source_overlap_before_creating_directories() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    std::fs::create_dir(&source).unwrap();
    assert!(prepare_work_dir(&source, &source.join("cache/nested")).is_err());
    assert!(!source.join("cache").exists());
    assert!(prepare_work_dir(&source, temp.path()).is_err());
    assert!(prepare_work_dir(&source, &source).is_err());
    for retired in [".ffclienteditor", "old.ffclient", ".FFCLIENTEDITOR"] {
        let destination = temp.path().join(retired);
        assert!(prepare_work_dir(&source, &destination).is_err());
        assert!(!destination.exists());
    }
}

#[test]
fn accepts_plain_work_directory_without_project_files() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    std::fs::create_dir(&source).unwrap();
    let work = prepare_work_dir(&source, &temp.path().join("work/case")).unwrap();
    assert!(work.is_dir());
    assert_eq!(std::fs::read_dir(work).unwrap().count(), 0);
}
