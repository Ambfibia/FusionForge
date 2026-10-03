//! Fail-closed installation of the exact static scene surrounding the
//! Retrotribution tutorial.
//!
//! The native world exporter emits one self-auditing directory per map tile.
//! This installer verifies those directories in full, publishes only the
//! runtime GLBs and their texture dependencies plus the static audit metadata,
//! and merges the static arrays into the already-installed native terrain
//! scenes.  The native heightmaps and the rest of each terrain tree remain
//! owned by the terrain exporter.

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result,
    error::io_at,
    runtime_metadata_cleanup::{
        ArchivedRuntimeMetadata, CLEAN_RUNTIME_METADATA_INDEX_FILE,
        CLEAN_RUNTIME_METADATA_INDEX_SCHEMA, validate_completed_cleanup_archive,
    },
    runtime_world_migration::{
        RuntimeWorldMigrationReport, sanitize_scene, validate_completed_migration,
    },
};

use crate::shared::canonical_json;

use crate::shared::transaction_stamp as unique_token;

#[cfg(test)]
mod tests;

mod assets;
mod constants;
mod output_install_with_contract;
mod output_load_previous_install;
mod types;
mod state;
mod input;
mod operations_canonicalize_migrated_tutorial_scene;
mod operations_hash_regular_file;
mod validation;
mod models;
mod projects;

pub use assets::{TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH, WORLD_MAP_STATIC_WORLD_OWNERSHIP_PATH};
use assets::{
    EXPORT_MANIFEST_SCHEMA, EXPORT_MANIFEST_FILE, STATIC_CATALOG_SCHEMA, WORLD_CATALOG_PATH, RUNTIME_WORLD_REGISTRY_PATH, RUNTIME_WORLD_REGISTRY_SCHEMA,
    CLEANUP_REVISION_INDEX_SCHEMA, ExportManifest, ExportManifestCounts, ExportManifestFile, RuntimeWorldRegistryEntry, RuntimeWorldRegistry,
    CleanupArchiveIndex, rewrite_world_catalog, rewrite_runtime_world_registry, move_path,
    publish_path, verify_manifest_file, tile_scene_path, static_metadata_path,
    relative_path_string
};
pub use constants::{
    TUTORIAL_STATIC_WORLD_OWNERSHIP_SCHEMA, TUTORIAL_STATIC_WORLD_SOURCE_BUILD,
    WORLD_MAP_STATIC_WORLD_OWNERSHIP_SCHEMA, WORLD_MAP_STATIC_WORLD_CONTRACT_SCHEMA
};
use constants::{
    WORLD_MAP_INSTALLER_ID, INSTALLER_ID, LEGACY_TUTORIAL_STATIC_WORLD_OWNERSHIP_SCHEMA,
    WORLD_SCENE_SCHEMA, STATIC_HIERARCHY_SCHEMA, STATIC_MATERIALS_SCHEMA, BASE_COVERAGE,
    STATIC_COVERAGE, CLEANUP_STATIC_REASON, EXACT_TILES
};
pub use output_install_with_contract::{
    TUTORIAL_STATIC_WORLD_INSTALL_REPORT_SCHEMA, WORLD_MAP_STATIC_WORLD_INSTALL_REPORT_SCHEMA,
    TutorialStaticWorldInstallOptions, WorldMapStaticWorldInstallOptions,
    TutorialStaticWorldInstallReport, install_tutorial_static_world,
    install_world_map_static_world
};
use output_install_with_contract::{
    EXPORT_REPORT_SCHEMA, EXPORT_STATUS, EXPORT_REPORT_FILE, ExportReport, ExportReportCounts
};
use output_load_previous_install::{
    build_install_report, load_previous_install, required_export_file, export_kind, copy_new,
    write_new
};
use types::{
    StaticWorldScope, TileContract, WorldMapStaticWorldContract, WorldReferenceScope, AuditedTile, SourcePublication,
    PublicationContent, Publication, ExpectedSceneReference, CleanupArchivedOwnership
};
pub use types::{
    TutorialStaticWorldOwnership, StaticWorldWindingRepairProof,
    TutorialStaticWorldReferenceProof, TutorialStaticWorldTileProof,
    TutorialStaticWorldOwnedFile, TutorialStaticWorldSceneProof
};
use state::RuntimeMigrationProof;
use input::{
    load_world_map_contract, load_runtime_migration_proof, load_cleanup_archived_ownership,
    collect_files, read_regular_file, read_json, parse_json
};
use operations_canonicalize_migrated_tutorial_scene::{
    stage_manifestless_publication, commit_manifestless_publication,
    scene_non_static_identity_matches, merge_scene, canonicalize_migrated_tutorial_scene,
    static_fields_blake3, build_reference_publications,
    verify_current_reference_documents, tile_coordinates, verify_reference_closure, previous_publication_paths,
    ensure_publication_paths_are_available, stage_publication, commit_publication, source_set_blake3, required_array, required_string,
    canonical_directory
};
use operations_hash_regular_file::{
    hash_regular_file, pretty_json, safe_join, is_windows_device_name, hash_bytes, invalid
};
use validation::{
    audit_tile_export,
    validate_observed_scene_hash, require_complete_reference_coverage,
    validate_installed_scene_identity, validate_cleanup_noop_source,
    validate_first_install_destinations, validate_owned_trees, validate_export_root_entries, validate_project_manifest,
    validate_owned_path, reject_symlink, validate_relative, validate_blake3,
    generated_json_error, invalid_error
};
use models::{validate_glb_dependencies, model_tile_root};
use projects::project_entry;
#[cfg(test)]
use assets::WORLD_CATALOG_SCHEMA;
#[cfg(test)]
use output_install_with_contract::install_with_contract;
