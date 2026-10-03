use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct AdapterInput {
    pub(super) schema: String,
    pub(super) reference_canvas: SizePx,
    pub(super) measurement_space: MeasurementSpace,
    pub(super) evidence: EvidenceInputs,
    pub(super) style: UnityTextStyle,
    pub(super) legacy_font: FontMetrics,
    pub(super) replacement_font: FontMetrics,
    pub(super) samples: Vec<CalibrationSample>,
    pub(super) acceptance: AcceptanceCriteria,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum MeasurementSpace {
    ReferenceCanvasPixelsAfterGlyphRenderScale,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum LegacySourceAlias {
    Retrobution,
    Academy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum CanonicalSourceRole {
    Primary,
    Alternate,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct EvidenceInputs {
    pub(super) legacy_font: LegacyFontEvidence,
    pub(super) replacement_font: ReplacementFontEvidence,
    pub(super) measurement: MeasurementEvidence,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct LegacyFontEvidence {
    pub(super) source: LegacySourceEvidence,
    pub(super) object: LegacyFontObjectEvidence,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct LegacySourceEvidence {
    pub(super) alias: LegacySourceAlias,
    pub(super) canonical_role: CanonicalSourceRole,
    pub(super) relative_container: String,
    pub(super) raw_container_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReplacementFontEvidence {
    pub(super) semantic_id: String,
    pub(super) bytes_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct MeasurementEvidence {
    pub(super) producer: String,
    pub(super) version: String,
    pub(super) capture_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct SizePx {
    pub(super) width: f64,
    pub(super) height: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct RectPx {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct InsetsPx {
    pub(super) left: f64,
    pub(super) right: f64,
    pub(super) top: f64,
    pub(super) bottom: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct PointPx {
    pub(super) x: f64,
    pub(super) y: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct BoundsPx {
    pub(super) left: f64,
    pub(super) top: f64,
    pub(super) right: f64,
    pub(super) bottom: f64,
}

impl BoundsPx {
    pub(super) fn width(self) -> f64 {
        self.right - self.left
    }

    pub(super) fn height(self) -> f64 {
        self.bottom - self.top
    }

    pub(super) fn translated(self, offset: PointPx) -> Self {
        Self {
            left: self.left + offset.x,
            top: self.top + offset.y,
            right: self.right + offset.x,
            bottom: self.bottom + offset.y,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum TextAlignment {
    UpperLeft,
    UpperCenter,
    UpperRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    LowerLeft,
    LowerCenter,
    LowerRight,
}

impl TextAlignment {
    pub(super) fn factors(self) -> PointPx {
        match self {
            Self::UpperLeft => PointPx { x: 0.0, y: 0.0 },
            Self::UpperCenter => PointPx { x: 0.5, y: 0.0 },
            Self::UpperRight => PointPx { x: 1.0, y: 0.0 },
            Self::MiddleLeft => PointPx { x: 0.0, y: 0.5 },
            Self::MiddleCenter => PointPx { x: 0.5, y: 0.5 },
            Self::MiddleRight => PointPx { x: 1.0, y: 0.5 },
            Self::LowerLeft => PointPx { x: 0.0, y: 1.0 },
            Self::LowerCenter => PointPx { x: 0.5, y: 1.0 },
            Self::LowerRight => PointPx { x: 1.0, y: 1.0 },
        }
    }

    pub(super) fn horizontal(self) -> &'static str {
        match self {
            Self::UpperLeft | Self::MiddleLeft | Self::LowerLeft => "left",
            Self::UpperCenter | Self::MiddleCenter | Self::LowerCenter => "center",
            Self::UpperRight | Self::MiddleRight | Self::LowerRight => "right",
        }
    }

    pub(super) fn vertical(self) -> &'static str {
        match self {
            Self::UpperLeft | Self::UpperCenter | Self::UpperRight => "top",
            Self::MiddleLeft | Self::MiddleCenter | Self::MiddleRight => "center",
            Self::LowerLeft | Self::LowerCenter | Self::LowerRight => "bottom",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum TextClipping {
    Clip,
    Overflow,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct FontMetrics {
    pub(super) semantic_id: String,
    pub(super) font_size_px: f64,
    pub(super) line_height_px: f64,
    /// Explicit post-font raster scale. Every supplied measurement must already
    /// include this transform (for example Jeffe's validated 1.0 x 0.70 adapter).
    pub(super) render_scale: PointPx,
    pub(super) baseline_from_line_top_px: f64,
    pub(super) ascent_px: f64,
    pub(super) descent_px: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CalibrationSample {
    pub(super) id: String,
    pub(super) text: String,
    pub(super) legacy: TextMeasurement,
    pub(super) locale: SampleLocale,
    pub(super) replacement: TextMeasurement,
    #[serde(default)]
    pub(super) replacement_wrap_candidates: Vec<WrapCandidate>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct TextMeasurement {
    pub(super) layout_width_px: f64,
    pub(super) layout_height_px: f64,
    pub(super) advance_width_px: f64,
    pub(super) ink_bounds_px: BoundsPx,
    pub(super) line_count: u32,
    pub(super) unicode_scalar_breaks: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct WrapCandidate {
    pub(super) width_px: f64,
    pub(super) unicode_scalar_breaks: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct AcceptanceCriteria {
    pub(super) max_translation_residual_px: f64,
    pub(super) max_baseline_residual_px: f64,
    pub(super) max_advance_delta_px: f64,
    pub(super) max_ink_size_delta_px: f64,
    pub(super) max_wrap_width_residual_px: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AdapterReport {
    pub(super) schema: &'static str,
    pub(super) input_sha256: String,
    pub(super) reference_canvas: SizePx,
    pub(super) style: StyleSummary,
    pub(super) measurement_space: MeasurementSpace,
    pub(super) evidence_inputs: EvidenceInputs,
    pub(super) legacy_font: FontSummary,
    pub(super) replacement_font: FontSummary,
    pub(super) native_contract: NativeTextContract,
    pub(super) compensation: AdapterCompensation,
    pub(super) samples: Vec<SampleReport>,
    pub(super) acceptance: AcceptanceReport,
    pub(super) triage: TriageStatus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StyleSummary {
    pub(super) semantic_id: String,
    pub(super) source_rect_px: RectPx,
    pub(super) content_rect_px: RectPx,
    pub(super) padding_px: InsetsPx,
    pub(super) content_offset_px: PointPx,
    pub(super) alignment: TextAlignment,
    pub(super) word_wrap: bool,
    pub(super) clipping: TextClipping,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FontSummary {
    pub(super) semantic_id: String,
    pub(super) font_size_px: f64,
    pub(super) line_height_px: f64,
    pub(super) render_scale: PointPx,
    pub(super) baseline_from_line_top_px: f64,
    pub(super) ascent_px: f64,
    pub(super) descent_px: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum CoordinateConvention {
    TopLeftOriginXRightYDown,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum PostScaleMetricPolicy {
    AlreadyInMeasurementSpaceDoNotScale,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NativeScaleContract {
    pub(super) glyph_render_scale_application: GlyphRenderScaleApplication,
    pub(super) line_height: PostScaleMetricPolicy,
    pub(super) offsets: PostScaleMetricPolicy,
    pub(super) wrap_width: PostScaleMetricPolicy,
    pub(super) measurements: PostScaleMetricPolicy,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NativeTextContract {
    pub(super) schema: &'static str,
    pub(super) style_semantic_id: String,
    pub(super) reference_canvas: SizePx,
    pub(super) coordinate_convention: CoordinateConvention,
    pub(super) measurement_space: MeasurementSpace,
    pub(super) scale_contract: NativeScaleContract,
    pub(super) rect_px: RectPx,
    pub(super) content_rect_px: RectPx,
    pub(super) horizontal_alignment: &'static str,
    pub(super) vertical_alignment: &'static str,
    pub(super) word_wrap: bool,
    pub(super) clipping: TextClipping,
    pub(super) replacement_font_semantic_id: String,
    pub(super) replacement_font_size_px: f64,
    pub(super) replacement_line_height_px: f64,
    pub(super) replacement_glyph_render_scale: PointPx,
    pub(super) content_offset_px: PointPx,
    pub(super) replacement_metric_compensation_px: PointPx,
    pub(super) final_text_offset_from_aligned_content_px: PointPx,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) wrap_layout_width_px: Option<f64>,
    pub(super) preserve_source_clip_rect: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AdapterCompensation {
    pub(super) baseline_delta_px: f64,
    pub(super) advance_delta_px: f64,
    pub(super) legacy_over_replacement_advance_ratio: f64,
    pub(super) ink_width_delta_px: f64,
    pub(super) ink_height_delta_px: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) wrap_width_delta_px: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SampleReport {
    pub(super) id: String,
    pub(super) locale: SampleLocale,
    pub(super) text_sha256: String,
    pub(super) unicode_scalar_count: u32,
    pub(super) legacy_placement: TextPlacement,
    pub(super) replacement_naive_placement: TextPlacement,
    pub(super) required_translation_px: PointPx,
    pub(super) baseline_delta_px: f64,
    pub(super) advance_delta_px: f64,
    pub(super) legacy_over_replacement_advance_ratio: f64,
    pub(super) ink_width_delta_px: f64,
    pub(super) ink_height_delta_px: f64,
    pub(super) legacy_unicode_scalar_breaks: Vec<u32>,
    pub(super) replacement_unicode_scalar_breaks: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) selected_wrap_width_px: Option<f64>,
    pub(super) residual: SampleResidual,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TextPlacement {
    pub(super) layout_origin_px: PointPx,
    pub(super) first_baseline_y_px: f64,
    pub(super) ink_bounds_px: BoundsPx,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SampleResidual {
    pub(super) translation_px: PointPx,
    pub(super) baseline_after_adapter_px: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) wrap_width_px: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcceptanceReport {
    pub(super) status: &'static str,
    pub(super) criteria: AcceptanceCriteriaSummary,
    pub(super) failures: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcceptanceCriteriaSummary {
    pub(super) max_translation_residual_px: f64,
    pub(super) max_baseline_residual_px: f64,
    pub(super) max_advance_delta_px: f64,
    pub(super) max_ink_size_delta_px: f64,
    pub(super) max_wrap_width_residual_px: f64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RawSampleAnalysis {
    pub(super) required_translation: PointPx,
    pub(super) baseline_delta: f64,
    pub(super) advance_delta: f64,
    pub(super) advance_ratio: f64,
    pub(super) ink_width_delta: f64,
    pub(super) ink_height_delta: f64,
    pub(super) selected_wrap_width: Option<f64>,
}
