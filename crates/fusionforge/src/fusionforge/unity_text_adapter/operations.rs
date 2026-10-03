use super::*;

pub(super) fn finite(name: &str, value: f64) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!("{name} must be finite"))
    }
}

pub(super) fn positive(name: &str, value: f64) -> Result<(), String> {
    finite(name, value)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be greater than zero"))
    }
}

pub(super) fn non_negative(name: &str, value: f64) -> Result<(), String> {
    finite(name, value)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must not be negative"))
    }
}

pub(super) fn explicit_lf_breaks(text: &str) -> Vec<u32> {
    text.chars()
        .enumerate()
        .filter_map(|(index, character)| (character == '\n').then_some((index + 1) as u32))
        .collect()
}

pub(super) fn content_rect(style: &UnityTextStyle) -> RectPx {
    RectPx {
        x: style.rect.x + style.padding.left,
        y: style.rect.y + style.padding.top,
        width: style.rect.width - style.padding.left - style.padding.right,
        height: style.rect.height - style.padding.top - style.padding.bottom,
    }
}

pub(super) fn placement(
    style: &UnityTextStyle,
    content: RectPx,
    font: &FontMetrics,
    measurement: &TextMeasurement,
) -> TextPlacement {
    let factors = style.alignment.factors();
    let origin = PointPx {
        x: content.x
            + (content.width - measurement.layout_width_px) * factors.x
            + style.content_offset.x,
        y: content.y
            + (content.height - measurement.layout_height_px) * factors.y
            + style.content_offset.y,
    };
    TextPlacement {
        layout_origin_px: origin,
        first_baseline_y_px: origin.y + font.baseline_from_line_top_px,
        ink_bounds_px: measurement.ink_bounds_px.translated(origin),
    }
}

pub(super) fn bounds_anchor(bounds: BoundsPx, alignment: TextAlignment) -> PointPx {
    let factors = alignment.factors();
    PointPx {
        x: bounds.left + bounds.width() * factors.x,
        y: bounds.top + bounds.height() * factors.y,
    }
}

pub(super) fn select_wrap_width(
    style: &UnityTextStyle,
    content_width: f64,
    sample: &CalibrationSample,
) -> Option<f64> {
    if !style.word_wrap {
        return None;
    }
    if sample.legacy.unicode_scalar_breaks == sample.replacement.unicode_scalar_breaks {
        return Some(content_width);
    }
    sample
        .replacement_wrap_candidates
        .iter()
        .filter(|candidate| candidate.unicode_scalar_breaks == sample.legacy.unicode_scalar_breaks)
        .min_by(|left, right| {
            let left_distance = (left.width_px - content_width).abs();
            let right_distance = (right.width_px - content_width).abs();
            left_distance
                .partial_cmp(&right_distance)
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    left.width_px
                        .partial_cmp(&right.width_px)
                        .unwrap_or(Ordering::Equal)
                })
        })
        .map(|candidate| candidate.width_px)
}

pub(super) fn round_px(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

pub(super) fn point_rounded(value: PointPx) -> PointPx {
    PointPx {
        x: round_px(value.x),
        y: round_px(value.y),
    }
}

pub(super) fn rect_rounded(value: RectPx) -> RectPx {
    RectPx {
        x: round_px(value.x),
        y: round_px(value.y),
        width: round_px(value.width),
        height: round_px(value.height),
    }
}

pub(super) fn bounds_rounded(value: BoundsPx) -> BoundsPx {
    BoundsPx {
        left: round_px(value.left),
        top: round_px(value.top),
        right: round_px(value.right),
        bottom: round_px(value.bottom),
    }
}

pub(super) fn placement_rounded(value: TextPlacement) -> TextPlacement {
    TextPlacement {
        layout_origin_px: point_rounded(value.layout_origin_px),
        first_baseline_y_px: round_px(value.first_baseline_y_px),
        ink_bounds_px: bounds_rounded(value.ink_bounds_px),
    }
}

pub(super) fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

pub(super) fn font_summary(font: &FontMetrics) -> FontSummary {
    FontSummary {
        semantic_id: font.semantic_id.clone(),
        font_size_px: round_px(font.font_size_px),
        line_height_px: round_px(font.line_height_px),
        baseline_from_line_top_px: round_px(font.baseline_from_line_top_px),
        render_scale: point_rounded(font.render_scale),
        ascent_px: round_px(font.ascent_px),
        descent_px: round_px(font.descent_px),
    }
}

pub(super) fn acceptance_summary(criteria: AcceptanceCriteria) -> AcceptanceCriteriaSummary {
    AcceptanceCriteriaSummary {
        max_translation_residual_px: round_px(criteria.max_translation_residual_px),
        max_baseline_residual_px: round_px(criteria.max_baseline_residual_px),
        max_advance_delta_px: round_px(criteria.max_advance_delta_px),
        max_ink_size_delta_px: round_px(criteria.max_ink_size_delta_px),
        max_wrap_width_residual_px: round_px(criteria.max_wrap_width_residual_px),
    }
}

pub(super) fn build_report(
    input: AdapterInput,
    input_sha256: String,
    allow_unsatisfied: bool,
) -> Result<AdapterReport, String> {
    let content = validate_input(&input)?;
    let mut raw = Vec::with_capacity(input.samples.len());
    let mut placements = Vec::with_capacity(input.samples.len());

    for sample in &input.samples {
        let legacy = placement(&input.style, content, &input.legacy_font, &sample.legacy);
        let replacement = placement(
            &input.style,
            content,
            &input.replacement_font,
            &sample.replacement,
        );
        let legacy_anchor = bounds_anchor(legacy.ink_bounds_px, input.style.alignment);
        let replacement_anchor = bounds_anchor(replacement.ink_bounds_px, input.style.alignment);
        let analysis = RawSampleAnalysis {
            required_translation: PointPx {
                x: legacy_anchor.x - replacement_anchor.x,
                y: legacy_anchor.y - replacement_anchor.y,
            },
            baseline_delta: legacy.first_baseline_y_px - replacement.first_baseline_y_px,
            advance_delta: sample.replacement.advance_width_px - sample.legacy.advance_width_px,
            advance_ratio: sample.legacy.advance_width_px / sample.replacement.advance_width_px,
            ink_width_delta: sample.replacement.ink_bounds_px.width()
                - sample.legacy.ink_bounds_px.width(),
            ink_height_delta: sample.replacement.ink_bounds_px.height()
                - sample.legacy.ink_bounds_px.height(),
            selected_wrap_width: select_wrap_width(&input.style, content.width, sample),
        };
        raw.push(analysis);
        placements.push((legacy, replacement));
    }

    let adapter_translation = PointPx {
        x: median(
            raw.iter()
                .map(|analysis| analysis.required_translation.x)
                .collect(),
        ),
        y: median(
            raw.iter()
                .map(|analysis| analysis.required_translation.y)
                .collect(),
        ),
    };
    let wrap_values = raw
        .iter()
        .filter_map(|analysis| analysis.selected_wrap_width)
        .collect::<Vec<_>>();
    let adapter_wrap_width = if input.style.word_wrap && !wrap_values.is_empty() {
        Some(median(wrap_values))
    } else {
        None
    };

    let mut failures = Vec::new();
    let mut sample_reports = Vec::with_capacity(input.samples.len());
    for ((sample, analysis), (legacy, replacement)) in input
        .samples
        .iter()
        .zip(raw.iter())
        .zip(placements.into_iter())
    {
        let translation_residual = PointPx {
            x: analysis.required_translation.x - adapter_translation.x,
            y: analysis.required_translation.y - adapter_translation.y,
        };
        let baseline_residual = analysis.baseline_delta - adapter_translation.y;
        let wrap_residual = match (analysis.selected_wrap_width, adapter_wrap_width) {
            (Some(sample_width), Some(adapter_width)) => Some(sample_width - adapter_width),
            _ => None,
        };

        if translation_residual.x.abs() > input.acceptance.max_translation_residual_px
            || translation_residual.y.abs() > input.acceptance.max_translation_residual_px
        {
            failures.push(format!(
                "sample '{}': translation residual ({:.6}, {:.6}) px exceeds {:.6} px",
                sample.id,
                translation_residual.x,
                translation_residual.y,
                input.acceptance.max_translation_residual_px
            ));
        }
        if baseline_residual.abs() > input.acceptance.max_baseline_residual_px {
            failures.push(format!(
                "sample '{}': baseline residual {:.6} px exceeds {:.6} px",
                sample.id, baseline_residual, input.acceptance.max_baseline_residual_px
            ));
        }
        if analysis.advance_delta.abs() > input.acceptance.max_advance_delta_px {
            failures.push(format!(
                "sample '{}': advance delta {:.6} px exceeds {:.6} px",
                sample.id, analysis.advance_delta, input.acceptance.max_advance_delta_px
            ));
        }
        if analysis.ink_width_delta.abs() > input.acceptance.max_ink_size_delta_px
            || analysis.ink_height_delta.abs() > input.acceptance.max_ink_size_delta_px
        {
            failures.push(format!(
                "sample '{}': ink size delta ({:.6}, {:.6}) px exceeds {:.6} px",
                sample.id,
                analysis.ink_width_delta,
                analysis.ink_height_delta,
                input.acceptance.max_ink_size_delta_px
            ));
        }
        if input.style.word_wrap && analysis.selected_wrap_width.is_none() {
            failures.push(format!(
                "sample '{}': no measured replacement width reproduces legacy Unicode-scalar breaks {:?}",
                sample.id, sample.legacy.unicode_scalar_breaks
            ));
        }
        if wrap_residual
            .is_some_and(|residual| residual.abs() > input.acceptance.max_wrap_width_residual_px)
        {
            failures.push(format!(
                "sample '{}': wrap-width residual {:.6} px exceeds {:.6} px",
                sample.id,
                wrap_residual.unwrap_or_default(),
                input.acceptance.max_wrap_width_residual_px
            ));
        }

        sample_reports.push(SampleReport {
            id: sample.id.clone(),
            locale: sample.locale,
            text_sha256: format!("{:x}", Sha256::digest(sample.text.as_bytes())),
            unicode_scalar_count: sample.text.chars().count() as u32,
            legacy_placement: placement_rounded(legacy),
            replacement_naive_placement: placement_rounded(replacement),
            required_translation_px: point_rounded(analysis.required_translation),
            baseline_delta_px: round_px(analysis.baseline_delta),
            advance_delta_px: round_px(analysis.advance_delta),
            legacy_over_replacement_advance_ratio: round_px(analysis.advance_ratio),
            ink_width_delta_px: round_px(analysis.ink_width_delta),
            ink_height_delta_px: round_px(analysis.ink_height_delta),
            legacy_unicode_scalar_breaks: sample.legacy.unicode_scalar_breaks.clone(),
            replacement_unicode_scalar_breaks: sample.replacement.unicode_scalar_breaks.clone(),
            selected_wrap_width_px: analysis.selected_wrap_width.map(round_px),
            residual: SampleResidual {
                translation_px: point_rounded(translation_residual),
                baseline_after_adapter_px: round_px(baseline_residual),
                wrap_width_px: wrap_residual.map(round_px),
            },
        });
    }

    let median_baseline = median(raw.iter().map(|value| value.baseline_delta).collect());
    let median_advance = median(raw.iter().map(|value| value.advance_delta).collect());
    let median_advance_ratio = median(raw.iter().map(|value| value.advance_ratio).collect());
    let median_ink_width = median(raw.iter().map(|value| value.ink_width_delta).collect());
    let median_ink_height = median(raw.iter().map(|value| value.ink_height_delta).collect());
    let status = if failures.is_empty() {
        "passed"
    } else {
        "failed"
    };

    Ok(AdapterReport {
        schema: UNITY_TEXT_ADAPTER_OUTPUT_SCHEMA,
        input_sha256,
        reference_canvas: SizePx {
            width: round_px(input.reference_canvas.width),
            height: round_px(input.reference_canvas.height),
        },
        style: StyleSummary {
            semantic_id: input.style.semantic_id.clone(),
            source_rect_px: rect_rounded(input.style.rect),
            content_rect_px: rect_rounded(content),
            padding_px: InsetsPx {
                left: round_px(input.style.padding.left),
                right: round_px(input.style.padding.right),
                top: round_px(input.style.padding.top),
                bottom: round_px(input.style.padding.bottom),
            },
            content_offset_px: point_rounded(input.style.content_offset),
            alignment: input.style.alignment,
            word_wrap: input.style.word_wrap,
            clipping: input.style.clipping,
        },
        measurement_space: input.measurement_space,
        evidence_inputs: input.evidence.clone(),
        legacy_font: font_summary(&input.legacy_font),
        replacement_font: font_summary(&input.replacement_font),
        native_contract: NativeTextContract {
            schema: NATIVE_TEXT_REPLACEMENT_CONTRACT_SCHEMA,
            style_semantic_id: input.style.semantic_id.clone(),
            reference_canvas: SizePx {
                width: round_px(input.reference_canvas.width),
                height: round_px(input.reference_canvas.height),
            },
            coordinate_convention: CoordinateConvention::TopLeftOriginXRightYDown,
            measurement_space: input.measurement_space,
            scale_contract: NativeScaleContract {
                glyph_render_scale_application:
                    GlyphRenderScaleApplication::ApplyExactlyOnceToGlyphRasterBeforeMeasurement,
                line_height: PostScaleMetricPolicy::AlreadyInMeasurementSpaceDoNotScale,
                offsets: PostScaleMetricPolicy::AlreadyInMeasurementSpaceDoNotScale,
                wrap_width: PostScaleMetricPolicy::AlreadyInMeasurementSpaceDoNotScale,
                measurements: PostScaleMetricPolicy::AlreadyInMeasurementSpaceDoNotScale,
            },
            rect_px: rect_rounded(input.style.rect),
            content_rect_px: rect_rounded(content),
            horizontal_alignment: input.style.alignment.horizontal(),
            vertical_alignment: input.style.alignment.vertical(),
            word_wrap: input.style.word_wrap,
            clipping: input.style.clipping,
            replacement_font_semantic_id: input.replacement_font.semantic_id.clone(),
            replacement_font_size_px: round_px(input.replacement_font.font_size_px),
            replacement_line_height_px: round_px(input.replacement_font.line_height_px),
            replacement_glyph_render_scale: point_rounded(input.replacement_font.render_scale),
            content_offset_px: point_rounded(input.style.content_offset),
            replacement_metric_compensation_px: point_rounded(adapter_translation),
            final_text_offset_from_aligned_content_px: point_rounded(PointPx {
                x: input.style.content_offset.x + adapter_translation.x,
                y: input.style.content_offset.y + adapter_translation.y,
            }),
            wrap_layout_width_px: adapter_wrap_width.map(round_px),
            preserve_source_clip_rect: matches!(input.style.clipping, TextClipping::Clip),
        },
        compensation: AdapterCompensation {
            baseline_delta_px: round_px(median_baseline),
            advance_delta_px: round_px(median_advance),
            legacy_over_replacement_advance_ratio: round_px(median_advance_ratio),
            ink_width_delta_px: round_px(median_ink_width),
            ink_height_delta_px: round_px(median_ink_height),
            wrap_width_delta_px: adapter_wrap_width.map(|width| round_px(width - content.width)),
        },
        samples: sample_reports,
        acceptance: AcceptanceReport {
            status,
            criteria: acceptance_summary(input.acceptance),
            failures,
        },
        triage: TriageStatus { allow_unsatisfied },
    })
}
