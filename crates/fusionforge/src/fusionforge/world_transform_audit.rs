//! Read-only, catalog-wide audit of authored world transform semantics.
//!
//! The audit starts from each physical `Map_*.unity3d` bundle and follows the
//! serialized archive dependency closure.  It never guesses a same-coordinate
//! `DongResources` pair and it never exports, recenters, rescales or mutates a
//! production asset.  Every reported value is converted through the shared
//! `coordinates.rs` `H=diag(-1,1,1)` contract.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::Value as JsonValue;

use super::{
    coordinates::{
        native_coordinate_contract_json, unity_to_native_quaternion, unity_to_native_scale,
        unity_to_native_vec3,
    },
    unity::{
        collect_archive_dependencies, normalize_bundle_name, object_name, vector, UnityEnvironment,
        UnityValue,
    },
    world::{
        build_archive_candidate_index, extract_archives, extract_bundle_to_session, normalize_path,
    },
};

#[cfg(test)]
mod tests;

mod validation;
mod constants;
mod collision;
mod containers;
mod types;
mod projects;
mod operations;
mod assets;
mod input;
mod output;

pub use validation::{
    WORLD_TRANSFORM_AUDIT_SCHEMA, WorldTransformAuditReport, WorldCatalogAuditCounts,
    MapTransformAuditCounts, MapTransformAudit, WorldAuditError,
    audit_world_transform_contract
};
use validation::is_owned_audit_temp_dir;
use constants::{APPROX_EPSILON, SINGULAR_EPSILON};
pub use collision::{ColliderCoordinateContract, ColliderException};
use collision::audit_collider;
pub use containers::DependencyBundleProof;
use containers::{game_object_name, dependency_bundle_proof, is_map_bundle};
pub use types::{AmbiguousDependencyArchive, RootOrigin, NativeTrs, TransformException};
use projects::{remove_owned_map_session, failed_map_session_report};
use operations::{
    quaternion, append_side_errors, effective_archive_candidates, aggregate_counts,
    expected_native_tile_horizontal_origin, file_name, approx_eq, blake3_file
};
use assets::{scene_asset_belongs_to_map, readable_absolute_path};
use input::parse_signed_decimal;
use output::write_report_exclusive;
