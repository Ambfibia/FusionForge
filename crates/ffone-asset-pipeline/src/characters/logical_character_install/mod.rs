//! Transactional installation of audited logical character trees into the
//! permanent Bevy asset root.
//!
//! This boundary never reads Unity data. It accepts only a native logical-model
//! candidate that passes the at-rest structural audit and an independently
//! reproducible native GPU-evidence audit.

use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use ffone_skinned_model::{
    GPU_EVIDENCE_SCHEMA, GPU_RENDER_PROFILE, LogicalModelGpuEvidence, VisualParityClaim,
    gpu_model_facts_from_glb, validate_glb_render_contract,
    validate_retrobution_fusion_eye_contract,
};

use crate::{
    ASSET_MANIFEST_FILE, GPU_EVIDENCE_AUDIT_SCHEMA, LogicalModelGpuEvidenceAuditReport,
    PipelineError, ProjectAssetFile, ProjectAssetKind, ProjectAssetManifest, Result,
    audit_logical_model_tree, error::io_at,
};

#[cfg(test)]
mod tests;

mod assets;
mod constants;
mod output;
mod types;
mod models;
mod animation;
mod materials;
mod textures;
mod operations;
mod validation;

pub use assets::{
    SEMANTIC_CHARACTER_CATALOG_SCHEMA, SEMANTIC_CHARACTER_CATALOG_PATH,
    CHARACTER_SCHEMA_UPGRADE_REPORT_PATH, SemanticCharacterCatalog
};
use assets::{
    CharacterRoute, replace_manifest, reserve_path, destination_file_path, relative_path,
    slash_path
};
use constants::STAGING_SEQUENCE;
pub use output::{
    LogicalCharacterInstallOptions, LogicalCharacterInstallReport, install_logical_characters
};
pub use types::{SemanticCharacterProofs, SemanticAuthoredRoot};
pub use models::SemanticCharacterModel;
use models::build_catalog_model;
pub(crate) use models::build_catalog_model_for_destination;
pub use animation::{SemanticRigSummary, SemanticAnimationSummary};
use animation::glb_animation_names;
pub use materials::{SemanticMaterialShaderMetadata, SemanticShader};
pub use textures::SemanticTexture;
use operations::{
    candidate_files, installed_character_files, create_stage, kind_for, semantic_kind,
    parent_slash, canonical_directory, join_relative, verify_sha256_file, string,
    required_string, integer, float_array, sha256, invalid
};
pub(crate) use operations::verify_archived_gpu_evidence;
use validation::{validate_retrobution_fusion_eye_report, validate_relative, invalid_error};
