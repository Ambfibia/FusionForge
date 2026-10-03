use super::*;
use serde_json::json;
use tempfile::tempdir;

fn identity(unity_type: &str, path_id: i64) -> UnityObjectIdentity {
    UnityObjectIdentity {
        source_alias: "primary".to_string(),
        relative_container: "StandaloneWindows/option.resourceFile".to_string(),
        raw_container_sha256: "1".repeat(64),
        serialized_asset: "sharedassets0.assets".to_string(),
        unity_type: unity_type.to_string(),
        path_id,
        raw_object_sha256: "2".repeat(64),
    }
}

fn texture(width: u32, height: u32) -> TextureEvidence {
    TextureEvidence {
        object: identity("Texture2D", 640),
        width,
        height,
        source_payload_sha256: "3".repeat(64),
        base_mip_decoded_rgba_sha256: "4".repeat(64),
    }
}

fn request(source: DrawSource) -> ImageModeRequest {
    ImageModeRequest {
        schema: REQUEST_SCHEMA.to_string(),
        semantic_role: "ui.option.button.normal".to_string(),
        texture_semantic_id: "ui.option.texture.button.normal".to_string(),
        control_state: ControlState {
            enabled: true,
            on: false,
            interaction: Interaction::Normal,
        },
        target_rect: Rect {
            x: 100.0,
            y: 20.0,
            width: 120.0,
            height: 30.0,
        },
        draw_sources: vec![source],
    }
}

fn style_source(border: RectOffset) -> DrawSource {
    DrawSource::GuiStyleBackground {
        style: GuiStyleEvidence {
            owner: identity("GUISkin", 1386),
            style_name: "button".to_string(),
        },
        selected_state: GuiStyleState::Normal,
        texture: texture(32, 20),
        border,
        overflow: RectOffset::ZERO,
    }
}

#[test]
fn direct_draw_texture_is_stretch_even_when_bitmap_looks_sliceable() {
    let source = DrawSource::DirectDrawTexture {
        overload: DirectDrawOverload::RectTexture,
        scale_mode: DirectScaleMode::StretchToFill,
        texture: texture(32, 20),
    };
    let evidence = build_evidence(request(source)).expect("direct evidence");
    assert_eq!(evidence.decision.image_mode, NativeImageMode::Stretch);
    assert_eq!(
        evidence.decision.reason,
        DecisionReason::DirectDrawTextureStretchToFill
    );
    let Geometry::Stretch { patches, .. } = evidence.geometry else {
        panic!("direct draw must produce stretch geometry");
    };
    assert_eq!(patches.len(), 1);
    assert_eq!(patches[0].role, "full");
}

#[test]
fn gui_style_nonzero_border_builds_exact_nine_slice_grid() {
    let evidence = build_evidence(request(style_source(RectOffset {
        left: 8,
        right: 6,
        top: 5,
        bottom: 4,
    })))
    .expect("nine-slice evidence");
    assert_eq!(evidence.decision.image_mode, NativeImageMode::NineSlice);
    let Geometry::NineSlice {
        source_columns,
        source_rows,
        target_columns,
        target_rows,
        patches,
        ..
    } = evidence.geometry
    else {
        panic!("GUIStyle border must produce nine-slice geometry");
    };
    assert_eq!(source_columns, [0.0, 8.0, 26.0, 32.0]);
    assert_eq!(source_rows, [0.0, 5.0, 16.0, 20.0]);
    assert_eq!(target_columns, [100.0, 108.0, 214.0, 220.0]);
    assert_eq!(target_rows, [20.0, 25.0, 46.0, 50.0]);
    assert_eq!(patches.len(), 9);
    let center = patches
        .iter()
        .find(|patch| patch.role == "center")
        .expect("center patch");
    assert_eq!(center.source_rect, Rect::from_edges(8.0, 5.0, 26.0, 16.0));
    assert_eq!(
        center.target_rect,
        Rect::from_edges(108.0, 25.0, 214.0, 46.0)
    );
    let top = patches
        .iter()
        .find(|patch| patch.role == "top")
        .expect("top edge");
    assert_eq!(top.source_rect, Rect::from_edges(8.0, 0.0, 26.0, 5.0));
    assert_eq!(top.target_rect, Rect::from_edges(108.0, 20.0, 214.0, 25.0));
}

#[test]
fn zero_style_border_is_stretch() {
    let evidence = build_evidence(request(style_source(RectOffset::ZERO))).expect("stretch");
    assert_eq!(evidence.decision.image_mode, NativeImageMode::Stretch);
    assert_eq!(evidence.decision.reason, DecisionReason::GuiStyleZeroBorder);
}

#[test]
fn impossible_source_and_target_borders_fail_closed() {
    let source_error = build_evidence(request(style_source(RectOffset {
        left: 20,
        right: 13,
        top: 0,
        bottom: 0,
    })))
    .expect_err("source width must reject border");
    assert!(source_error.contains("impossible GUIStyle border"));

    let mut target_request = request(style_source(RectOffset {
        left: 8,
        right: 6,
        top: 5,
        bottom: 4,
    }));
    target_request.target_rect.width = 10.0;
    let target_error = build_evidence(target_request).expect_err("small target");
    assert!(target_error.contains("exact 1:1 corner geometry is impossible"));
}

#[test]
fn canonical_source_aliases_and_gui_style_texture_alias_match_are_enforced() {
    let mut noncanonical = DrawSource::DirectDrawTexture {
        overload: DirectDrawOverload::RectTexture,
        scale_mode: DirectScaleMode::StretchToFill,
        texture: texture(32, 20),
    };
    let DrawSource::DirectDrawTexture {
        texture: selected_texture,
        ..
    } = &mut noncanonical
    else {
        unreachable!();
    };
    selected_texture.object.source_alias = "academy".to_string();
    let error = build_evidence(request(noncanonical)).expect_err("raw build alias");
    assert!(error.contains("exactly 'primary', 'patched', or 'alternate'"));
    assert!(error.contains("Academy maps to canonical role 'alternate'"));

    let mut mixed = style_source(RectOffset::ZERO);
    let DrawSource::GuiStyleBackground {
        texture: selected_texture,
        ..
    } = &mut mixed
    else {
        unreachable!();
    };
    selected_texture.object.source_alias = "alternate".to_string();
    let error = build_evidence(request(mixed)).expect_err("mixed style and texture roles");
    assert!(error.contains("must share one canonical source role"));

    let mut unsafe_id = request(DrawSource::DirectDrawTexture {
        overload: DirectDrawOverload::RectTexture,
        scale_mode: DirectScaleMode::StretchToFill,
        texture: texture(32, 20),
    });
    unsafe_id.texture_semantic_id = "../button".to_string();
    let error = build_evidence(unsafe_id).expect_err("unsafe semantic ID");
    assert!(error.contains("textureSemanticId"));
    assert!(error.contains("may not be empty, '.' or '..'"));
}

#[test]
fn every_authority_hash_must_be_lowercase_sha256() {
    let direct = |texture| DrawSource::DirectDrawTexture {
        overload: DirectDrawOverload::RectTexture,
        scale_mode: DirectScaleMode::StretchToFill,
        texture,
    };

    let mut invalid = texture(32, 20);
    invalid.object.raw_container_sha256 = "A".repeat(64);
    let error = build_evidence(request(direct(invalid))).expect_err("uppercase container hash");
    assert!(error.contains("rawContainerSha256"));
    assert!(error.contains("lowercase 64-hex"));

    let mut invalid = texture(32, 20);
    invalid.object.raw_object_sha256 = "2".repeat(63);
    let error = build_evidence(request(direct(invalid))).expect_err("short object hash");
    assert!(error.contains("rawObjectSha256"));

    let mut invalid = texture(32, 20);
    invalid.source_payload_sha256 = "g".repeat(64);
    let error = build_evidence(request(direct(invalid))).expect_err("non-hex payload hash");
    assert!(error.contains("sourcePayloadSha256"));

    let mut invalid = texture(32, 20);
    invalid.base_mip_decoded_rgba_sha256 = "F".repeat(64);
    let error =
        build_evidence(request(direct(invalid))).expect_err("uppercase decoded RGBA hash");
    assert!(error.contains("baseMipDecodedRgbaSha256"));
}

#[test]
fn gui_style_overflow_derives_the_effective_nine_slice_draw_rect() {
    let mut source = style_source(RectOffset {
        left: 8,
        right: 6,
        top: 5,
        bottom: 4,
    });
    let DrawSource::GuiStyleBackground { overflow, .. } = &mut source else {
        unreachable!();
    };
    *overflow = RectOffset {
        left: 3,
        right: 5,
        top: 7,
        bottom: 2,
    };

    let evidence = build_evidence(request(source)).expect("overflow evidence");
    assert_eq!(
        evidence.native_contract.layout_rect,
        Rect {
            x: 100.0,
            y: 20.0,
            width: 120.0,
            height: 30.0,
        }
    );
    assert_eq!(
        evidence.native_contract.effective_draw_rect,
        Rect {
            x: 97.0,
            y: 13.0,
            width: 128.0,
            height: 39.0,
        }
    );
    let Geometry::NineSlice {
        target_columns,
        target_rows,
        ..
    } = evidence.geometry
    else {
        panic!("nonzero border must remain nine-slice");
    };
    assert_eq!(target_columns, [97.0, 105.0, 219.0, 225.0]);
    assert_eq!(target_rows, [13.0, 18.0, 48.0, 52.0]);
}

#[test]
fn invalid_or_overflowing_gui_style_overflow_fails_closed() {
    let mut collapsed = style_source(RectOffset::ZERO);
    let DrawSource::GuiStyleBackground { overflow, .. } = &mut collapsed else {
        unreachable!();
    };
    *overflow = RectOffset {
        left: -61,
        right: -60,
        top: 0,
        bottom: 0,
    };
    let error = build_evidence(request(collapsed)).expect_err("collapsed draw rect");
    assert!(error.contains("effectiveDrawRect derived from GUIStyle overflow"));
    assert!(error.contains("width and height must be positive"));

    let mut overflowing = style_source(RectOffset::ZERO);
    let DrawSource::GuiStyleBackground { overflow, .. } = &mut overflowing else {
        unreachable!();
    };
    *overflow = RectOffset {
        left: i64::MAX,
        right: 1,
        top: 0,
        bottom: 0,
    };
    let error = build_evidence(request(overflowing)).expect_err("overflow sum");
    assert!(error.contains("horizontal RectOffset overflow sum overflows"));
}
#[test]
fn more_than_one_draw_source_is_rejected_as_ambiguous() {
    let direct = DrawSource::DirectDrawTexture {
        overload: DirectDrawOverload::RectTexture,
        scale_mode: DirectScaleMode::StretchToFill,
        texture: texture(32, 20),
    };
    let mut request = request(direct.clone());
    request.draw_sources.push(style_source(RectOffset::ZERO));
    let error = build_evidence(request).expect_err("ambiguous source");
    assert!(error.contains("draw source is ambiguous"));
    assert!(error.contains("directDrawTexture, guiStyleBackground"));
}

#[test]
fn state_axes_select_hover_on_and_disabled_style_states() {
    let hover = resolve_control_state(ControlState {
        enabled: true,
        on: false,
        interaction: Interaction::Hover,
    });
    assert_eq!(hover.canonical_state, "hover");
    assert_eq!(hover.expected_gui_style_state, GuiStyleState::Hover);

    let on_active = resolve_control_state(ControlState {
        enabled: true,
        on: true,
        interaction: Interaction::Active,
    });
    assert_eq!(on_active.canonical_state, "onActive");
    assert_eq!(on_active.expected_gui_style_state, GuiStyleState::OnActive);

    let disabled_hover = resolve_control_state(ControlState {
        enabled: false,
        on: false,
        interaction: Interaction::Hover,
    });
    assert_eq!(disabled_hover.canonical_state, "disabled");
    assert_eq!(
        disabled_hover.expected_gui_style_state,
        GuiStyleState::Normal
    );
}

#[test]
fn mismatched_gui_style_state_is_rejected() {
    let mut request = request(style_source(RectOffset::ZERO));
    request.control_state.interaction = Interaction::Hover;
    let error = build_evidence(request).expect_err("state mismatch");
    assert!(error.contains("contradicts controlState"));
    assert!(error.contains("Hover"));
}

#[test]
fn cli_round_trip_writes_versioned_source_neutral_native_contract() {
    let root = tempdir().expect("tempdir");
    let input = root.path().join("request.json");
    let output = root.path().join("reports").join("ui").join("evidence.json");
    let fixture = json!({
        "schema": REQUEST_SCHEMA,
        "semanticRole": "ui.barker.box",
        "textureSemanticId": "ui.barker.texture.box.normal",
        "controlState": {"enabled": true, "on": false, "interaction": "normal"},
        "targetRect": {"x": 10.0, "y": 20.0, "width": 200.0, "height": 80.0},
        "drawSources": [{
            "kind": "guiStyleBackground",
            "style": {
                "owner": {
                    "sourceAlias": "primary",
                    "relativeContainer": "StandaloneWindows/barker.resourceFile",
                    "rawContainerSha256": "1".repeat(64),
                    "serializedAsset": "sharedassets0.assets",
                    "type": "GUISkin",
                    "pathId": 1386,
                    "rawObjectSha256": "2".repeat(64)
                },
                "styleName": "bbBox"
            },
            "selectedState": "normal",
            "texture": {
                "object": {
                    "sourceAlias": "primary",
                    "relativeContainer": "StandaloneWindows/barker.resourceFile",
                    "rawContainerSha256": "1".repeat(64),
                    "serializedAsset": "sharedassets0.assets",
                    "type": "Texture2D",
                    "pathId": 236,
                    "rawObjectSha256": "2".repeat(64)
                },
                "width": 21,
                "height": 19,
                "sourcePayloadSha256": "3".repeat(64),
                "baseMipDecodedRgbaSha256": "4".repeat(64)
            },
            "border": {"left": 6, "right": 6, "top": 6, "bottom": 6},
            "overflow": {"left": 0, "right": 0, "top": 0, "bottom": 0}
        }]
    });
    let request_bytes = serde_json::to_vec_pretty(&fixture).unwrap();
    fs::write(&input, &request_bytes).expect("request");

    export_ui_image_mode_evidence_cli(&[
        input.to_string_lossy().to_string(),
        "--out".to_string(),
        output.to_string_lossy().to_string(),
    ])
    .expect("CLI export");

    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("evidence")).expect("JSON");
    assert_eq!(document["schema"], EVIDENCE_SCHEMA);
    assert_eq!(document["schemaVersion"], 1);
    assert_eq!(document["decision"]["imageMode"], "nineSlice");
    assert_eq!(document["nativeContract"]["schema"], NATIVE_CONTRACT_SCHEMA);
    assert_eq!(document["nativeContract"]["semanticRole"], "ui.barker.box");
    assert_eq!(
        document["nativeContract"]["textureSemanticId"],
        "ui.barker.texture.box.normal"
    );
    assert_eq!(document["nativeContract"]["contractScope"], "geometryOnly");
    assert_eq!(
        document["nativeContract"]["requiredExternalContracts"],
        json!([
            "texturePayload",
            "sampler",
            "decodedImageOrientation",
            "colorAlphaContract"
        ])
    );
    assert_eq!(
        document["nativeContract"]["layoutRect"],
        json!({"x": 10.0, "y": 20.0, "width": 200.0, "height": 80.0})
    );
    assert_eq!(
        document["nativeContract"]["effectiveDrawRect"],
        document["nativeContract"]["layoutRect"]
    );
    assert!(document["nativeContract"].get("drawSource").is_none());
    assert_eq!(document["requestSha256"], sha256_hex(&request_bytes));
    assert_eq!(
        document["drawSource"]["style"]["owner"]["rawContainerSha256"],
        "1".repeat(64)
    );
    assert_eq!(
        document["drawSource"]["style"]["owner"]["rawObjectSha256"],
        "2".repeat(64)
    );
    assert_eq!(
        document["drawSource"]["texture"]["sourcePayloadSha256"],
        "3".repeat(64)
    );
    assert_eq!(
        document["drawSource"]["texture"]["baseMipDecodedRgbaSha256"],
        "4".repeat(64)
    );
    assert_eq!(
        document["drawSource"]["overflow"],
        json!({"left": 0, "right": 0, "top": 0, "bottom": 0})
    );
    let native_json = serde_json::to_string(&document["nativeContract"]).unwrap();
    for forbidden in [
        "sourceAlias",
        "pathId",
        "rawContainerSha256",
        "rawObjectSha256",
        "sourcePayloadSha256",
        "baseMipDecodedRgbaSha256",
    ] {
        assert!(
            !native_json.contains(forbidden),
            "{forbidden} must remain evidence-only"
        );
    }
}
#[test]
fn lexically_equivalent_nonexistent_output_path_is_rejected() {
    let error = parse_cli(&[
        "request.json".to_string(),
        "--out".to_string(),
        "./request.json".to_string(),
    ])
    .expect_err("lexically identical path");
    assert!(error.contains("must be different files"));
    assert!(same_path(
        Path::new("missing/../request.json"),
        Path::new("./request.json")
    )
    .expect("lexical comparison"));
    #[cfg(windows)]
    assert!(same_path(Path::new("REQUEST.JSON"), Path::new("request.json")).unwrap());
}

#[test]
fn unsupported_direct_scale_modes_are_not_misreported_as_stretch() {
    let source = DrawSource::DirectDrawTexture {
        overload: DirectDrawOverload::RectTextureScaleMode,
        scale_mode: DirectScaleMode::ScaleToFit,
        texture: texture(32, 20),
    };
    let error = build_evidence(request(source)).expect_err("unsupported scale mode");
    assert!(error.contains("not a stretch/nine-slice contract"));
}
