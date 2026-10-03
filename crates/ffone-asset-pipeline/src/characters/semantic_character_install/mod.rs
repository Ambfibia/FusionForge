//! Transactional publication of individually GPU-accepted semantic characters.
//!
//! The raw `models/` recovery layer is deliberately outside this installer's
//! ownership. Exact legacy KFM routes are reconstructed from the native batch
//! report and classified with the consolidated NPC/Nano tables. Only GLB/PNG
//! runtime closure files and one registry are installed.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    ASSET_MANIFEST_FILE, GPU_EVIDENCE_AUDIT_SCHEMA, LOGICAL_MODEL_TREE_AUDIT_SCHEMA,
    LogicalModelGpuEvidenceAuditReport, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile,
    ProjectAssetKind, ProjectAssetManifest, Result, SemanticAnimationSummary, SemanticAuthoredRoot,
    SemanticRigSummary, SemanticShader, audit_logical_model_tree,
    error::io_at,
    logical_character_install::{
        build_catalog_model_for_destination, verify_archived_gpu_evidence,
    },
};

use crate::shared::normalize_route;

#[cfg(test)]
mod tests;

mod assets;
mod output;
mod constants;
mod state;
mod types;
mod textures;
mod materials;
mod models;
mod operations_plan_manifestless_character_content;
mod operations_regular_files;
mod validation;
mod input;

pub use assets::{
    SEMANTIC_CHARACTER_REGISTRY_SCHEMA, SEMANTIC_CHARACTER_REGISTRY_PATH,
    CharacterRegistryProofs, SemanticCharacterRegistry
};
use assets::{
    exact_character_route_evidence, classify_route,
    exact_route_from_mapping, exact_route_from_source_blocker, ensure_unique_registry_models,
    is_managed_path, sibling_transaction_path, relative_path, slash_path
};
pub use output::{
    SEMANTIC_CHARACTER_INSTALL_REPORT_SCHEMA, SemanticCharacterInstallOptions,
    SemanticCharacterInstallCounts, CharacterInstallInput, SemanticCharacterInstallReport,
    install_semantic_characters
};
use output::{
    RuntimeCopy, write_new_file, write_exact_new_file
};
use constants::{
    BATCH_REPORT_FILE, TABLE_SET_SCHEMA, CONSOLIDATED_TABLE, COORDINATE_CONTRACT,
    SEMANTIC_CHARACTER_SOURCE_PREFIX, MANAGED_TARGETS, NEW_TARGETS,
    MANIFESTLESS_EXTERNAL_NON_PACKAGE_ROOTS, TRANSACTION_SEQUENCE
};
use state::{
    RUNTIME_CHARACTER_PACKAGE_ROOTS, runtime_closure_files, runtime_character_package_root
};
pub use state::RuntimeCharacterCategory;
pub use types::{
    PublishedCharacterClassification, PublishedCharacter, CharacterSkipReason,
    SkippedCharacter
};
use types::{TableRole, TableReference, Classification, PreservedCharacterContent};
pub use textures::PublishedCharacterTexture;
pub use materials::PublishedCharacterMaterialSummary;
pub use models::RuntimeCharacterModel;
use models::route_from_model_field;
use operations_plan_manifestless_character_content::{
    legacy_aliases, plan_preserved_character_content, plan_manifestless_character_content,
    stage_preserved_character_content, commit_manifestless_transaction, commit_transaction,
    create_transaction_directory, canonical_output_file, input_proof, string_array,
    value_array, string, required_string, int_field, pretty_json, create_parent
};
use operations_regular_files::{
    reserve_destination, regular_files, canonical_directory, join_relative, sha256, invalid
};
use validation::{
    validate_existing_managed_tree, validate_semantic_directories, validate_file_name,
    validate_relative, invalid_error
};
use input::read_file;
