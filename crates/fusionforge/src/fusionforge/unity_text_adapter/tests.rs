use super::*;
use serde_json::json;
use tempfile::tempdir;

pub(super) fn base_input_value() -> serde_json::Value {
    json!({
        "schema": UNITY_TEXT_ADAPTER_INPUT_SCHEMA,
        "measurementSpace": "referenceCanvasPixelsAfterGlyphRenderScale",
        "evidence": {
            "legacyFont": {
                "source": {
                    "alias": "retrobution",
                    "canonicalRole": "primary",
                    "relativeContainer": "main.unity3d",
                    "rawContainerSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                },
                "object": {
                    "semanticId": "legacy.jeffe.16",
                    "serializedAsset": "sharedassets0.assets",
                    "unityType": "Font",
                    "pathId": 903,
                    "rawObjectSha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                }
            },
            "replacementFont": {
                "semanticId": "ffone.font.jeffe",
                "bytesSha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
            },
            "measurement": {
                "producer": "fusionforge.ui-text-metrics",
                "version": "1.0.0",
                "captureSha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
            }
        },
        "referenceCanvas": {"width": 1280.0, "height": 720.0},
        "style": {
            "semanticId": "option.title.jeffe16",
            "rect": {"x": 100.0, "y": 50.0, "width": 200.0, "height": 40.0},
            "padding": {"left": 10.0, "right": 10.0, "top": 4.0, "bottom": 6.0},
            "contentOffset": {"x": 2.0, "y": -1.0},
            "alignment": "middleCenter",
            "wordWrap": false,
            "clipping": "clip"
        },
        "legacyFont": {
            "semanticId": "legacy.jeffe.16",
            "fontSizePx": 16.0,
            "lineHeightPx": 13.56,
            "baselineFromLineTopPx": 10.0,
            "renderScale": {"x": 1.0, "y": 1.0},
            "ascentPx": 10.0,
            "descentPx": 3.56
        },
        "replacementFont": {
            "semanticId": "ffone.font.jeffe",
            "fontSizePx": 11.0,
            "lineHeightPx": 13.56,
            "baselineFromLineTopPx": 12.0,
            "renderScale": {"x": 1.0, "y": 0.70},
            "ascentPx": 10.5,
            "descentPx": 3.06
        },
        "samples": [{
            "id": "english.title",
            "locale": "en",
            "text": "SETTINGS MENU",
            "legacy": {
                "layoutWidthPx": 100.0,
                "layoutHeightPx": 14.0,
                "advanceWidthPx": 100.0,
                "inkBoundsPx": {"left": 1.0, "top": 2.0, "right": 99.0, "bottom": 13.0},
                "lineCount": 1,
                "unicodeScalarBreaks": []
            },
            "replacement": {
                "layoutWidthPx": 94.0,
                "layoutHeightPx": 16.0,
                "advanceWidthPx": 94.0,
                "inkBoundsPx": {"left": -1.0, "top": 3.0, "right": 93.0, "bottom": 15.0},
                "lineCount": 1,
                "unicodeScalarBreaks": []
            }
        }],
        "acceptance": {
            "maxTranslationResidualPx": 0.0,
            "maxBaselineResidualPx": 2.0,
            "maxAdvanceDeltaPx": 6.0,
            "maxInkSizeDeltaPx": 4.0,
            "maxWrapWidthResidualPx": 0.0
        }
    })
}

fn base_input() -> AdapterInput {
    let mut input: AdapterInput = serde_json::from_value(base_input_value()).expect("valid fixture");
    let mut russian = input.samples[0].clone();
    russian.id = "russian.title".to_string();
    russian.locale = SampleLocale::Ru;
    russian.text = "НАСТРОЙКИ".to_string();
    input.samples.push(russian);
    input
}

#[test]
fn middle_center_jeffe_adapter_preserves_content_rect_and_ink_anchor() {
    let report =
        build_report(base_input(), "fixture-hash".to_string(), false).expect("adapter report");

    assert_eq!(report.acceptance.status, "passed");
    assert_eq!(report.schema, UNITY_TEXT_ADAPTER_OUTPUT_SCHEMA);
    assert_eq!(
        report.measurement_space,
        MeasurementSpace::ReferenceCanvasPixelsAfterGlyphRenderScale
    );
    assert_eq!(
        report.native_contract.schema,
        NATIVE_TEXT_REPLACEMENT_CONTRACT_SCHEMA
    );
    assert_eq!(
        report.native_contract.style_semantic_id,
        "option.title.jeffe16"
    );
    assert_eq!(
        report.native_contract.reference_canvas,
        SizePx {
            width: 1280.0,
            height: 720.0
        }
    );
    assert!(matches!(
        report.native_contract.coordinate_convention,
        CoordinateConvention::TopLeftOriginXRightYDown
    ));
    assert_eq!(
        report.native_contract.measurement_space,
        MeasurementSpace::ReferenceCanvasPixelsAfterGlyphRenderScale
    );
    assert_eq!(
        report.native_contract.replacement_glyph_render_scale,
        PointPx { x: 1.0, y: 0.7 }
    );
    assert!(matches!(
        report
            .native_contract
            .scale_contract
            .glyph_render_scale_application,
        GlyphRenderScaleApplication::ApplyExactlyOnceToGlyphRasterBeforeMeasurement
    ));
    assert!(matches!(
        report.native_contract.scale_contract.line_height,
        PostScaleMetricPolicy::AlreadyInMeasurementSpaceDoNotScale
    ));
    assert!(matches!(
        report.native_contract.scale_contract.offsets,
        PostScaleMetricPolicy::AlreadyInMeasurementSpaceDoNotScale
    ));
    assert!(matches!(
        report.native_contract.scale_contract.wrap_width,
        PostScaleMetricPolicy::AlreadyInMeasurementSpaceDoNotScale
    ));
    assert!(matches!(
        report.native_contract.scale_contract.measurements,
        PostScaleMetricPolicy::AlreadyInMeasurementSpaceDoNotScale
    ));
    assert!(report.native_contract.preserve_source_clip_rect);
    assert_eq!(report.samples[0].locale, SampleLocale::En);
    assert_eq!(report.samples[1].locale, SampleLocale::Ru);
    assert_eq!(report.samples[0].unicode_scalar_count, 13);
    assert_eq!(
        report.samples[0].text_sha256,
        "9828d0cb73505d073b9195b81610ffe7a8375668d584a1f9a2a46ce9cbd36cc4"
    );
    assert_eq!(
        report.style.content_rect_px,
        RectPx {
            x: 110.0,
            y: 54.0,
            width: 180.0,
            height: 30.0
        }
    );
    assert_eq!(
        report.native_contract.replacement_metric_compensation_px,
        PointPx { x: 1.0, y: -0.5 }
    );
    assert_eq!(
        report
            .native_contract
            .final_text_offset_from_aligned_content_px,
        PointPx { x: 3.0, y: -1.5 }
    );
    assert_eq!(report.compensation.baseline_delta_px, -1.0);
    assert_eq!(report.compensation.advance_delta_px, -6.0);
    assert_eq!(report.compensation.ink_width_delta_px, -4.0);
    assert_eq!(report.compensation.ink_height_delta_px, 1.0);

    let native_json = serde_json::to_value(&report.native_contract).expect("native contract");
    assert!(native_json.get("evidenceInputs").is_none());
    assert!(native_json.get("legacyFont").is_none());
    assert!(native_json.get("inputSha256").is_none());

    let mut overflow_input = base_input();
    assert!(native_json.get("contentOffsetPx").is_some());
    assert!(native_json.get("unityContentOffsetPx").is_none());
    let native_text = serde_json::to_string(&report.native_contract).expect("native JSON");
    for source_token in [
        "retrobution",
        "academy",
        "sharedassets0.assets",
        "main.unity3d",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert!(!native_text.contains(source_token));
    }
    overflow_input.style.clipping = TextClipping::Overflow;
    let overflow_report =
        build_report(overflow_input, "overflow-fixture-hash".to_string(), false)
            .expect("overflow adapter report");
    assert!(!overflow_report.native_contract.preserve_source_clip_rect);
}

#[test]
fn wrap_candidates_produce_a_measured_consensus_width() {
    let mut input = base_input();
    input.style.word_wrap = true;
    input.style.alignment = TextAlignment::UpperLeft;
    input.style.rect.width = 120.0;
    input.style.padding.left = 10.0;
    input.style.padding.right = 10.0;
    for sample in &mut input.samples {
        sample.legacy.line_count = 2;
        sample.legacy.unicode_scalar_breaks = vec![6];
        sample.replacement.line_count = 2;
        sample.replacement.unicode_scalar_breaks = vec![5];
        sample.replacement_wrap_candidates = vec![
            WrapCandidate {
                width_px: 96.0,
                unicode_scalar_breaks: vec![6],
            },
            WrapCandidate {
                width_px: 104.0,
                unicode_scalar_breaks: vec![6],
            },
        ];
    }
    input.acceptance.max_wrap_width_residual_px = 0.0;

    let report = build_report(input, "fixture-hash".to_string(), false).expect("wrap report");
    assert_eq!(report.native_contract.wrap_layout_width_px, Some(96.0));
    assert_eq!(report.compensation.wrap_width_delta_px, Some(-4.0));
    assert_eq!(report.samples[0].selected_wrap_width_px, Some(96.0));
}

#[test]
fn missing_wrap_parity_fails_closed_unless_triage_is_explicit() {
    let mut input = base_input();
    input.style.word_wrap = true;
    for sample in &mut input.samples {
        sample.legacy.line_count = 2;
        sample.legacy.unicode_scalar_breaks = vec![6];
        sample.replacement.line_count = 2;
        sample.replacement.unicode_scalar_breaks = vec![5];
    }

    let report = build_report(input, "fixture-hash".to_string(), true).expect("triage report");
    assert_eq!(report.acceptance.status, "failed");
    assert!(report.acceptance.failures[0].contains("no measured replacement width"));
    assert!(report.triage.allow_unsatisfied);
}

#[test]
fn invalid_padding_and_machine_path_identifiers_are_rejected() {
    let mut invalid_padding = base_input();
    invalid_padding.style.padding.left = 120.0;
    invalid_padding.style.padding.right = 120.0;
    assert!(build_report(invalid_padding, "fixture".to_string(), false)
        .unwrap_err()
        .contains("style content rect after padding.width"));

    let mut machine_path = base_input();
    machine_path.replacement_font.semantic_id = "D:\\fonts\\jeffe.otf".to_string();
    assert!(build_report(machine_path, "fixture".to_string(), false)
        .unwrap_err()
        .contains("not a machine path"));

    let mut empty_text = base_input();
    empty_text.samples[0].text.clear();
    assert!(build_report(empty_text, "fixture".to_string(), false)
        .unwrap_err()
        .contains("text must not be empty"));

    let mut whitespace_text = base_input();
    whitespace_text.samples[0].text = " \t ".to_string();
    assert!(build_report(whitespace_text, "fixture".to_string(), false)
        .unwrap_err()
        .contains("text must contain a visible character"));

    let mut control_text = base_input();
    control_text.samples[0].text = "BAD\0TEXT".to_string();
    assert!(build_report(control_text, "fixture".to_string(), false)
        .unwrap_err()
        .contains("unsupported control U+0000"));
}

#[test]
fn explicit_lf_is_retained_without_word_wrap_and_locales_are_required() {
    let mut input = base_input();
    input.samples[0].text = "SETTINGS\nMENU".to_string();
    input.samples[0].legacy.line_count = 2;
    input.samples[0].legacy.unicode_scalar_breaks = vec![9];
    input.samples[0].replacement.line_count = 2;
    input.samples[0].replacement.unicode_scalar_breaks = vec![9];

    let report = build_report(input.clone(), "lf-fixture".to_string(), false)
        .expect("explicit LF is not automatic word wrapping");
    assert!(!report.native_contract.word_wrap);
    assert_eq!(report.samples[0].unicode_scalar_count, 13);
    assert_eq!(report.samples[0].legacy_unicode_scalar_breaks, vec![9]);
    assert_eq!(report.samples[0].replacement_unicode_scalar_breaks, vec![9]);

    let mut omitted_explicit_break = input;
    omitted_explicit_break.samples[0].legacy.line_count = 1;
    omitted_explicit_break.samples[0]
        .legacy
        .unicode_scalar_breaks
        .clear();
    omitted_explicit_break.samples[0].replacement.line_count = 1;
    omitted_explicit_break.samples[0]
        .replacement
        .unicode_scalar_breaks
        .clear();
    assert!(build_report(
        omitted_explicit_break,
        "missing-lf-break".to_string(),
        false
    )
    .unwrap_err()
    .contains("must retain explicit LF break"));

    let mut missing_russian = base_input();
    missing_russian
        .samples
        .retain(|sample| sample.locale == SampleLocale::En);
    assert!(
        build_report(missing_russian, "missing-ru".to_string(), false)
            .unwrap_err()
            .contains("at least one locale=en and one locale=ru")
    );
}

#[test]
fn evidence_identity_is_portable_bound_and_role_checked() {
    let report = build_report(base_input(), "evidence-fixture".to_string(), false)
        .expect("bound evidence");
    assert_eq!(
        report.evidence_inputs.legacy_font.object.semantic_id,
        report.legacy_font.semantic_id
    );
    assert_eq!(
        report.evidence_inputs.replacement_font.semantic_id,
        report.replacement_font.semantic_id
    );
    assert_eq!(
        report
            .evidence_inputs
            .legacy_font
            .source
            .raw_container_sha256,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(
        report.evidence_inputs.legacy_font.object.raw_object_sha256,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    assert_eq!(
        report.evidence_inputs.measurement.producer,
        "fusionforge.ui-text-metrics"
    );

    let mut academy = base_input();
    academy.evidence.legacy_font.source.alias = LegacySourceAlias::Academy;
    academy.evidence.legacy_font.source.canonical_role = CanonicalSourceRole::Alternate;
    build_report(academy, "academy-fixture".to_string(), false)
        .expect("academy alternate role");

    let mut bad_role = base_input();
    bad_role.evidence.legacy_font.source.canonical_role = CanonicalSourceRole::Alternate;
    assert!(build_report(bad_role, "bad-role".to_string(), false)
        .unwrap_err()
        .contains("retrobution must use canonicalRole primary"));

    let mut bad_container_hash = base_input();
    bad_container_hash
        .evidence
        .legacy_font
        .source
        .raw_container_sha256 = "A".repeat(64);
    assert!(
        build_report(bad_container_hash, "bad-hash".to_string(), false)
            .unwrap_err()
            .contains("64 lowercase hexadecimal")
    );

    let mut bad_replacement_hash = base_input();
    bad_replacement_hash.evidence.replacement_font.bytes_sha256 = "abc".to_string();
    assert!(build_report(
        bad_replacement_hash,
        "bad-replacement-hash".to_string(),
        false
    )
    .unwrap_err()
    .contains("64 lowercase hexadecimal"));

    let mut bad_object_hash = base_input();
    bad_object_hash
        .evidence
        .legacy_font
        .object
        .raw_object_sha256 = "b".repeat(63);
    assert!(
        build_report(bad_object_hash, "bad-object-hash".to_string(), false)
            .unwrap_err()
            .contains("64 lowercase hexadecimal")
    );

    let mut bad_capture_hash = base_input();
    bad_capture_hash.evidence.measurement.capture_sha256 = "d".repeat(65);
    assert!(
        build_report(bad_capture_hash, "bad-capture-hash".to_string(), false)
            .unwrap_err()
            .contains("64 lowercase hexadecimal")
    );

    let mut bad_measurement_version = base_input();
    bad_measurement_version.evidence.measurement.version = "1/0".to_string();
    assert!(build_report(
        bad_measurement_version,
        "bad-measurement-version".to_string(),
        false
    )
    .unwrap_err()
    .contains("portable version characters"));

    let mut traversal = base_input();
    traversal.evidence.legacy_font.source.relative_container =
        "bundles/../main.unity3d".to_string();
    assert!(build_report(traversal, "traversal".to_string(), false)
        .unwrap_err()
        .contains("portable relative path without traversal"));

    let mut mixed_identity = base_input();
    mixed_identity.evidence.replacement_font.semantic_id = "ffone.font.other".to_string();
    assert!(build_report(mixed_identity, "mixed".to_string(), false)
        .unwrap_err()
        .contains("must match replacementFont.semanticId"));

    let mut unknown_alias = base_input_for_json();
    unknown_alias["evidence"]["legacyFont"]["source"]["alias"] = json!("retribution");
    let error = serde_json::from_value::<AdapterInput>(unknown_alias).unwrap_err();
    assert!(error.to_string().contains("unknown variant"));
}

#[test]
fn strict_json_rejects_unknown_fields() {
    let mut value = serde_json::to_value(base_input_for_json()).expect("serialize fixture");
    value
        .as_object_mut()
        .expect("object")
        .insert("unexpected".to_string(), json!(true));
    let error = serde_json::from_value::<AdapterInput>(value).unwrap_err();
    assert!(error.to_string().contains("unknown field"));

    let mut missing_space = base_input_for_json();
    missing_space
        .as_object_mut()
        .expect("object")
        .remove("measurementSpace");
    let error = serde_json::from_value::<AdapterInput>(missing_space).unwrap_err();
    assert!(error
        .to_string()
        .contains("missing field `measurementSpace`"));

    let mut wrong_space = base_input_for_json();
    wrong_space["measurementSpace"] = json!("referenceCanvasPixelsBeforeGlyphRenderScale");
    let error = serde_json::from_value::<AdapterInput>(wrong_space).unwrap_err();
    assert!(error.to_string().contains("unknown variant"));

    let mut duplicate_count = base_input_for_json();
    duplicate_count["samples"][0]
        .as_object_mut()
        .expect("sample")
        .insert("unicodeScalarCount".to_string(), json!(4));
    let error = serde_json::from_value::<AdapterInput>(duplicate_count).unwrap_err();
    assert!(error.to_string().contains("unknown field"));
}

fn base_input_for_json() -> serde_json::Value {
    let mut value = json!({
        "schema": UNITY_TEXT_ADAPTER_INPUT_SCHEMA,
        "measurementSpace": "referenceCanvasPixelsAfterGlyphRenderScale",
        "evidence": {
            "legacyFont": {
                "source": {
                    "alias": "retrobution",
                    "canonicalRole": "primary",
                    "relativeContainer": "main.unity3d",
                    "rawContainerSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                },
                "object": {
                    "semanticId": "legacy.jeffe.14",
                    "serializedAsset": "sharedassets0.assets",
                    "unityType": "Font",
                    "pathId": 903,
                    "rawObjectSha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                }
            },
            "replacementFont": {
                "semanticId": "ffone.font.jeffe",
                "bytesSha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
            },
            "measurement": {
                "producer": "fusionforge.ui-text-metrics",
                "version": "1.0.0",
                "captureSha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
            }
        },
        "referenceCanvas": {"width": 100.0, "height": 100.0},
        "style": {
            "semanticId": "test.style",
            "rect": {"x": 0.0, "y": 0.0, "width": 50.0, "height": 20.0},
            "padding": {"left": 0.0, "right": 0.0, "top": 0.0, "bottom": 0.0},
            "contentOffset": {"x": 0.0, "y": 0.0},
            "alignment": "upperLeft",
            "wordWrap": false,
            "clipping": "clip"
        },
        "legacyFont": {
            "semanticId": "legacy.jeffe.14",
            "fontSizePx": 14.0,
            "lineHeightPx": 13.71,
            "baselineFromLineTopPx": 10.0,
            "renderScale": {"x": 1.0, "y": 1.0},
            "ascentPx": 10.0,
            "descentPx": 3.0
        },
        "replacementFont": {
            "semanticId": "ffone.font.jeffe",
            "fontSizePx": 12.0,
            "lineHeightPx": 13.71,
            "baselineFromLineTopPx": 10.0,
            "renderScale": {"x": 1.0, "y": 0.70},
            "ascentPx": 10.0,
            "descentPx": 3.0
        },
        "samples": [{
            "id": "sample",
            "locale": "en",
            "text": "TEST",
            "legacy": {
                "layoutWidthPx": 20.0,
                "layoutHeightPx": 14.0,
                "advanceWidthPx": 20.0,
                "inkBoundsPx": {"left": 0.0, "top": 0.0, "right": 20.0, "bottom": 12.0},
                "lineCount": 1,
                "unicodeScalarBreaks": []
            },
            "replacement": {
                "layoutWidthPx": 20.0,
                "layoutHeightPx": 14.0,
                "advanceWidthPx": 20.0,
                "inkBoundsPx": {"left": 0.0, "top": 0.0, "right": 20.0, "bottom": 12.0},
                "lineCount": 1,
                "unicodeScalarBreaks": []
            }
        }],
        "acceptance": {
            "maxTranslationResidualPx": 0.0,
            "maxBaselineResidualPx": 0.0,
            "maxAdvanceDeltaPx": 0.0,
            "maxInkSizeDeltaPx": 0.0,
            "maxWrapWidthResidualPx": 0.0
        }
    });
    let mut russian = value["samples"][0].clone();
    russian["id"] = json!("sample.ru");
    russian["locale"] = json!("ru");
    russian["text"] = json!("ТЕСТ");
    value["samples"]
        .as_array_mut()
        .expect("samples")
        .push(russian);
    value
}

#[test]
fn cli_rejects_lexical_output_aliases_of_input() {
    let directory = tempdir().expect("tempdir");
    let input_path = directory.path().join("input.json");
    fs::write(
        &input_path,
        serde_json::to_vec_pretty(&base_input_for_json()).expect("input JSON"),
    )
    .expect("write input");
    let original = fs::read(&input_path).expect("original input");

    for output_alias in [
        directory.path().join(".").join("input.json"),
        directory
            .path()
            .join("missing")
            .join("..")
            .join("input.json"),
    ] {
        let error = adapt_unity_text_cli(&[
            input_path.to_string_lossy().to_string(),
            "--out".to_string(),
            output_alias.to_string_lossy().to_string(),
        ])
        .expect_err("lexical alias must be rejected");
        assert!(error.contains("must not overwrite the input JSON"));
        assert_eq!(
            fs::read(&input_path).expect("input after rejected output"),
            original
        );
    }
}

#[test]
fn cli_never_embeds_input_or_output_machine_paths() {
    let directory = tempdir().expect("tempdir");
    let input_path = directory.path().join("jeffe-input.json");
    let output_path = directory.path().join("jeffe-report.json");
    let input_bytes = serde_json::to_vec_pretty(&base_input_for_json()).expect("input JSON");
    let expected_input_sha256 = format!("{:x}", Sha256::digest(&input_bytes));
    fs::write(&input_path, input_bytes).expect("write input");

    adapt_unity_text_cli(&[
        input_path.to_string_lossy().to_string(),
        "--out".to_string(),
        output_path.to_string_lossy().to_string(),
    ])
    .expect("CLI conversion");

    let output = fs::read_to_string(output_path).expect("report");
    assert!(output.contains(UNITY_TEXT_ADAPTER_OUTPUT_SCHEMA));
    let report: serde_json::Value = serde_json::from_str(&output).expect("report JSON");
    assert_eq!(report["inputSha256"], expected_input_sha256);
    assert_eq!(report["samples"][0]["unicodeScalarCount"], 4);
    assert_eq!(
        report["samples"][0]["textSha256"],
        "94ee059335e587e501cc4bf90613e0814f00a7b08bc7c648fd865a2af6a22cc2"
    );
    assert!(report["samples"][0].get("text").is_none());
    assert!(!output.contains(&directory.path().to_string_lossy().to_string()));
    assert!(!output.contains("jeffe-input.json"));
    assert!(!output.contains("jeffe-report.json"));
}
