//! Audited asset-level repair for legacy static-world triangle winding.
//!
//! The original publication reflected Unity geometry into native space but a
//! subset of exporter revisions left visual triangle order opposed to the
//! authored vertex normals. This module repairs the GLB index accessors by
//! value, rewrites every dependent scene/registry hash, and publishes an
//! explicit derivation proof. Source Unity containers remain offline and
//! immutable.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::{
    PipelineError, Result,
    error::io_at,
    runtime_metadata_cleanup::ArchivedRuntimeMetadata,
    tutorial_static_world_install::{
        StaticWorldWindingRepairProof, TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH,
        TUTORIAL_STATIC_WORLD_SOURCE_BUILD, TutorialStaticWorldOwnership,
        WORLD_MAP_STATIC_WORLD_OWNERSHIP_PATH,
    },
};

use crate::shared::canonical_json;

#[cfg(test)]
mod tests;

mod constants;
mod assets;
mod models;
mod state;
mod types;
mod commands;
mod operations_repair_static_world_winding;
mod operations_accessor_layout;
mod validation;
mod input;
mod projects;
mod output;

pub use constants::{
    STATIC_WORLD_WINDING_REPAIR_SCHEMA, STATIC_WORLD_WINDING_REPAIR_PROOF_SCHEMA,
    STATIC_WORLD_WINDING_REPAIR_TOOL, STATIC_WORLD_WINDING_ARCHIVE_SCHEMA
};
use constants::{REVISION_REPORT_SCHEMA, REVISION_PLAN_SCHEMA, OPERATION, SOURCE_CONTRACT};
use assets::{
    REVISION_INDEX_SCHEMA, RUNTIME_WORLD_PATH, RevisionIndex,
    repaired_tutorial_ownership_path, read_index_accessor, reverse_index_accessor, path_text
};
use models::{
    GLTF_UNSIGNED_BYTE, GLTF_UNSIGNED_SHORT,
    GLTF_UNSIGNED_INT, GLTF_FLOAT, ParsedGlb, analyze_glb, reverse_glb_indices,
    is_visual_glb
};
use state::rewrite_runtime_world;
pub use state::StaticWorldWindingRepairMode;
pub use types::{
    StaticWorldWindingRepairOptions, StaticWorldWindingRepairReport,
    StaticWorldWindingRepairScopeReport, StaticWorldWindingRepairCounts,
    StaticWorldWindingRepairFile, StaticWorldWindingVerification,
    StaticWorldWindingArchiveReport
};
use types::{RevisionReport, RevisionPlanIdentity, GeometryStats, ScopeWork, AccessorLayout};
pub use commands::StaticWorldWindingRepairAction;
pub use operations_repair_static_world_winding::{
    archive_repaired_tutorial_winding_ownership, verify_static_world_winding,
    repair_static_world_winding
};
use operations_accessor_layout::{
    accessor_layout, component_width, required_array, required_u64, le_u32, dot3,
    static_fields_blake3, initialize_backup_root, backup_and_replace, append_set_hash,
    safe_join, canonical_directory, pretty_json, hash_bytes, invalid
};
use validation::{
    validate_winding_revision, require_repair_improves, generated_json_error, invalid_error
};
use input::{
    load_world_map_scope, load_tutorial_scope,
    read_vec3_accessor, read_regular_file, read_json, parse_json
};
use projects::{absolute_under_project, project_relative};
use output::{write_json_replace, write_new};
#[cfg(test)]
use models::{GLB_JSON_CHUNK, GLB_BIN_CHUNK};
