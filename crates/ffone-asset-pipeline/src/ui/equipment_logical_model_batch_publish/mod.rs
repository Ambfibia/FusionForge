//! Atomic semantic-root publication of table-owned player equipment.
//!
//! This boundary consumes only audited `ffone.logical-model-source.v1`
//! documents and their sibling equipment source manifest. It never reads
//! Unity bundles and never mutates production assets.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use ffone_skinned_model::{
    minimal_windows_glb_filename, minimal_windows_png_filename,
    windows_png_filename_preserving_legacy_extension,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::{
    PipelineError, Result, audit_logical_model_tree,
    error::io_at,
    logical_model_publish::{
        LOGICAL_MODEL_SOURCE_SCHEMA, LogicalModelPublishOptions, prepare_logical_model,
        write_prepared_logical_model,
    },
};

use crate::shared::has_generated_identity;

#[cfg(test)]
mod tests;

mod models;
mod constants;
mod types;
mod assets;
mod output;
mod textures;
mod operations;
mod validation;
mod collision;

pub use models::{
    EQUIPMENT_LOGICAL_MODEL_BATCH_SCHEMA, EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE,
    EquipmentLogicalModelBatchPublishOptions, EquipmentLogicalModelBatchReport,
    EquipmentLogicalModelBatchTiming, EquipmentLogicalModelBatchCounts,
    EquipmentLogicalModelMapping, EquipmentLogicalModelBlocker,
    publish_equipment_logical_model_batch
};
use models::PublishedModel;
use constants::{
    EQUIPMENT_SOURCE_BATCH_SCHEMA, EQUIPMENT_TAXONOMY, ALLOWED_CATEGORIES, STAGING_SEQUENCE
};
pub use types::FileEvidence;
use types::{
    EquipmentSourceCounts, EquipmentSourceBlocker, SourcePreflightDocument, EquipmentPlan, PreflightResult,
    OutputClaim, EquipmentBatchStagingDirectory
};
use assets::{
    EquipmentSourceManifest, route_stem_qualifier, sibling_manifest_path, portable_path_key,
    slash_path, slash_path_checked
};
use output::{EquipmentSourceExport, write_report};
use textures::{PreflightTexture, texture_format};
use operations::{
    preflight_batch, register_output_claims,
    native_preparation_blocker, native_contract_blocker, published_file_counts,
    coordinate_summary, rewrite_report, report_bytes,
    clean_relative_components, portable_key, sort_blockers, elapsed_milliseconds, u64_count,
    sha256_hex
};
use validation::{
    validate_source_manifest, reject_existing_output, validate_true_name,
    validate_windows_component, equipment_error, equipment_error_value
};
use collision::global_collision_blockers;
