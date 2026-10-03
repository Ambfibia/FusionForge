//! Deterministic, resumable standalone GPU acceptance for published equipment.
//!
//! The existing Bevy preview proves one exact GLB's standalone
//! load/material/skin/animation/render path. It intentionally cannot prove
//! player socket placement, skinned body assembly, hide policies, or vehicle
//! mounting. This orchestrator keeps those scopes separate and never mutates
//! the candidate or production asset trees.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    process::Command,
    time::Instant,
};

use ffone_skinned_model::{
    LogicalModelGpuFacts, gpu_evidence_relative_paths, gpu_model_facts_from_glb,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE, EQUIPMENT_LOGICAL_MODEL_BATCH_SCHEMA,
    EquipmentLogicalModelBatchReport, PipelineError, Result, audit_logical_model_gpu_evidence,
    audit_logical_model_tree, error::io_at,
};

#[cfg(test)]
mod tests;

mod constants;
mod state;
mod types;
mod models;
mod operations_run_equipment_gpu_batch;
mod operations_classify_preview_failure;
mod validation;
mod assets;
mod output;

pub use constants::{EQUIPMENT_GPU_BATCH_SCHEMA, EQUIPMENT_GPU_SCOPE};
use constants::REQUIRED_SLOTS;
pub use state::EquipmentGpuBatchMode;
use state::{Selection, status};
pub use types::{
    EquipmentGpuBatchOptions, EquipmentGpuShard, EquipmentGpuBatchReport,
    GpuBatchFileEvidence, EquipmentGpuBatchTiming, EquipmentGpuBatchCounts,
    EquipmentGpuCoverage, EquipmentGpuSlotCoverage, EquipmentGpuFactsSummary,
    EquipmentGpuBlocker
};
use types::{Candidate, ClassifiedPreviewFailure};
pub use models::EquipmentGpuModelRun;
use models::model_blocker;
pub use operations_run_equipment_gpu_batch::run_equipment_gpu_batch;
use operations_run_equipment_gpu_batch::canonical_directory;
use operations_classify_preview_failure::{
    classify_preview_failure, elapsed_milliseconds, u64_count, sha256_hex
};
use validation::{
    validate_options, reject_overlapping_paths, gpu_batch_error, gpu_batch_error_value
};
use assets::{absolute_new_report_path, slash_path, is_exact_equipment_output_path};
use output::write_report_new;
#[cfg(test)]
use operations_run_equipment_gpu_batch::{select_candidates, variant_labels, scope_blockers};
