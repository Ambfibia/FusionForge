//! Atomic, collision-safe publication of a complete logical-model source tree.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use ffone_skinned_model::{
    minimal_windows_glb_filename, minimal_windows_png_filename, model_relative_path,
    windows_png_filename_preserving_legacy_extension,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{IgnoredAny, SeqAccess, Visitor},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::{
    PipelineError, Result, audit_logical_model_tree,
    error::io_at,
    legacy_shader_state::exact_shader_texture_defaults,
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
mod textures;
mod operations;
mod assets;
mod collision;
mod output;
mod validation;

pub use models::{
    LOGICAL_MODEL_BATCH_REPORT_SCHEMA, LOGICAL_MODEL_BATCH_REPORT_FILE,
    LogicalModelBatchPublishOptions, LogicalModelBatchPublishReport, LogicalModelBatchCounts,
    LogicalModelBatchMapping, LogicalModelBatchBlocker, publish_logical_model_batch
};
use constants::STAGING_SEQUENCE;
use types::{
    SourcePreflightDocument, ArrayLength,
    BatchPlan, BatchCoordinateEvidence, BatchStagingDirectory
};
use textures::{PreflightTexture, invalid_saved_texture_slot_evidence};
use operations::{
    preflight_batch, invalid_native_name,
    build_report, portable_string_key, sha256_hex
};
use assets::{
    PortableOutputRegistry, RegisteredPath, relative_path_string,
    slash_path
};
use collision::output_collision;
use output::write_batch_report;
use validation::{validate_windows_component, batch_error, batch_error_value};
