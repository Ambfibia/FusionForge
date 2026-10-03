use std::{
    cmp::Ordering,
    collections::BTreeSet,
    env, fs,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(test)]
mod tests;

mod containers;
mod constants;
mod types;
mod localization;
mod materials;
mod state;
mod operations;
mod validation;
mod assets;
mod output;
mod native_quit_menu;
pub(super) use native_quit_menu::calibrate_quit_menu;

pub use containers::{
    UNITY_TEXT_ADAPTER_INPUT_SCHEMA, UNITY_TEXT_ADAPTER_OUTPUT_SCHEMA, adapt_unity_text_cli
};
use containers::{LegacyFontObjectEvidence, UnityTextStyle};
pub use constants::NATIVE_TEXT_REPLACEMENT_CONTRACT_SCHEMA;
use constants::MAX_CALIBRATION_TEXT_SCALARS;
use types::{
    AdapterInput, LegacySourceAlias, CanonicalSourceRole,
    SizePx, RectPx, InsetsPx, PointPx, BoundsPx, TextAlignment, TextClipping, FontMetrics,
    CalibrationSample, TextMeasurement, AcceptanceCriteria, AdapterReport,
    StyleSummary, FontSummary, CoordinateConvention, PostScaleMetricPolicy,
    NativeScaleContract, NativeTextContract, AdapterCompensation, SampleReport, TextPlacement,
    SampleResidual, AcceptanceReport, AcceptanceCriteriaSummary, RawSampleAnalysis
};
use localization::SampleLocale;
use materials::GlyphRenderScaleApplication;
use state::TriageStatus;
use operations::{
    finite, positive, non_negative, explicit_lf_breaks, content_rect, build_report
};
use validation::validate_input;
use assets::same_path;
use output::write_report;
#[cfg(test)]
use types::{MeasurementSpace, WrapCandidate};
