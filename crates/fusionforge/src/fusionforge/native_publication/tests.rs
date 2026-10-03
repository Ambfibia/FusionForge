use super::*;
#[test]
fn additive_texture_never_overwrites_different_artwork() {
    let root = tempfile::tempdir().unwrap();
    assert!(check_additive(root.path(), "icon.png", b"accepted").is_ok());
    fs::write(root.path().join("icon.png"), b"accepted").unwrap();
    assert!(check_additive(root.path(), "icon.png", b"accepted").is_ok());
    assert!(check_additive(root.path(), "icon.png", b"different").is_err());
    assert_eq!(fs::read(root.path().join("icon.png")).unwrap(), b"accepted");
}
#[test]
fn native_json_constants_retain_published_order_and_numeric_spelling() {
    let raw:Box<serde_json::value::RawValue>=serde_json::from_str("{\r\n        \"schema\": \"native.v1\",\r\n        \"z\": 1.2345000,\r\n        \"a\": [0]\r\n      }").unwrap();
    assert_eq!(
        native_json_bytes(&raw, &LineEnding::Crlf).unwrap(),
        b"{\r\n  \"schema\": \"native.v1\",\r\n  \"z\": 1.2345000,\r\n  \"a\": [0]\r\n}\r\n"
    );
    let lf = native_json_bytes(&raw, &LineEnding::Lf).unwrap();
    assert!(!lf.contains(&b'\r'));
    let lf_raw: Box<serde_json::value::RawValue> = serde_json::from_slice(&lf).unwrap();
    assert_eq!(
        native_json_bytes(&lf_raw, &LineEnding::Crlf).unwrap(),
        native_json_bytes(&raw, &LineEnding::Crlf).unwrap()
    );
}
#[test]
fn rollback_restores_replaced_and_new_files_and_rejects_changed_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    let stage = temp.path().join("stage");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("old"), b"original").unwrap();
    let rows = vec![
        ("old".into(), b"edited".to_vec()),
        ("new".into(), b"created".to_vec()),
        ("fail".into(), vec![1]),
    ];
    let before = snapshot(&target, &["old".into(), "new".into(), "fail".into()]).unwrap();
    let result = install_with(&target, &stage, &rows, true, &before, |path, bytes| {
        if path.file_name().unwrap() == "fail" {
            Err("injected write failure".into())
        } else {
            replace(path, bytes)
        }
    });
    assert!(result.unwrap_err().contains("rollback failures: []"));
    assert_eq!(fs::read(target.join("old")).unwrap(), b"original");
    assert!(!target.join("new").exists());
    fs::write(target.join("old"), b"concurrent change").unwrap();
    assert!(
        install_checked(&target, &stage, &rows, true, &before)
            .unwrap_err()
            .contains("inputs changed")
    );
    assert_eq!(fs::read(target.join("old")).unwrap(), b"concurrent change");
    assert!(!target.join("new").exists());
}
#[test]
fn publication_checks_all_paths_and_stages_without_target_writes() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep.json"), "old").unwrap();
    let outputs = vec![("keep.json".into(), b"new".to_vec())];
    let stage = root.path().join("stage");
    install(&target, &stage, &outputs, false).unwrap();
    assert_eq!(fs::read(target.join("keep.json")).unwrap(), b"old");
    assert_eq!(fs::read(stage.join("native/keep.json")).unwrap(), b"new");
    let mut invalid = outputs.clone();
    invalid.push(("../escape".into(), vec![1]));
    assert!(install(&target, &stage, &invalid, true).is_err());
    assert_eq!(fs::read(target.join("keep.json")).unwrap(), b"old");
    install(&target, &stage, &outputs, true).unwrap();
    assert_eq!(fs::read(target.join("keep.json")).unwrap(), b"new");
    assert_eq!(fs::read(stage.join("before/keep.json")).unwrap(), b"old");
}
#[test]
fn rejects_missing_proof_wrong_hash_and_duplicate_outputs() {
    for path in [
        "../escape",
        ".. /escape",
        "x./file",
        "CON.png",
        "a/NUL",
        "file.json ",
        "a:b",
    ] {
        assert!(relative(Path::new("root"), path).is_err(), "{path}");
    }
    assert!(check_bytes(b"wrong", Some(&digest(b"right")), None).is_err());
    assert!(
        check_assertions(
            &json!({}),
            &[Assertion {
                pointer: "/missing".into(),
                equals: Value::Null
            }]
        )
        .is_err()
    );
    let root = tempfile::tempdir().unwrap();
    let outputs = vec![("same.json".into(), vec![1]), ("SAME.json".into(), vec![2])];
    assert!(install(root.path(), &root.path().join("stage"), &outputs, true).is_err());
    assert!(!root.path().join("same.json").exists());
}
