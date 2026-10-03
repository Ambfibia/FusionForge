//! Transactional exact-byte deduplication for an already published map library.
//!
//! The legacy organizer normally performs this work while publishing from the
//! offline world staging tree. Release workspaces intentionally do not retain
//! that staging, so this migration operates on the self-contained
//! `assets/game/map` closure and refreshes every dependent acceptance hash.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::{PipelineError, Result, error::io_at};

#[cfg(test)]
mod tests;

mod terrain;
mod constants;
mod types;
mod operations_build_shift_restore_plan;
mod operations_verify_physical_library;
mod input;
mod validation;
mod output;
mod textures;
mod systems;
mod assets;

pub use terrain::{
    PUBLISHED_TERRAIN_DEDUP_SCHEMA, PUBLISHED_TERRAIN_SHIFT_RESTORE_SCHEMA,
    PublishedTerrainDedupOptions, PublishedTerrainDedupCounts, PublishedTerrainDetailRoute,
    PublishedTerrainDedupReport, PublishedTerrainDedupVerification,
    PublishedTerrainShiftRestoreOptions, PublishedTerrainShiftRestoreCounts,
    PublishedTerrainShiftRestoreTile, PublishedTerrainShiftRestoreReport,
    dedupe_published_terrain, restore_published_terrain_shifts
};
use terrain::validate_migrated_terrain_references;
use constants::{
    DETAIL_SHARED_PREFIX, DETAIL_SHARED_DIRECTORY, LOCK_FILE, STAGE_DIRECTORY,
    BACKUP_DIRECTORY
};
use types::{
    DetailRoutes, PackageFile, DetailPackage, TileSource, MigrationPlan, TransactionLock,
    RewrittenTile, PhysicalArtifact
};
use operations_build_shift_restore_plan::{
    build_shift_restore_plan, build_plan
};
use operations_verify_physical_library::{
    verify_physical_library, exact_hashed_files_match, normalized_hash, verify_artifact,
    verify_artifact_from_roots, artifact, insert_bytes, acquire_lock,
    clone_tree, replace_staged_file, map_relative, remove_transaction_directory,
    canonical_directory, checked_join, safe_slug, required_string, pretty_json,
    append_set_hash, hash, invalid
};
use input::{
    load_published_tile_core, load_tile_source,
    discover_physical_artifacts, collect_files, read_metadata, read_file
};
use validation::{
    validate_shift_restore_identity, validate_detail_identity, validate_prototype_references,
    validate_all_scene_links, validate_migrated_map, validate_scene_link, validate_relative,
    validate_relative_path, generated_json_error, invalid_error
};
use output::{publish_shared_package, write_new, write_replace};
use textures::collapse_base_mip_zero;
use systems::apply_plan;
use assets::{
    ensure_manifest_file, read_asset_from_roots, verify_source_catalog, checked_report_path,
    asset_relative, slash_path
};
#[cfg(test)]
use operations_build_shift_restore_plan::{canonical_shift_encoding, allocate_package_roots};
