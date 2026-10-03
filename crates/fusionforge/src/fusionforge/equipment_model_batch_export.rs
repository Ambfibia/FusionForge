//! Transactional export of every table-owned player-equipment model.
//!
//! This is an offline Unity-data boundary. The output contains only exact
//! `ffone.logical-model-source.v1` documents plus a compact ownership manifest;
//! the Bevy runtime never opens the legacy bundles.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ffone_skinned_model::minimal_windows_glb_filename;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::logical_model_catalog::normalize_logical_model_route;

#[cfg(test)]
mod tests;

mod models;
mod constants;
mod types;
mod containers;
mod assets;
mod input;
mod validation;
mod output;
mod operations;

pub use models::{
    EQUIPMENT_MODEL_SOURCE_BATCH_SCHEMA, EquipmentModelSourceBatchManifest,
    EquipmentModelSourceBatchCounts, EquipmentModelSourceExported,
    EquipmentModelSourceBlocker, export_equipment_model_sources_batch,
    export_equipment_model_sources_batch_limited
};
use constants::{
    SEMANTIC_PLAN_SCHEMA, LOGICAL_SOURCE_SCHEMA, FAMILY, SEMANTIC_PREFIX, ALLOWED_CATEGORIES
};
pub use types::{
    EquipmentBatchTiming, EquipmentTimingSummary, EquipmentTableRowProof,
    EquipmentPhysicalTarget, EquipmentSourceFacts
};
use types::{SemanticPlan, StagingResult, PublicationResult};
pub use containers::{
    EquipmentBundleWarmupTiming, EquipmentBundleProof, EquipmentContainerOwner
};
use containers::{BundleEntry, bundle_proofs};
pub use assets::EquipmentRouteTiming;
use assets::{
    BundleIndex, BundleAsset, RoutePlan, RouteCandidate, StagedRoute, collect_route_plans,
    collect_route_occurrences, stage_route_sources, source_relative_path,
    route_stem_qualifier, resolve_bundle_index_input, project_dir_for_bundle_index,
    batch_manifest_path, slash_path
};
use input::resolve_routes;
use validation::{validate_exact_source, reject_existing_destination};
use output::{publish_staged_routes, write_new_json, write_new_bytes};
use operations::{
    destination_key, source_stage_blocker, staging_paths, sort_blockers, remove_staging,
    rename_with_transient_permission_retry, required_owned_string, required_u64, required_i64,
    u64_count, elapsed_milliseconds, timing_summary, sha256_hex, portable_key
};
#[cfg(test)]
use types::SemanticEntity;
