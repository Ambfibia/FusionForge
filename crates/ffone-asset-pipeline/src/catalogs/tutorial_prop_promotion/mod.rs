//! Transactional promotion of the two proven static tutorial props out of the
//! legacy `tutorial/models/mob` staging taxonomy.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ffone_skinned_model::{
    AutomatedGpuStatus, GPU_EVIDENCE_SCHEMA, GPU_RENDER_PROFILE, LogicalModelGpuEvidence,
    LogicalModelGpuFacts, SourceOutlineMode, VisualParityClaim, gpu_evidence_relative_paths,
    gpu_model_facts_from_glb,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    ASSET_MANIFEST_FILE, LOGICAL_MODEL_BATCH_REPORT_FILE, LOGICAL_MODEL_BATCH_REPORT_SCHEMA,
    LOGICAL_MODEL_PUBLISH_REPORT_SCHEMA, LogicalModelBatchMapping, LogicalModelBatchPublishReport,
    ModelPublishContract, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, error::io_at,
};

#[cfg(test)]
mod tests;

mod constants;
mod assets;
mod types;
mod state;
mod models;
mod operations;
mod systems;
mod validation;
mod input;

pub use constants::TUTORIAL_PROP_PROMOTION_SCHEMA;
use constants::{SOURCE_PACKAGE_ROOT, CANDIDATES};
use assets::{
    ASSET_ROOT_RELATIVE, build_next_manifest, index_batch, index_manifest,
    manifest_paths_below, native_path, slash_path
};
use types::{PromotionCandidate, FileIdentity, PreparedPromotion, PromotionPlan};
pub use types::{
    TutorialPropPromotionOptions, TutorialPropPromotionBlocker, TutorialPropPromotionProof,
    TutorialPropPromotionCounts, TutorialPropPromotionReport
};
pub use state::{
    TutorialPropPromotionMode, TutorialPropPromotionStatus, TutorialPropRuntimeReferenceProof
};
use state::{PreparedState, prepare_runtime_reference, RuntimeCommitState};
pub use models::TutorialPropPromotionModel;
use models::report_model;
pub use operations::promote_tutorial_props;
use operations::{
    regular_directory_exists, sha256, unique_stamp,
    invalid
};
use systems::apply_plan;
use validation::{
    reject_stale_staging, validate_plan_unchanged, validate_committed_plan,
    validate_tree_manifest, reject_overlaps, validate_relative, require_sha256, invalid_error
};
use input::{collect_tree, read_regular};
