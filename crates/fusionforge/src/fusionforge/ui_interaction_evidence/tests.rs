use super::*;
use tempfile::tempdir;

fn member(owner: &str, name: &str, return_type: &str) -> ManagedMemberIdentity {
    let parameter_types = match (owner, name) {
        ("UnityEngine.GUI", "Button") => vec!["UnityEngine.Rect", "System.String"],
        ("UnityEngine.GUI", "set_enabled") => vec!["System.Boolean"],
        ("UnityEngine.GUI", "Toolbar") => {
            vec!["UnityEngine.Rect", "System.Int32", "System.String[]"]
        }
        ("UnityEngine.GUI", "SelectionGrid") => vec![
            "UnityEngine.Rect",
            "System.Int32",
            "System.String[]",
            "System.Int32",
        ],
        ("UnityEngine.GUI", "HorizontalSlider") => vec![
            "UnityEngine.Rect",
            "System.Single",
            "System.Single",
            "System.Single",
        ],
        ("UnityEngine.GUILayout", "VerticalScrollbar") => vec![
            "System.Single",
            "System.Single",
            "System.Single",
            "System.Single",
            "UnityEngine.GUILayoutOption[]",
        ],
        ("UnityEngine.GUIUtility", "GetControlID") => vec!["UnityEngine.FocusType"],
        ("UnityEngine.Event", "GetTypeForControl") => vec!["System.Int32"],
        ("UnityEngine.GUILayout", "Button") => {
            vec!["System.String", "UnityEngine.GUILayoutOption[]"]
        }
        ("UnityEngine.GUILayout", "BeginHorizontal") => {
            vec!["UnityEngine.GUILayoutOption[]"]
        }
        _ => Vec::new(),
    };
    ManagedMemberIdentity {
        declaring_type: owner.to_string(),
        name: name.to_string(),
        parameter_types: parameter_types.into_iter().map(str::to_string).collect(),
        return_type: return_type.to_string(),
        metadata_token: "0x0A000001".to_string(),
    }
}
#[test]
fn request_is_versioned_and_requires_an_exact_root() {
    let request = UiInteractionAnalysisRequest {
        schema: REQUEST_SCHEMA.to_string(),
        schema_version: 1,
        managed: ManagedAssemblyInput {
            evidence_report: PathBuf::from("managed.json"),
            payload: None,
        },
        roots: Vec::new(),
        traversal: TraversalLimits::default(),
    };
    assert!(request.validate().unwrap_err().contains("root"));

    let wrong = UiInteractionAnalysisRequest {
        schema: "unbound-il.v1".to_string(),
        roots: vec![ManagedMethodSelector {
            declaring_type: "cnOptionMode".to_string(),
            name: "OnGUI".to_string(),
            parameter_types: Vec::new(),
            return_type: Some("System.Void".to_string()),
        }],
        ..request
    };
    assert!(wrong.validate().unwrap_err().contains(REQUEST_SCHEMA));
}

#[test]
fn api_recognition_uses_exact_clr_owner_and_member_identity() {
    let button = classify_api(&member("UnityEngine.GUI", "Button", "System.Boolean"));
    assert_eq!(button.category, CallCategory::Control);
    assert_eq!(button.interaction, InteractionKind::Button);
    assert!(button.boolean_result);

    let spoofed_owner = classify_api(&member(
        "Legacy.UnityEngine.GUI",
        "Button",
        "System.Boolean",
    ));
    assert_eq!(spoofed_owner.category, CallCategory::Other);
    let substring = classify_api(&member("UnityEngine.GUI", "ButtonLike", "System.Boolean"));
    assert_eq!(substring.category, CallCategory::Other);

    let event_use = classify_api(&member("UnityEngine.Event", "Use", "System.Void"));
    assert_eq!(event_use.interaction, InteractionKind::EventUse);
    let enabled = classify_api(&member("UnityEngine.GUI", "set_enabled", "System.Void"));
    assert_eq!(enabled.interaction, InteractionKind::GuiEnabledWrite);
}

#[test]
fn decoder_and_cfg_preserve_both_short_branch_successors() {
    // ldc.i4.0; brfalse.s IL_0005; ldc.i4.1; ret; ldc.i4.2; ret
    let code = [0x16, 0x2c, 0x02, 0x17, 0x2a, 0x18, 0x2a];
    let instructions = decode_il(&code).expect("decode");
    let cfg = build_cfg(&instructions, &code).expect("cfg");
    assert_eq!(cfg.blocks.len(), 3);
    assert!(cfg.edges.iter().any(|edge| {
        edge.from == 0 && edge.to == 1 && edge.kind == ControlFlowEdgeKind::Fallthrough
    }));
    assert!(cfg.edges.iter().any(|edge| {
        edge.from == 0 && edge.to == 2 && edge.kind == ControlFlowEdgeKind::BranchTaken
    }));
    assert!(cfg
        .edges
        .iter()
        .all(|edge| edge.certainty == EvidenceLevel::StaticProven));
}

#[test]
fn adjacent_gui_boolean_call_has_a_proven_il_result_branch_not_runtime_truth() {
    // call token; brfalse.s IL_0008; ret; ret
    let code = [0x28, 1, 0, 0, 10, 0x2c, 0x01, 0x2a, 0x2a];
    let instructions = decode_il(&code).expect("decode");
    let cfg = build_cfg(&instructions, &code).expect("cfg");
    let calls = vec![CallSiteEvidence {
        site_id: "M06000001_IL_0000".to_string(),
        il_offset: 0,
        basic_block: 0,
        opcode: "call".to_string(),
        referenced_member: member("UnityEngine.GUI", "Button", "System.Boolean"),
        dispatch: CallDispatchKind::Direct,
        runtime_target_proven: true,
        category: CallCategory::Control,
        interaction: InteractionKind::Button,
        certainty: EvidenceLevel::StaticProven,
        runtime_execution_proven: false,
    }];
    let branches =
        direct_result_branches(&calls, &instructions, &code, &cfg).expect("result branch");
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].true_successor, 1);
    assert_eq!(branches[0].false_successor, 2);
    assert_eq!(branches[0].certainty, EvidenceLevel::StaticProven);
    assert!(!calls[0].runtime_execution_proven);
}

#[test]
fn field_value_is_only_proven_for_the_immediately_consumed_constant() {
    let code = [0x1f, 0xfb, 0x80, 1, 0, 0, 4];
    let instructions = decode_il(&code).expect("decode");
    assert_eq!(previous_i32_constant(&instructions, &code, 1), Some(-5));

    let with_conversion = [0x17, 0x69, 0x80, 1, 0, 0, 4];
    let converted = decode_il(&with_conversion).expect("decode");
    assert_eq!(previous_i32_constant(&converted, &with_conversion, 2), None);
}

#[test]
fn reserved_opcode_fails_closed_instead_of_desynchronizing_il() {
    let error = decode_il(&[0x24]).unwrap_err();
    assert!(error.contains("reserved"));
}

#[test]
fn request_paths_are_portable_and_cannot_escape_the_request_directory() {
    assert_eq!(
        portable_relative_parts(Path::new("work\\managed.json"), "evidence").expect("portable"),
        vec!["work".to_string(), "managed.json".to_string()]
    );
    assert!(portable_relative_parts(Path::new("../managed.json"), "evidence").is_err());
    assert!(portable_relative_parts(Path::new("D:\\managed.json"), "evidence").is_err());
}

#[test]
fn output_guard_protects_request_evidence_and_payload_inputs() {
    let root = tempdir().expect("tempdir");
    let request = root.path().join("request.json");
    let evidence = root.path().join("managed.json");
    let payload = root.path().join("Assembly.dll");
    fs::write(&request, b"{}").expect("request");
    fs::write(&evidence, b"{}").expect("evidence");
    fs::write(&payload, b"MZ").expect("payload");
    assert!(
        preflight_output(Some(&request), &request, &evidence, Some(&payload))
            .unwrap_err()
            .contains("request")
    );
    assert!(
        preflight_output(Some(&evidence), &request, &evidence, Some(&payload))
            .unwrap_err()
            .contains("evidence")
    );
    assert!(
        preflight_output(Some(&payload), &request, &evidence, Some(&payload))
            .unwrap_err()
            .contains("payload")
    );
}

#[test]
fn evidence_levels_serialize_with_fail_closed_vocabulary() {
    assert_eq!(
        serde_json::to_string(&EvidenceLevel::StaticProven).expect("json"),
        "\"staticProven\""
    );
    assert_eq!(
        serde_json::to_string(&EvidenceLevel::Candidate).expect("json"),
        "\"candidate\""
    );
    assert_eq!(
        serde_json::to_string(&EvidenceLevel::Unresolved).expect("json"),
        "\"unresolved\""
    );
}

#[test]
fn hash_envelope_rejects_same_length_wrong_payload() {
    let expected = sha256_hex(b"managed");
    require_bytes_and_hash("managed payload", b"managed", 7, &expected).expect("match");
    let error =
        require_bytes_and_hash("managed payload", b"changed", 7, &expected).unwrap_err();
    assert!(error.contains("SHA-256 mismatch"));
}
#[test]
fn extended_imgui_catalog_is_typed_and_wrong_return_overloads_fail_closed() {
    let cases = [
        (
            "UnityEngine.GUI",
            "Toolbar",
            "System.Int32",
            InteractionKind::Toolbar,
        ),
        (
            "UnityEngine.GUI",
            "SelectionGrid",
            "System.Int32",
            InteractionKind::SelectionGrid,
        ),
        (
            "UnityEngine.GUI",
            "HorizontalSlider",
            "System.Single",
            InteractionKind::Slider,
        ),
        (
            "UnityEngine.GUILayout",
            "VerticalScrollbar",
            "System.Single",
            InteractionKind::Scrollbar,
        ),
        (
            "UnityEngine.GUI",
            "DragWindow",
            "System.Void",
            InteractionKind::DragWindow,
        ),
        (
            "UnityEngine.GUIUtility",
            "GetControlID",
            "System.Int32",
            InteractionKind::ControlId,
        ),
        (
            "UnityEngine.GUIUtility",
            "ExitGUI",
            "System.Void",
            InteractionKind::ExitGui,
        ),
        (
            "UnityEngine.Event",
            "GetTypeForControl",
            "UnityEngine.EventType",
            InteractionKind::EventTypeForControl,
        ),
        (
            "UnityEngine.GUILayout",
            "Button",
            "System.Boolean",
            InteractionKind::Button,
        ),
        (
            "UnityEngine.GUILayout",
            "BeginHorizontal",
            "System.Void",
            InteractionKind::HorizontalScopeBegin,
        ),
        (
            "UnityEngine.GUILayout",
            "EndVertical",
            "System.Void",
            InteractionKind::VerticalScopeEnd,
        ),
    ];
    for (owner, name, return_type, expected) in cases {
        let classified = classify_api(&member(owner, name, return_type));
        assert_eq!(classified.interaction, expected, "{owner}::{name}");
    }
    let unknown_overload = classify_api(&member(
        "UnityEngine.GUILayout",
        "Toolbar",
        "System.Boolean",
    ));
    assert_eq!(unknown_overload.category, CallCategory::Other);
    let mut unknown_parameter_shape = member("UnityEngine.GUI", "Toolbar", "System.Int32");
    unknown_parameter_shape.parameter_types = vec!["System.DateTime".to_string()];
    assert_eq!(
        classify_api(&unknown_parameter_shape).category,
        CallCategory::Other
    );
    assert!(is_candidate_control_kind(&InteractionKind::Toolbar));
    assert!(is_candidate_control_kind(&InteractionKind::Scrollbar));
}

#[test]
fn nested_explicit_output_is_preflighted_created_and_round_trips() {
    let root = tempdir().expect("tempdir");
    let request = root.path().join("request.json");
    let evidence = root.path().join("managed.json");
    let payload = root.path().join("Assembly.dll");
    fs::write(&request, b"{}").expect("request");
    fs::write(&evidence, b"{}").expect("evidence");
    fs::write(&payload, b"MZ").expect("payload");
    let output = root.path().join("nested").join("reports").join("ui.json");
    preflight_output(Some(&output), &request, &evidence, Some(&payload))
        .expect("safe missing target");
    fs::create_dir_all(output.parent().expect("parent")).expect("parents");
    preflight_output(Some(&output), &request, &evidence, Some(&payload))
        .expect("safe materialized parent");
    fs::write(&output, b"{\"schema\":\"round-trip\"}\n").expect("write");
    assert_eq!(
        fs::read(&output).expect("read"),
        b"{\"schema\":\"round-trip\"}\n"
    );
}

#[test]
fn request_fingerprint_is_lowercase_and_serialized_evidence_has_no_cli_paths() {
    let raw_request = br#"{"managed":{"evidenceReport":"D:\\Case\\managed.json"}}"#;
    let request_sha256 = sha256_hex_lower(raw_request);
    assert_eq!(request_sha256.len(), 64);
    assert!(request_sha256
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    let evidence = UiInteractionEvidence {
        schema: EVIDENCE_SCHEMA,
        schema_version: 1,
        request_sha256: request_sha256.clone(),
        request_bytes: raw_request.len() as u64,
        request_hash_mode: "exact-request-json-bytes-v1".to_string(),
        traversal: TraversalLimits {
            max_direct_call_depth: 2,
            max_methods: 32,
        },
        authority: ManagedAuthority {
            evidence_report_bytes: 1,
            evidence_report_sha256: "A".repeat(64),
            source_alias: "primary".to_string(),
            raw_container_relative_path: "main.unity3d".to_string(),
            raw_container_sha256: "B".repeat(64),
            selected_level: 0,
            selected_exact_entry: "Assembly - CSharp.dll".to_string(),
            selected_route: "level0/Assembly - CSharp.dll".to_string(),
            payload_bytes: 1,
            payload_sha256: "C".repeat(64),
            clr_metadata_bytes: 1,
            clr_metadata_sha256: "D".repeat(64),
        },
        roots: Vec::new(),
        methods: Vec::new(),
        direct_calls: Vec::new(),
        limitations: Vec::new(),
        runtime_behavior_proven: false,
    };
    let json = serde_json::to_string(&evidence).expect("json");
    assert!(json.contains(&format!("\"requestSha256\":\"{request_sha256}\"")));
    assert!(json.contains("\"maxDirectCallDepth\":2"));
    assert!(!json.contains("D:\\\\Case"));
    assert!(!json.contains("requestPath"));
    assert!(!json.contains("evidenceReportPath"));
}

#[test]
fn candidate_schema_is_editor_only_and_not_publishable_as_an_ffone_contract() {
    assert_eq!(CANDIDATE_SCHEMA, "fusionforge.ui-interaction-candidate.v1");
    assert_ne!(CANDIDATE_SCHEMA, "fusionforge.native-ui-contract.v1");
}
#[test]
fn callvirt_is_a_reference_only_and_is_never_traversed_by_v1() {
    assert!(should_traverse_call(IlOpcode::Single(0x28)));
    assert!(!should_traverse_call(IlOpcode::Single(0x6f)));
    assert!(!should_traverse_call(IlOpcode::Single(0x73)));
    assert_eq!(
        call_dispatch_kind(IlOpcode::Single(0x6f)),
        CallDispatchKind::VirtualReference
    );
}

#[test]
fn owner_field_read_control_dependency_and_gate_gaps_are_explicit() {
    let limitations = global_limitations();
    let codes = limitations
        .iter()
        .map(|limitation| limitation.code.as_str())
        .collect::<BTreeSet<_>>();
    assert!(codes.contains("serializedStateNotJoined"));
    assert!(codes.contains("fieldReadsAndControlDependenciesUnresolved"));
    assert!(codes.contains("dynamicDispatchUnknown"));
    assert!(limitations
        .iter()
        .all(|limitation| { limitation.level == EvidenceLevel::Unresolved }));
}
