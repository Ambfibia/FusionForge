//! Selective, fail-closed installation of GPU-accepted tutorial models.
//!
//! The installer owns only [`TUTORIAL_MODEL_ROOT`]. It consumes an immutable
//! native logical-model candidate plus one exact GPU evidence pair per selected
//! GLB. Unity data and the general `characters/` tree are intentionally outside
//! this boundary.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use ffone_skinned_model::{
    AutomatedGpuStatus, GPU_EVIDENCE_SCHEMA, GPU_RENDER_PROFILE, LogicalModelGpuEvidence,
    LogicalModelGpuFacts, SourceOutlineMode, VisualParityClaim, gpu_evidence_relative_paths,
    gpu_model_facts_from_glb,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::{
    ASSET_MANIFEST_FILE, LOGICAL_MODEL_BATCH_REPORT_FILE, LOGICAL_MODEL_BATCH_REPORT_SCHEMA,
    LOGICAL_MODEL_PUBLISH_REPORT_SCHEMA, LogicalModelBatchMapping, LogicalModelBatchPublishReport,
    ModelPublishContract, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, error::io_at,
};

#[cfg(test)]
#[path = "../../tutorial_model_install_tests.rs"]
mod tests;

mod models;
mod constants;
mod assets;
mod output;
mod types;
mod validation;
mod operations;
mod input;
mod animation;
mod state;

pub use models::{
    TUTORIAL_MODEL_ROOT, TUTORIAL_MODEL_CATALOG_PATH, TUTORIAL_MODEL_CATALOG_SCHEMA,
    TUTORIAL_MODEL_INSTALL_REPORT_SCHEMA, TutorialModelInstallOptions, TutorialModelCatalog,
    TutorialModelBatchProof, TutorialModelCatalogEntry, TutorialModelRuntimeFacts,
    TutorialModelGpuProof, TutorialModelClosureFile, TutorialModelInstallReport
};
use models::{prepare_model, glb_document};
use constants::{INSTALLER_ID, STAGING_SEQUENCE};
use assets::{
    MANIFEST_NEXT, MANIFEST_BACKUP, index_batch_mappings, exact_index_path,
    index_regular_tree, owned_path, slash_path
};
use output::INSTALL_BACKUP;
pub use output::install_tutorial_models;
use types::{PreparedFile, EvidenceRoot, EvidenceProof, ExpectedExternalFile};
use validation::{
    validate_publish_identity, validate_batch, validate_mapping, validate_selections,
    validate_manifest, validate_previous_install, reject_overlapping_roots, validate_relative,
    validate_uri, validate_source_build,
    require_sha256, invalid_error
};
use operations::{
    verify_semantic_roundtrip_hashes, reported_external_files,
    external_uri_set, verify_evidence, prepare_evidence_roots, rollback_empty_parent,
    ensure_no_stale_transaction, create_stage, canonical_plain_directory, kind_for_external, file_name, parent_slash, relative_slash, casefold, sha256, invalid
};
use input::{collect_uris, find_exact_evidence, read_regular_relative, read_u32};
use animation::verify_animation_evidence;
use state::{verify_runtime_evidence, runtime_facts};
#[cfg(test)]
use operations::screenshot_foreground;
