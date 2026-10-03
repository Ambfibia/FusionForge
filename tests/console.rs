use std::process::Command;

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fusionforge"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn help_and_errors_are_console_only() {
    let native = cli(&["native", "--help"]);
    assert!(native.status.success());
    assert!(
        String::from_utf8(native.stdout)
            .unwrap()
            .contains("publish-logical-model")
    );
    for args in [vec![], vec!["--help"], vec!["fusionforge", "--help"]] {
        let output = cli(&args);
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("index-client <raw-build-root> [final-reference.json]"));
        assert!(text.contains("export-native-pack"));
        assert!(text.contains("convert-native-model"));
        assert!(!text.contains("cook-ffone-pack"));
    }
    for command in [
        "not-a-command",
        "build-client-project",
        "cook-ffone-pack",
        "create-client-project",
    ] {
        let output = cli(&[command]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("unknown")
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_fusionforge"))
        .args(["fusionforge", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn typed_publication_checks_then_installs_without_a_script_runtime() {
    use std::fs;
    let target = tempfile::tempdir().unwrap();
    let cases = tempfile::tempdir().unwrap();
    let recipe = cases.path().join("recipe.json");
    fs::write(
        target.path().join("table.json"),
        b"{\"value\":0,\"other\":1.2345000}\n",
    )
    .unwrap();
    fs::write(&recipe,br#"{
  "schema": "fusionforge.native-cli-recipe.v1",
  "id": "test-publication",
  "sources": [],
  "steps": [
    {"operation":"json-patch","output":"table.json","edits":[{"path":"/value","before":0,"after":1}]},
    {
      "operation": "native-json",
      "output": "catalog.json",
      "value": {
        "z": 1.2345000,
        "a": [0]
      }
    }
  ]
}"#).unwrap();
    for (case, mode, success) in [
        ("preview", "--check", false),
        ("install", "--apply", true),
        ("verify", "--check", true),
    ] {
        let stage = cases.path().join(case);
        let output = cli(&[
            "publish-native",
            "--recipe",
            recipe.to_str().unwrap(),
            "--target-root",
            target.path().to_str().unwrap(),
            "--replace-existing",
            mode,
        ]);
        assert_eq!(
            output.status.success(),
            success,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !stage.exists(),
            "direct conversion created a case directory"
        );
        if !success {
            assert_eq!(
                fs::read(target.path().join("table.json")).unwrap(),
                b"{\"value\":0,\"other\":1.2345000}\n"
            );
            assert!(!target.path().join("catalog.json").exists());
        }
    }
    assert_eq!(
        fs::read(target.path().join("table.json")).unwrap(),
        b"{\"value\":1,\"other\":1.2345000}\n"
    );
    assert_eq!(
        fs::read(target.path().join("catalog.json")).unwrap(),
        b"{\n  \"z\": 1.2345000,\n  \"a\": [0]\n}\n"
    );
}

#[test]
fn indexing_needs_no_patch_project_and_keeps_source_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("raw-client");
    let work = temp.path().join("reference.json");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("readme.txt"), b"immutable source").unwrap();
    for _ in 0..2 {
        let output = cli(&[
            "index-client",
            source.to_str().unwrap(),
            work.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(summary["bundles"], 0);
        assert!(summary.get("workDir").is_none());
    }
    assert!(work.is_file());
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 2);
    assert_eq!(std::fs::read_dir(source).unwrap().count(), 1);
}

#[test]
fn model_conversion_rejects_missing_sources_and_invalid_options_without_output() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("native-output");
    let args = [
        "convert-native-model",
        "missing.resourceFile",
        "exact/route",
        "characters",
        target.to_str().unwrap(),
    ];
    let result = cli(&args);
    assert!(!result.status.success());
    assert!(!result.stderr.is_empty());
    assert!(!target.exists());
    let mut invalid = args.to_vec();
    invalid.extend(["--unknown", "value"]);
    let result = cli(&invalid);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("convert-native-model <bundle>"));
    assert!(!target.exists());
}

#[test]
fn incomplete_native_export_returns_failure_and_keeps_a_reviewable_report() {
    use sha2::{Digest, Sha256};
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let output = temp.path().join("native");
    std::fs::create_dir(&source).unwrap();
    let bytes = b"not-a-valid-unity-bundle";
    std::fs::write(source.join("main.unity3d"), bytes).unwrap();
    let uuid = "11111111-2222-3333-4444-555555555555";
    std::fs::write(
        source.join(format!("{uuid}.json")),
        serde_json::to_vec(&serde_json::json!({
            "uuid": uuid,
            "main_file_info": {"hash": format!("{:x}", Sha256::digest(bytes)), "size": bytes.len()},
            "bundles": {}
        }))
        .unwrap(),
    )
    .unwrap();
    let result = cli(&[
        "export-native-pack",
        source.to_str().unwrap(),
        output.to_str().unwrap(),
        "en-US",
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("incomplete"));
    let summary: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(summary["complete"], false);
    assert!(std::path::Path::new(summary["report"].as_str().unwrap()).is_file());
    assert!(!source.join("ffpatch.json").exists());
}

#[test]
fn focused_inspection_is_bounded_and_read_only() {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."));
    let input = root.join("builds/retrobution-20260821/FutureNano.resourceFile");
    if !input.exists() {
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fusionforge"))
        .current_dir(scratch.path())
        .args([
            "inspect",
            input.to_str().unwrap(),
            "--type",
            "GameObject",
            "--limit",
            "2",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["objects"].as_array().unwrap().len(), 2);
    assert_eq!(value["truncated"], true);
    assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
}

#[test]
fn streamed_bundle_names_match_full_decode() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../builds/retrobution-20260821");
    if !root.is_dir() {
        return;
    }
    for name in ["main.unity3d", "FutureNano.resourceFile"] {
        let path = root.join(name);
        let entries = ffbuildtool::bundle::AssetBundle::entry_names(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let (_, bundle) = ffbuildtool::bundle::AssetBundle::from_bytes(&bytes).unwrap();
        let full = bundle
            .iter_files()
            .map(|(_, name, _)| name.to_owned())
            .collect::<Vec<_>>();
        assert_eq!(entries, full);
    }
}
