use super::*;

fn fixture() -> (tempfile::TempDir, AuditOptions) {
    let temp = tempfile::tempdir().unwrap();
    let csharp = temp.path().join("csharp");
    let firstpass = temp.path().join("firstpass");
    let openfusion = temp.path().join("OpenFusion/src");
    fs::create_dir_all(&csharp).unwrap();
    fs::create_dir_all(&firstpass).unwrap();
    fs::create_dir_all(openfusion.join("core")).unwrap();
    fs::create_dir_all(openfusion.join("servers")).unwrap();
    fs::write(
        csharp.join("Player.cs"),
        "public class Player : MonoBehaviour\n{\n private void OnGUI() {}\n}\n",
    )
    .unwrap();
    fs::write(
        firstpass.join("sP_TEST.cs"),
        "public class sP_TEST { public int P_FE2CL_TEST; }\n",
    )
    .unwrap();
    fs::write(
        openfusion.join("core/Packets.cpp"),
        "PACKET(P_CL2FE_TEST);\n",
    )
    .unwrap();
    fs::write(
        openfusion.join("Combat.cpp"),
        "REGISTER_SHARD_PACKET(P_CL2FE_TEST, testHandler);\n",
    )
    .unwrap();
    fs::write(
        openfusion.join("servers/CNLoginServer.cpp"),
        "case P_CL2LS_REQ_LOGIN:\n",
    )
    .unwrap();
    let coverage = temp.path().join("coverage.json");
    fs::write(
        &coverage,
        serde_json::to_vec_pretty(&CoverageMap {
            schema: COVERAGE_SCHEMA.to_owned(),
            entries: vec![CoverageEntry {
                source: "assembly-csharp/Player.cs".to_owned(),
                status: CoverageStatus::Partial,
                native_modules: vec!["movement".to_owned()],
                evidence: Some("fixture".to_owned()),
            }],
        })
        .unwrap(),
    )
    .unwrap();
    let options = AuditOptions {
        assembly_csharp: csharp,
        assembly_firstpass: firstpass,
        openfusion_src: openfusion,
        coverage: Some(coverage),
    };
    (temp, options)
}

#[test]
fn inventory_is_sorted_hashed_and_joins_explicit_coverage() {
    let (_temp, options) = fixture();
    let inventory = scan(&options).unwrap();
    assert_eq!(inventory.schema, INVENTORY_SCHEMA);
    assert_eq!(inventory.summary.script_files, 2);
    assert_eq!(inventory.summary.classes, 2);
    assert_eq!(inventory.summary.mono_behaviours, 1);
    assert_eq!(inventory.summary.on_gui_scripts, 1);
    assert_eq!(inventory.summary.packet_layout_scripts, 1);
    assert_eq!(inventory.summary.registered_shard_packets, 1);
    assert_eq!(inventory.summary.login_packet_cases, 1);
    assert_eq!(inventory.summary.packet_definitions, 1);
    assert_eq!(inventory.scripts[0].source, "assembly-csharp/Player.cs");
    assert_eq!(inventory.scripts[0].coverage, CoverageStatus::Partial);
    assert_eq!(inventory.scripts[0].blake3.len(), 64);
    assert_eq!(
        inventory.open_fusion.shard_registrations[0].handler,
        "testHandler"
    );
}

#[test]
fn unchanged_inputs_serialize_identically() {
    let (_temp, options) = fixture();
    let first = serde_json::to_vec(&scan(&options).unwrap()).unwrap();
    let second = serde_json::to_vec(&scan(&options).unwrap()).unwrap();
    assert_eq!(first, second);
}

#[test]
fn stale_or_duplicate_coverage_is_rejected() {
    let (_temp, mut options) = fixture();
    let coverage = options.coverage.as_ref().unwrap();
    let stale = CoverageMap {
        schema: COVERAGE_SCHEMA.to_owned(),
        entries: vec![CoverageEntry {
            source: "assembly-csharp/Missing.cs".to_owned(),
            status: CoverageStatus::Partial,
            native_modules: Vec::new(),
            evidence: None,
        }],
    };
    fs::write(coverage, serde_json::to_vec(&stale).unwrap()).unwrap();
    assert!(scan(&options).unwrap_err().contains("does not exist"));

    options.coverage = None;
    assert!(scan(&options).is_ok());
}

fn checked_in_ui_parity_matrix() -> UiParityMatrix {
    serde_json::from_slice(include_bytes!(
        "../../../docs/reference/evidence/legacy/ffone/ui/parity-matrix.json"
    ))
    .unwrap()
}

#[test]
fn checked_in_ui_parity_matrix_is_complete_and_fail_closed() {
    let matrix = checked_in_ui_parity_matrix();
    validate_ui_parity_matrix(&matrix).unwrap();
    assert_eq!(
        matrix
            .entries
            .iter()
            .filter(|entry| entry.kind == UiParityEntryKind::Mode)
            .count(),
        32
    );
    assert_eq!(
        matrix
            .entries
            .iter()
            .filter(|entry| entry.kind == UiParityEntryKind::Family)
            .count(),
        9
    );
}

#[test]
fn ui_parity_matrix_rejects_duplicate_and_missing_mode_ids() {
    let mut duplicate = checked_in_ui_parity_matrix();
    duplicate.entries[1].id = duplicate.entries[0].id.clone();
    assert!(validate_ui_parity_matrix(&duplicate)
        .unwrap_err()
        .contains("duplicate"));

    let mut missing = checked_in_ui_parity_matrix();
    missing.entries.retain(|entry| entry.id != "mode.rule");
    assert!(validate_ui_parity_matrix(&missing)
        .unwrap_err()
        .contains("mode.rule"));
}

#[test]
fn ui_parity_matrix_rejects_invalid_status_and_unproven_implementation() {
    let invalid_status = String::from_utf8(
        include_bytes!("../../../docs/reference/evidence/legacy/ffone/ui/parity-matrix.json").to_vec(),
    )
    .unwrap()
    .replacen(
        "\"native_status\": \"partial\"",
        "\"native_status\": \"complete\"",
        1,
    );
    assert!(serde_json::from_str::<UiParityMatrix>(&invalid_status).is_err());

    let mut implemented = checked_in_ui_parity_matrix();
    let login = implemented
        .entries
        .iter_mut()
        .find(|entry| entry.id == "mode.login")
        .unwrap();
    login.native_status = UiNativeStatus::Implemented;
    login.functional_gaps.clear();
    validate_ui_parity_matrix(&implemented).unwrap();

    let mut no_tests = implemented.clone();
    no_tests
        .entries
        .iter_mut()
        .find(|entry| entry.id == "mode.login")
        .unwrap()
        .native_tests
        .clear();
    assert!(validate_ui_parity_matrix(&no_tests)
        .unwrap_err()
        .contains("without native tests"));

    let mut no_captures = implemented.clone();
    no_captures
        .entries
        .iter_mut()
        .find(|entry| entry.id == "mode.login")
        .unwrap()
        .native_captures
        .clear();
    assert!(validate_ui_parity_matrix(&no_captures)
        .unwrap_err()
        .contains("without native captures"));

    let mut no_evidence = implemented;
    no_evidence
        .entries
        .iter_mut()
        .find(|entry| entry.id == "mode.login")
        .unwrap()
        .evidence_authority
        .clear();
    assert!(validate_ui_parity_matrix(&no_evidence)
        .unwrap_err()
        .contains("no legacy evidence authority"));
}
