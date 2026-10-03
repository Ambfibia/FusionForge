use super::*;

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn help_does_not_read_reference_roots() {
    assert_eq!(run(args(&["--help"])).unwrap(), USAGE);
}

#[test]
fn scan_command_is_required() {
    assert!(run(Vec::<OsString>::new()).unwrap_err().contains("scan"));
}

#[test]
fn scan_requires_explicit_external_source_roots() {
    let error = run(args(&["scan"])).unwrap_err();
    assert!(error.contains("--csharp is required"));
}

#[test]
fn ui_parity_command_requires_a_value_after_input() {
    assert!(
        run(args(&["validate-ui-parity", "--input"]))
            .unwrap_err()
            .contains("requires a path")
    );
}
