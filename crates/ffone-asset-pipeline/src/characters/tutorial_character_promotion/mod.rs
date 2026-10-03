//! Transactional promotion of proven tutorial character packages into the
//! general semantic character tree.
//!
//! The candidate list is intentionally closed. Props remain tutorial-owned and
//! a missing model is never synthesized. Every promoted package must still be
//! byte-identical to its checked-in logical-model candidate, covered by an
//! exact source document, publish contract, GPU evidence JSON and screenshot,
//! and present in the project manifest with matching identities.

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
    ProjectAssetManifest, Result, RuntimeCharacterCategory, RuntimeCharacterModel,
    SEMANTIC_CHARACTER_REGISTRY_PATH, SEMANTIC_CHARACTER_REGISTRY_SCHEMA,
    SemanticCharacterRegistry, error::io_at,
};

#[cfg(test)]
mod tests;

mod constants;
mod types;
mod operations;
mod state;
mod models;
mod assets;
mod systems;
mod validation;
mod input;

pub use constants::TUTORIAL_CHARACTER_PROMOTION_SCHEMA;
use constants::{TUTORIAL_CHARACTER_ROOT, CANDIDATES};
use types::{PromotionCandidate, FileIdentity, PreparedPromotion, PromotionPlan};
pub use types::{
    TutorialCharacterPromotionOptions, TutorialCharacterPromotionBlocker,
    TutorialCharacterPromotionProof, TutorialCharacterPromotionCounts,
    TutorialCharacterPromotionReport
};
use operations::{
    candidate, unique_stamp, invalid
};
pub use operations::promote_tutorial_characters;
pub use state::{TutorialCharacterPromotionMode, TutorialCharacterPromotionStatus};
use state::PreparedState;
pub use models::TutorialCharacterPromotionModel;
use models::proof_glb_true_name;
use assets::{
    update_registry_manifest_entry, index_batch, index_manifest, manifest_paths_below,
    native_path, slash_path
};
use systems::apply_plan;
use validation::{
    validate_registry, validate_registry_manifest_identity, validate_tree_manifest,
    reject_overlaps, validate_relative, require_sha256, invalid_error
};
use input::{collect_tree, read_regular};
#[cfg(test)]
use operations::category_directory;
