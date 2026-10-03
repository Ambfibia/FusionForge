use super::*;

fn args(values: &[&str]) -> Vec<std::ffi::OsString> {
    values.iter().map(std::ffi::OsString::from).collect()
}

#[test]
fn help_does_not_require_a_pack() {
    assert_eq!(run(args(&["--help"])).unwrap(), USAGE);
}

#[test]
fn import_requires_pack() {
    let error = run(args(&["import"])).unwrap_err();
    assert!(error.contains("--pack <DIR> is required"));
}

#[test]
fn asset_composition_help_does_not_touch_asset_roots() {
    assert_eq!(run(args(&["compose-assets", "--help"])).unwrap(), USAGE);
}

#[test]
fn asset_composition_requires_a_prefix_pair() {
    let error = run(args(&["compose-assets", "base", "overlay", "output"])).unwrap_err();
    assert!(error.contains("at least one --overlay-prefix"));
}

#[test]
fn asset_composition_rejects_unknown_options_before_io() {
    let error = run(args(&[
        "compose-assets",
        "base",
        "overlay",
        "output",
        "--unknown",
        "audio/",
    ]))
    .unwrap_err();
    assert!(error.contains("unknown compose-assets argument"));
}

#[test]
fn legacy_source_flags_are_rejected() {
    let error = run(args(&["import", "--project", "old.ffclient"])).unwrap_err();
    assert!(error.contains("unknown argument"));
}

#[test]
fn logical_model_publish_requires_three_positional_arguments() {
    let error = run(args(&["publish-logical-model", "source.json", "npc"])).unwrap_err();
    assert!(error.contains("requires <SOURCE_JSON> <FAMILY> <OUTPUT_ROOT>"));
}

#[test]
fn logical_model_batch_publish_requires_two_positional_arguments() {
    let error = run(args(&["publish-logical-model-batch", "sources"])).unwrap_err();
    assert!(error.contains("requires <SOURCE_ROOT> <FRESH_OUTPUT_ROOT>"));
}

#[test]
fn equipment_logical_model_batch_publish_requires_two_positional_arguments() {
    let error = run(args(&["publish-equipment-logical-model-batch", "sources"])).unwrap_err();
    assert!(error.contains("requires <SOURCE_ROOT> <FRESH_OUTPUT_ROOT>"));
}

#[test]
fn equipment_gpu_batch_requires_five_positional_arguments() {
    let error = run(args(&[
        "accept-equipment-gpu-batch",
        "candidate",
        "evidence",
        "preview.exe",
        "smoke",
    ]))
    .unwrap_err();
    assert!(error.contains("requires <CANDIDATE_ROOT> <EVIDENCE_ROOT>"));
}

#[test]
fn logical_model_tree_audit_requires_an_output_root() {
    let error = run(args(&["audit-logical-model-tree"])).unwrap_err();
    assert!(error.contains("requires <OUTPUT_ROOT> [REPORT_JSON]"));
}

#[test]
fn tutorial_model_install_help_does_not_touch_roots() {
    assert_eq!(
        run(args(&["install-tutorial-models", "--help"])).unwrap(),
        USAGE
    );
}

#[test]
fn tutorial_model_install_requires_both_repeatable_proof_flags() {
    let error = run(args(&[
        "install-tutorial-models",
        "candidate",
        "assets",
        "build",
        "--evidence-root",
        "evidence",
    ]))
    .unwrap_err();
    assert!(error.contains("at least one --evidence-root and at least one --model"));
}

#[test]
fn tutorial_model_install_rejects_unknown_option_before_io() {
    let error = run(args(&[
        "install-tutorial-models",
        "candidate",
        "assets",
        "build",
        "--unknown",
        "value",
    ]))
    .unwrap_err();
    assert!(error.contains("unknown install-tutorial-models argument"));
}

#[test]
fn tutorial_static_world_install_help_does_not_touch_roots() {
    assert_eq!(
        run(args(&["install-tutorial-static-world", "--help"])).unwrap(),
        USAGE
    );
}

#[test]
fn tutorial_static_world_install_requires_two_roots() {
    let error = run(args(&["install-tutorial-static-world", "export"])).unwrap_err();
    assert!(error.contains("requires <EXPORT_ROOT> <ASSET_ROOT>"));
}

#[test]
fn semantic_audio_install_requires_three_positional_arguments() {
    let error = run(args(&[
        "install-semantic-audio",
        "assets/game",
        "cook-report.json",
    ]))
    .unwrap_err();
    assert!(error.contains("requires <ASSET_ROOT> <COOK_REPORT_JSON> <SOURCE_BUILD>"));
}

#[test]
fn legacy_gui_skin_help_marks_output_as_non_publishable_candidate() {
    let help =
        run_legacy_gui_skin_conversion(&args(&["convert-legacy-gui-skins", "--help"])).unwrap();
    let command = help
        .lines()
        .skip_while(|line| !line.contains("convert-legacy-gui-skins <"))
        .take(2)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(command.contains("editor-only candidate"));
    assert!(command.contains("publication is forbidden"));
    assert!(!command.contains("exact"));
}
