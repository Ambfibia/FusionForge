//! Bind final metric captures to the actual source and native consumer in memory.
use super::*;
use ffone_ui_layout::quit_menu::QuitMenuDocument;

pub(in super::super) fn calibrate_quit_menu(
    document: &mut QuitMenuDocument,
    bytes: &[u8],
    source_hash: &str,
    font_id: i64,
    object_hash: &str,
    replacement_hash: &str,
    output: &Path,
) -> Result<(), String> {
    // One adapter input per serialized style, in document style order.
    // No report file or staging tree is needed between measurement and conversion.
    let inputs: [AdapterInput; 2] = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let locales = ["en", "ru"].map(|locale| {
        fs::read(output.join(format!("localization/{locale}.json")))
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| e.to_string())
            })
    });
    let [en, ru] = locales;
    let locales = [en?, ru?];
    let mut compensated = document.clone();
    for (index, input) in inputs.into_iter().enumerate() {
        let style = &document.styles[index];
        let evidence = &input.evidence;
        if evidence.legacy_font.source.raw_container_sha256 != source_hash
            || evidence.legacy_font.source.relative_container != "main.unity3d"
            || evidence.legacy_font.source.canonical_role != CanonicalSourceRole::Primary
            || evidence.legacy_font.object.path_id != font_id
            || evidence.legacy_font.object.serialized_asset != "sharedassets0.assets"
            || evidence.legacy_font.object.raw_object_sha256 != object_hash
            || evidence.replacement_font.bytes_sha256 != replacement_hash
            || input.replacement_font.semantic_id != "ffone.font.jeffe"
        {
            return Err(format!(
                "quit style {index}: metric capture does not match source/font bytes"
            ));
        }
        let first = document
            .buttons
            .iter()
            .find(|b| b.style == index)
            .ok_or("unused quit style")?;
        let s = &input.style;
        let near = |a: f64, b: f32| (a - f64::from(b)).abs() <= 0.0001;
        if !near(s.rect.width, first.rect[2])
            || !near(s.rect.height, first.rect[3])
            || ![
                s.padding.left,
                s.padding.right,
                s.padding.top,
                s.padding.bottom,
            ]
            .into_iter()
            .zip(style.padding)
            .all(|(a, b)| near(a, b))
            || !near(s.content_offset.x, style.content_offset[0])
            || !near(s.content_offset.y, style.content_offset[1])
            || !matches!(s.alignment, TextAlignment::MiddleCenter)
            || s.word_wrap != style.word_wrap
            || matches!(s.clipping, TextClipping::Clip) != style.clips_text
            || !near(input.replacement_font.font_size_px, document.font_size)
            || !near(input.replacement_font.line_height_px, document.line_height)
            || input.replacement_font.render_scale != (PointPx { x: 1.0, y: 1.0 })
        {
            return Err(format!(
                "quit style {index}: metric capture differs from native layout"
            ));
        }
        for button in document.buttons.iter().filter(|b| b.style == index) {
            for (locale, bundle) in [SampleLocale::En, SampleLocale::Ru]
                .into_iter()
                .zip(&locales)
            {
                let text = bundle["entries"][&button.localization_key]
                    .as_str()
                    .ok_or("missing calibration label")?;
                if !input
                    .samples
                    .iter()
                    .any(|s| s.locale == locale && s.text == text)
                {
                    return Err(format!(
                        "quit style {index}: missing measured label {} in {locale:?}",
                        button.localization_key
                    ));
                }
            }
        }
        let report = build_report(input, format!("{:x}", Sha256::digest(bytes)), false)?;
        if report.acceptance.status != "passed" {
            return Err(format!(
                "quit style {index}: {}",
                report.acceptance.failures.join("; ")
            ));
        }
        let contract = report.native_contract;
        if contract
            .wrap_layout_width_px
            .is_some_and(|w| (w - contract.content_rect_px.width).abs() > 0.0001)
        {
            return Err(
                "quit-menu consumer does not support a separate calibrated wrap width".into(),
            );
        }
        let offset = contract.replacement_metric_compensation_px;
        compensated.styles[index].font_compensation = [offset.x as f32, offset.y as f32];
    }
    compensated.validate()?;
    *document = compensated;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn conversion_binds_capture_and_applies_compensation_once_without_geometry_changes() {
        let mut document = QuitMenuDocument::from_json(include_bytes!(
            "../../../../../../FFOneClient/assets/game/ui/en/gameplay/quit-menu/menu.ffquit.json"
        ))
        .unwrap();
        let original = document.clone();
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("localization")).unwrap();
        let entries = document
            .buttons
            .iter()
            .map(|b| (b.localization_key.clone(), json!(b.fallback)))
            .collect::<serde_json::Map<_, _>>();
        for locale in ["en", "ru"] {
            fs::write(
                root.path().join(format!("localization/{locale}.json")),
                serde_json::to_vec(&json!({"entries": entries})).unwrap(),
            )
            .unwrap();
        }
        let mut capture = Vec::new();
        for (index, style) in document.styles.iter().enumerate() {
            let mut input = super::super::tests::base_input_value();
            let [left, right, top, bottom] = style.padding;
            input["style"]["rect"] = json!({"x":0,"y":0,"width":175,"height":45});
            input["style"]["padding"] =
                json!({"left":left,"right":right,"top":top,"bottom":bottom});
            input["style"]["contentOffset"] = json!({"x":0,"y":0});
            input["style"]["wordWrap"] = json!(style.word_wrap);
            input["style"]["clipping"] = json!(if style.clips_text { "clip" } else { "overflow" });
            input["replacementFont"]["fontSizePx"] = json!(document.font_size);
            input["replacementFont"]["lineHeightPx"] = json!(document.line_height);
            input["replacementFont"]["renderScale"] = json!({"x":1,"y":1});
            input["evidence"]["legacyFont"]["object"]["pathId"] = json!(903);
            let mut samples = Vec::new();
            for button in document.buttons.iter().filter(|b| b.style == index) {
                for locale in ["en", "ru"] {
                    let mut sample = input["samples"][0].clone();
                    sample["id"] = json!(format!("{}.{locale}", button.localization_key));
                    sample["locale"] = json!(locale);
                    sample["text"] = json!(button.fallback);
                    samples.push(sample);
                }
            }
            input["samples"] = json!(samples);
            capture.push(input);
        }
        let apply = |document: &mut QuitMenuDocument, capture: &[serde_json::Value]| {
            calibrate_quit_menu(
                document,
                &serde_json::to_vec(capture).unwrap(),
                &"a".repeat(64),
                903,
                &"b".repeat(64),
                &"c".repeat(64),
                root.path(),
            )
        };
        apply(&mut document, &capture).unwrap();
        assert_eq!(document.styles[0].font_compensation, [1.0, -0.5]);
        assert_eq!(document.buttons, original.buttons);
        assert_eq!(
            document.styles[0].content_offset,
            original.styles[0].content_offset
        );
        let once = document.clone();
        apply(&mut document, &capture).unwrap();
        assert_eq!(document, once);
        capture[1]["evidence"]["replacementFont"]["bytesSha256"] = json!("e".repeat(64));
        assert!(apply(&mut document, &capture).is_err());
        assert_eq!(
            document, once,
            "failure must not apply just the first style"
        );
    }
}
