use super::*;

#[test]
fn style_candidate_keeps_observed_unity_offsets_and_state_assets() {
    let style = serde_json::json!({
        "m_Font": {"fileId": 0, "pathId": 903},
        "m_Normal": {
            "m_Background": {"fileId": 0, "pathId": 640},
            "m_TextColor": {"r": 0.8, "g": 1.0, "b": 1.0, "a": 1.0}
        },
        "m_Border": {"m_Left": 8, "m_Right": 8, "m_Top": 5, "m_Bottom": 5},
        "m_Margin": {"m_Left": 4, "m_Right": 4, "m_Top": 4, "m_Bottom": 4},
        "m_Padding": {"m_Left": 6, "m_Right": 6, "m_Top": 3, "m_Bottom": 6},
        "m_Overflow": {"m_Left": 0, "m_Right": 0, "m_Top": 0, "m_Bottom": 0},
        "m_ContentOffset": {"x": 1.25, "y": -2.5},
        "m_ImagePosition": 0,
        "m_Alignment": 4,
        "m_WordWrap": 0,
        "m_TextClipping": 1,
        "m_FixedWidth": 0.0,
        "m_FixedHeight": 0.0,
        "m_StretchWidth": 1,
        "m_StretchHeight": 0
    });
    let identities = BTreeMap::from([
        (
            ("ui.assets".to_owned(), 640),
            ObjectIdentityMatch::Unique(ObjectIdentity {
                name: "blue_button_normal".to_owned(),
                asset_type: "Texture2D".to_owned(),
            }),
        ),
        (
            ("ui.assets".to_owned(), 903),
            ObjectIdentityMatch::Unique(ObjectIdentity {
                name: "JEFFE___14".to_owned(),
                asset_type: "Font".to_owned(),
            }),
        ),
    ]);
    let converted = convert_style("button", &style, "ui.assets", &identities);
    assert_eq!(
        converted.border,
        LegacyGuiInsets {
            left: 8,
            right: 8,
            top: 5,
            bottom: 5
        }
    );
    assert_eq!(
        converted.padding,
        LegacyGuiInsets {
            left: 6,
            right: 6,
            top: 3,
            bottom: 6
        }
    );
    assert_eq!(
        converted.content_offset,
        LegacyGuiVector2 { x: 1.25, y: -2.5 }
    );
    assert_eq!(converted.font.asset_name.as_deref(), Some("JEFFE___14"));
    assert_eq!(
        converted.states["normal"].background.asset_name.as_deref(),
        Some("blue_button_normal")
    );
}

#[test]
fn pointer_resolution_scopes_colliding_path_ids_to_the_owner_asset() {
    let objects = vec![
        serde_json::json!({"asset": "a.assets", "pathId": 640, "type": "Texture2D", "name": "from_a"}),
        serde_json::json!({"asset": "b.assets", "pathId": 640, "type": "Texture2D", "name": "from_b"}),
    ];
    let identities = object_identities(&objects);
    let reference = serde_json::json!({"fileId": 0, "pathId": 640});

    let from_a = pointer(&reference, "a.assets", &identities);
    let from_b = pointer(&reference, "b.assets", &identities);
    assert_eq!(from_a.asset_name.as_deref(), Some("from_a"));
    assert_eq!(from_b.asset_name.as_deref(), Some("from_b"));
}

#[test]
fn ambiguous_composite_and_external_pointer_resolution_fail_closed() {
    let objects = vec![
        serde_json::json!({"asset": "ui.assets", "pathId": 640, "type": "Texture2D", "name": "first"}),
        serde_json::json!({"asset": "ui.assets", "pathId": 640, "type": "Texture2D", "name": "second"}),
    ];
    let identities = object_identities(&objects);
    let local = serde_json::json!({"fileId": 0, "pathId": 640});
    let external = serde_json::json!({"fileId": 1, "pathId": 640});
    let local_pointer = pointer(&local, "ui.assets", &identities);
    let external_pointer = pointer(&external, "ui.assets", &identities);

    assert!(local_pointer.asset_name.is_none());
    assert!(local_pointer.asset_type.is_none());
    assert!(external_pointer.asset_name.is_none());
    assert!(external_pointer.asset_type.is_none());

    let mut diagnostics = Vec::new();
    assert_eq!(
        append_unresolved_pointer_diagnostic(
            &local_pointer,
            "Skin",
            Some("button"),
            "m_Normal.m_Background",
            &mut diagnostics,
        ),
        1
    );
    assert_eq!(
        append_unresolved_pointer_diagnostic(
            &external_pointer,
            "Skin",
            Some("button"),
            "m_Hover.m_Background",
            &mut diagnostics,
        ),
        1
    );
    assert_eq!(diagnostics.len(), 2);
    assert!(
        diagnostics
            .iter()
            .all(|item| item.code == "unresolvedPointer")
    );
}

#[test]
fn selected_skin_requires_exact_gui_skin_type() {
    let wrong = serde_json::json!({"type": "GameObject"});
    let error = require_gui_skin_type(&wrong, "FusionFallOptionSkin")
        .expect_err("same-name non-GUISkin must fail closed")
        .to_string();
    assert!(error.contains("expected exactly GUISkin"));

    let exact = serde_json::json!({"type": "GUISkin"});
    require_gui_skin_type(&exact, "FusionFallOptionSkin").unwrap();
}

#[test]
fn missing_acceptance_fields_are_candidate_diagnostics_not_proven_zeroes() {
    let style = serde_json::json!({
        "m_Font": {"fileId": 0, "pathId": 0},
        "m_Normal": {
            "m_Background": {"fileId": 0, "pathId": 0},
            "m_TextColor": {"r": 1.0}
        },
        "m_Border": {"m_Left": 8}
    });
    let mut diagnostics = Vec::new();
    diagnose_style_fields(&mut diagnostics, "FusionFallOptionSkin", "button", &style);

    assert!(
        diagnostics
            .iter()
            .any(|item| { item.code == "missingField" && item.field == "m_Border.m_Right" })
    );
    assert!(
        diagnostics.iter().any(|item| {
            item.code == "missingField" && item.field == "m_Normal.m_TextColor.g"
        })
    );
    assert!(
        diagnostics
            .iter()
            .any(|item| { item.code == "missingField" && item.field == "m_Alignment" })
    );

    let converted = convert_style("button", &style, "ui.assets", &ObjectIdentityIndex::new());
    assert_eq!(converted.border.left, 8);
    assert_eq!(converted.border.right, 0);
    assert_eq!(converted.alignment, 0);
}

#[test]
fn candidate_metadata_forbids_publication_and_serializes_limitations() {
    let candidate = LegacyGuiSkinCandidate {
        schema: LEGACY_GUI_SKIN_SCHEMA.to_owned(),
        evidence_level: LEGACY_GUI_SKIN_EVIDENCE_LEVEL.to_owned(),
        publication_allowed: false,
        limitations: LEGACY_GUI_SKIN_LIMITATIONS
            .iter()
            .map(|limitation| (*limitation).to_owned())
            .collect(),
        source_build: "retrobution".to_owned(),
        source_assets: vec!["ui.assets".to_owned()],
        unresolved_pointer_count: 2,
        diagnostics: Vec::new(),
        skins: Vec::new(),
    };
    let json = serde_json::to_value(candidate).unwrap();

    assert_eq!(
        json["schema"],
        "fusionforge.legacy-unity-gui-skin-candidate.v1"
    );
    assert_eq!(json["evidenceLevel"], "candidate");
    assert_eq!(json["publicationAllowed"], false);
    assert_eq!(json["unresolvedPointerCount"], 2);
    assert!(
        json["limitations"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
}
