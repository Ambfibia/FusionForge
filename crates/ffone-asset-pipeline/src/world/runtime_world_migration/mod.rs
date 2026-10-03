use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{self, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, error::io_at,
    runtime_metadata_cleanup::validate_completed_cleanup_archive,
};

use crate::shared::transaction_stamp;

#[cfg(test)]
mod tests;

mod assets;
mod state;
mod constants;
mod types;
mod validation;
mod input;
mod operations;
mod terrain;
mod textures;
mod systems;
mod containers;
mod output;

pub use assets::{
    RUNTIME_WORLD_REGISTRY_SCHEMA, RUNTIME_WORLD_REGISTRY_PATH, LEGACY_WORLD_CATALOG_PATH,
    RUNTIME_WORLD_ARCHIVE_INDEX_SCHEMA, RuntimeWorldRegistryEntry, RuntimeWorldRegistry,
    LegacyWorldCatalogHashDrift, RuntimeWorldRegistryArtifact
};
use assets::{
    LEGACY_WORLD_CATALOG_SCHEMA, RuntimeWorldArchiveIndex, CompletedRuntimeWorldArchiveIndex,
    LegacyWorldCatalog, LegacyWorldCatalogEntry, manifest_index, verified_manifest_bytes,
    native_path
};
pub use state::{
    RUNTIME_WORLD_MIGRATION_REPORT_SCHEMA, RuntimeWorldMigrationMode,
    RuntimeWorldMigrationOptions, RuntimeWorldContentReference, RewrittenRuntimeWorldMetadata,
    RuntimeWorldMigrationCounts, RuntimeWorldMigrationReport, migrate_runtime_world
};
use state::{PreparedRuntimeRewrite, RuntimeWorldMigrationPlan};
use constants::{
    WORLD_ARCHIVE_DIRECTORY, ARCHIVE_FILES_DIRECTORY, ARCHIVE_ORIGINALS_DIRECTORY,
    ARCHIVE_NEXT_DIRECTORY
};
pub use types::ArchivedWorldTechnicalMetadata;
use types::LegacyWorldEnvironmentReference;
pub(crate) use validation::validate_completed_migration;
use validation::{
    validate_catalog_identity, audit_sanitized_json,
    validate_archive_coverage, reject_stale_transactions,
    validate_source_build, invalid_error
};
use input::{
    read_json_file, collect_regular_files,
    collect_world_json_paths, resolve_document_reference
};
use operations::{
    required_original_hash, push_expected_hash_drift, sort_hash_drifts, build_plan, replace_provenance_words,
    technical_metadata_reason, rollback, normalized_relative, remove_required,
    remove_optional, pretty_json_bytes, pretty_value_bytes, verify_file_identity,
    invalid
};
pub(crate) use operations::sanitize_scene;
use terrain::sanitize_terrain;
use textures::strip_mip_source_evidence;
use systems::apply_plan;
use containers::{object_mut, child_object_mut};
use output::{copy_file_new_verified, write_bytes_new, write_json_new};
