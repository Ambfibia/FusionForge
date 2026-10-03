//! Transactional migration of the reusable map-object library to short,
//! top-level runtime routes.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::{PipelineError, Result, error::io_at, world_prefab_organizer::WorldPrefabVerification};

#[cfg(test)]
mod tests;

mod assets;
mod constants;
mod types;
mod containers;
mod operations;
mod systems;
mod models;
mod output;
mod input;
mod validation;

pub use assets::{
    OBJECT_ROUTE_NORMALIZATION_SCHEMA, ObjectRouteNormalizationOptions,
    ObjectRouteNormalizationReport, ObjectRouteNormalizationCounts, ObjectRoutePathLengths,
    ObjectPackageRoute
};
use assets::{
    path_length_report, insert_route, stage_object_path, checked_absolute_path,
    asset_relative, relative_path, slash_path
};
use constants::MAX_COMPONENT_LEN;
use types::NormalizationPlan;
pub use containers::normalize_object_routes;
use operations::{
    build_plan, stage_map_json, rollback, replace_paths, canonical_directory, checked_join,
    create_parent, pretty_json, normal_components, hash, invalid
};
use systems::{apply_plan, refresh_artifacts};
use models::rewrite_glb_uris;
use output::{write_report, write_new, write_replace};
use input::{collect_files, read_file, read_json, read_u32};
use validation::{generated_json_error, invalid_error};
#[cfg(test)]
use operations::{compact_slug, allocate_component};
