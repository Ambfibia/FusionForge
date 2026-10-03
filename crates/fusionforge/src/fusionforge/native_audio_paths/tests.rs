use super::*;
#[test]
fn move_failure_restores_earlier_sources() {
    let dir = tempfile::tempdir().unwrap();
    let mut rows = Vec::new();
    for name in ["first", "second"] {
        let p = dir.path().join(name);
        fs::write(&p, name.as_bytes()).unwrap();
        rows.push((
            p,
            dir.path().join(format!("{name}.new")),
            name.as_bytes().to_vec(),
        ));
    }
    let result = commit_moves(&rows, |a, b| {
        if a.file_name().unwrap() == "second" {
            Err("injected failure".into())
        } else {
            fs::rename(a, b).map_err(|e| e.to_string())
        }
    });
    assert!(result.unwrap_err().contains("rollback failures: []"));
    for (a, b, bytes) in rows {
        assert_eq!(fs::read(a).unwrap(), bytes);
        assert!(!b.exists());
    }
}
