use super::*;

pub(super) fn validate_semantic_id(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 160 {
        return Err(format!("{name} must contain 1..=160 characters"));
    }
    if value.starts_with('/')
        || value.starts_with('\\')
        || value.contains('\\')
        || value.contains(':')
        || value.split('/').any(|segment| segment == "..")
    {
        return Err(format!(
            "{name} must be a portable semantic identifier, not a machine path"
        ));
    }
    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "._/-".contains(character))
    {
        return Err(format!(
            "{name} may contain only ASCII letters, digits, '.', '_', '/', and '-'"
        ));
    }
    Ok(())
}

pub(super) fn validate_sha256(name: &str, value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{name} must be exactly 64 lowercase hexadecimal characters"
        ));
    }
    Ok(())
}

pub(super) fn validate_portable_relative_path(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.starts_with('/')
        || value.contains('\\')
        || value.contains(':')
        || value.chars().any(char::is_control)
        || value
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(format!(
            "{name} must be a portable relative path without traversal"
        ));
    }
    Ok(())
}

pub(super) fn validate_serialized_asset(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || matches!(value, "." | "..")
        || value.contains(['/', '\\', ':'])
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{name} must be one exact serialized-asset name, not a path"
        ));
    }
    Ok(())
}

pub(super) fn validate_producer_version(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 64
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | '+')
        })
    {
        return Err(format!(
            "{name} must use 1..=64 portable version characters"
        ));
    }
    Ok(())
}

pub(super) fn validate_evidence(input: &AdapterInput) -> Result<(), String> {
    let legacy = &input.evidence.legacy_font;
    match (legacy.source.alias, legacy.source.canonical_role) {
        (LegacySourceAlias::Retrobution, CanonicalSourceRole::Primary)
        | (LegacySourceAlias::Academy, CanonicalSourceRole::Alternate) => {}
        (LegacySourceAlias::Retrobution, _) => {
            return Err(
                "evidence.legacyFont source retrobution must use canonicalRole primary".into(),
            );
        }
        (LegacySourceAlias::Academy, _) => {
            return Err(
                "evidence.legacyFont source academy must use canonicalRole alternate".into(),
            );
        }
    }
    validate_portable_relative_path(
        "evidence.legacyFont.source.relativeContainer",
        &legacy.source.relative_container,
    )?;
    validate_sha256(
        "evidence.legacyFont.source.rawContainerSha256",
        &legacy.source.raw_container_sha256,
    )?;
    validate_semantic_id(
        "evidence.legacyFont.object.semanticId",
        &legacy.object.semantic_id,
    )?;
    if legacy.object.semantic_id != input.legacy_font.semantic_id {
        return Err(
            "evidence.legacyFont.object.semanticId must match legacyFont.semanticId".to_string(),
        );
    }
    validate_serialized_asset(
        "evidence.legacyFont.object.serializedAsset",
        &legacy.object.serialized_asset,
    )?;
    if legacy.object.unity_type != "Font" {
        return Err("evidence.legacyFont.object.unityType must be exact value 'Font'".to_string());
    }
    if legacy.object.path_id <= 0 {
        return Err("evidence.legacyFont.object.pathId must be greater than zero".to_string());
    }
    validate_sha256(
        "evidence.legacyFont.object.rawObjectSha256",
        &legacy.object.raw_object_sha256,
    )?;

    let replacement = &input.evidence.replacement_font;
    validate_semantic_id(
        "evidence.replacementFont.semanticId",
        &replacement.semantic_id,
    )?;
    if replacement.semantic_id != input.replacement_font.semantic_id {
        return Err(
            "evidence.replacementFont.semanticId must match replacementFont.semanticId".to_string(),
        );
    }
    validate_sha256(
        "evidence.replacementFont.bytesSha256",
        &replacement.bytes_sha256,
    )?;

    validate_semantic_id(
        "evidence.measurement.producer",
        &input.evidence.measurement.producer,
    )?;
    validate_producer_version(
        "evidence.measurement.version",
        &input.evidence.measurement.version,
    )?;
    validate_sha256(
        "evidence.measurement.captureSha256",
        &input.evidence.measurement.capture_sha256,
    )
}

pub(super) fn validate_rect(name: &str, rect: RectPx) -> Result<(), String> {
    finite(&format!("{name}.x"), rect.x)?;
    finite(&format!("{name}.y"), rect.y)?;
    positive(&format!("{name}.width"), rect.width)?;
    positive(&format!("{name}.height"), rect.height)
}

pub(super) fn validate_bounds(name: &str, bounds: BoundsPx) -> Result<(), String> {
    finite(&format!("{name}.left"), bounds.left)?;
    finite(&format!("{name}.top"), bounds.top)?;
    finite(&format!("{name}.right"), bounds.right)?;
    finite(&format!("{name}.bottom"), bounds.bottom)?;
    positive(&format!("{name}.width"), bounds.width())?;
    positive(&format!("{name}.height"), bounds.height())
}

pub(super) fn validate_sample_text(name: &str, text: &str) -> Result<u32, String> {
    let scalar_count = text.chars().count();
    if scalar_count == 0 {
        return Err(format!("{name} must not be empty"));
    }
    if scalar_count > MAX_CALIBRATION_TEXT_SCALARS {
        return Err(format!(
            "{name} has {scalar_count} Unicode scalars; maximum is {MAX_CALIBRATION_TEXT_SCALARS}"
        ));
    }
    if text.chars().all(char::is_whitespace) {
        return Err(format!("{name} must contain a visible character"));
    }
    for (index, character) in text.chars().enumerate() {
        if character.is_control() && character != '\t' && character != '\n' {
            return Err(format!(
                "{name} contains unsupported control U+{:04X} at Unicode scalar index {index}",
                character as u32
            ));
        }
    }
    u32::try_from(scalar_count).map_err(|_| format!("{name} is too long"))
}

pub(super) fn validate_breaks(name: &str, breaks: &[u32], scalar_count: u32) -> Result<(), String> {
    let mut previous = 0;
    for (index, value) in breaks.iter().copied().enumerate() {
        if value == 0 || value >= scalar_count {
            return Err(format!(
                "{name}[{index}]={value} must be between 1 and the derived text scalar count - 1"
            ));
        }
        if index > 0 && value <= previous {
            return Err(format!("{name} must be strictly increasing"));
        }
        previous = value;
    }
    Ok(())
}

pub(super) fn validate_font(name: &str, font: &FontMetrics) -> Result<(), String> {
    validate_semantic_id(&format!("{name}.semanticId"), &font.semantic_id)?;
    positive(&format!("{name}.fontSizePx"), font.font_size_px)?;
    positive(&format!("{name}.lineHeightPx"), font.line_height_px)?;
    positive(&format!("{name}.renderScale.x"), font.render_scale.x)?;
    positive(&format!("{name}.renderScale.y"), font.render_scale.y)?;
    non_negative(
        &format!("{name}.baselineFromLineTopPx"),
        font.baseline_from_line_top_px,
    )?;
    positive(&format!("{name}.ascentPx"), font.ascent_px)?;
    non_negative(&format!("{name}.descentPx"), font.descent_px)?;
    if font.baseline_from_line_top_px > font.line_height_px * 2.0 {
        return Err(format!(
            "{name}.baselineFromLineTopPx is implausibly larger than two line heights"
        ));
    }
    Ok(())
}

pub(super) fn validate_measurement(
    name: &str,
    measurement: &TextMeasurement,
    scalar_count: u32,
) -> Result<(), String> {
    positive(
        &format!("{name}.layoutWidthPx"),
        measurement.layout_width_px,
    )?;
    positive(
        &format!("{name}.layoutHeightPx"),
        measurement.layout_height_px,
    )?;
    positive(
        &format!("{name}.advanceWidthPx"),
        measurement.advance_width_px,
    )?;
    validate_bounds(&format!("{name}.inkBoundsPx"), measurement.ink_bounds_px)?;
    if measurement.line_count == 0 {
        return Err(format!("{name}.lineCount must be greater than zero"));
    }
    validate_breaks(
        &format!("{name}.unicodeScalarBreaks"),
        &measurement.unicode_scalar_breaks,
        scalar_count,
    )?;
    if measurement.unicode_scalar_breaks.len() + 1 != measurement.line_count as usize {
        return Err(format!(
            "{name}.lineCount must equal unicodeScalarBreaks.length + 1"
        ));
    }
    Ok(())
}

pub(super) fn validate_input(input: &AdapterInput) -> Result<RectPx, String> {
    if input.schema != UNITY_TEXT_ADAPTER_INPUT_SCHEMA {
        return Err(format!(
            "unsupported schema '{}'; expected {UNITY_TEXT_ADAPTER_INPUT_SCHEMA}",
            input.schema
        ));
    }
    positive("referenceCanvas.width", input.reference_canvas.width)?;
    positive("referenceCanvas.height", input.reference_canvas.height)?;
    validate_semantic_id("style.semanticId", &input.style.semantic_id)?;
    validate_rect("style.rect", input.style.rect)?;
    for (name, value) in [
        ("style.padding.left", input.style.padding.left),
        ("style.padding.right", input.style.padding.right),
        ("style.padding.top", input.style.padding.top),
        ("style.padding.bottom", input.style.padding.bottom),
        ("style.contentOffset.x", input.style.content_offset.x),
        ("style.contentOffset.y", input.style.content_offset.y),
    ] {
        finite(name, value)?;
    }
    let content = content_rect(&input.style);
    validate_rect("style content rect after padding", content)?;
    validate_font("legacyFont", &input.legacy_font)?;
    validate_font("replacementFont", &input.replacement_font)?;
    validate_evidence(input)?;
    if input.samples.is_empty() {
        return Err("samples must contain at least one measured string".to_string());
    }

    let mut ids = BTreeSet::new();
    let mut has_en = false;
    let mut has_ru = false;
    for (sample_index, sample) in input.samples.iter().enumerate() {
        let prefix = format!("samples[{sample_index}]");
        validate_semantic_id(&format!("{prefix}.id"), &sample.id)?;
        if !ids.insert(sample.id.as_str()) {
            return Err(format!("duplicate calibration sample id '{}'", sample.id));
        }
        match sample.locale {
            SampleLocale::En => has_en = true,
            SampleLocale::Ru => has_ru = true,
        }
        let unicode_scalar_count = validate_sample_text(&format!("{prefix}.text"), &sample.text)?;
        validate_measurement(
            &format!("{prefix}.legacy"),
            &sample.legacy,
            unicode_scalar_count,
        )?;
        validate_measurement(
            &format!("{prefix}.replacement"),
            &sample.replacement,
            unicode_scalar_count,
        )?;
        let mut candidate_widths = Vec::new();
        for (candidate_index, candidate) in sample.replacement_wrap_candidates.iter().enumerate() {
            positive(
                &format!("{prefix}.replacementWrapCandidates[{candidate_index}].widthPx"),
                candidate.width_px,
            )?;
            validate_breaks(
                &format!(
                    "{prefix}.replacementWrapCandidates[{candidate_index}].unicodeScalarBreaks"
                ),
                &candidate.unicode_scalar_breaks,
                unicode_scalar_count,
            )?;
            if candidate_widths
                .iter()
                .any(|width: &f64| (*width - candidate.width_px).abs() <= 0.000_001)
            {
                return Err(format!(
                    "{prefix}.replacementWrapCandidates contains duplicate width {}",
                    candidate.width_px
                ));
            }
            candidate_widths.push(candidate.width_px);
        }

        let explicit_breaks = explicit_lf_breaks(&sample.text);
        for explicit_break in &explicit_breaks {
            if !sample.legacy.unicode_scalar_breaks.contains(explicit_break)
                || !sample
                    .replacement
                    .unicode_scalar_breaks
                    .contains(explicit_break)
            {
                return Err(format!(
                    "{prefix} measurements must retain explicit LF break after Unicode scalar {}",
                    explicit_break - 1
                ));
            }
        }
        if !input.style.word_wrap {
            if !sample.replacement_wrap_candidates.is_empty() {
                return Err(format!(
                    "{prefix}.replacementWrapCandidates must be empty when style.wordWrap is false"
                ));
            }
            if sample.legacy.unicode_scalar_breaks != explicit_breaks
                || sample.replacement.unicode_scalar_breaks != explicit_breaks
            {
                return Err(format!(
                    "{prefix} may contain only explicit LF breaks when style.wordWrap is false"
                ));
            }
        }
    }

    if !has_en || !has_ru {
        return Err("samples must include at least one locale=en and one locale=ru".to_string());
    }

    for (name, value) in [
        (
            "acceptance.maxTranslationResidualPx",
            input.acceptance.max_translation_residual_px,
        ),
        (
            "acceptance.maxBaselineResidualPx",
            input.acceptance.max_baseline_residual_px,
        ),
        (
            "acceptance.maxAdvanceDeltaPx",
            input.acceptance.max_advance_delta_px,
        ),
        (
            "acceptance.maxInkSizeDeltaPx",
            input.acceptance.max_ink_size_delta_px,
        ),
        (
            "acceptance.maxWrapWidthResidualPx",
            input.acceptance.max_wrap_width_residual_px,
        ),
    ] {
        non_negative(name, value)?;
    }
    Ok(content)
}
