//! Normalizes published map objects into self-contained resource sets.
//!
//! A set owns one copy of every texture atlas used by its member objects. Each
//! member continues to own its visual/collision GLBs and object definition.
//! This is deliberately a native post-publication step: legacy containers are
//! evidence only and are never consulted here.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::{PipelineError, Result, error::io_at};
#[cfg(test)]
mod tests;

mod constants;
mod types;
mod assets;
mod models;
mod containers;
mod textures;
mod input;
mod operations_organize_player_item_sets;
mod operations_normalize_existing_player_item_sets;
mod entities;
mod systems;
mod output;
mod validation;

pub use constants::{
    RESOURCE_SET_SCHEMA, RESOURCE_SET_ORGANIZER_REPORT_SCHEMA, PLAYER_ITEM_SET_REPORT_SCHEMA,
    PLAYER_ITEM_SCHEMA
};
pub use types::{
    ResourceSetArtifact, ResourceSetMember, ResourceSetDocument, ResourceSetOrganizerReport,
    PlayerItemDefinition, PlayerItemSetOrganizerReport
};
use types::{PlannedSet, PlayerSetPlan};
pub use assets::{ResourceSetCatalogEntry, PLAYER_ITEM_SET_CATALOG_SCHEMA, PlayerItemSetCatalog};
use assets::{
    sort_player_catalog_models, player_catalog_provenance_files,
    update_player_catalog_provenance, player_stage_path,
    verify_player_item_sets_at_asset_root, update_catalog, stage_path_for_rooted,
    asset_relative, checked_asset_path, relative_path, slash_path
};
pub use models::PlayerItemCatalogModel;
use models::{PlayerModel, read_glb_json_bytes, rewrite_glb_uris, read_glb_json};
use containers::MapObject;
use textures::{
    TextureUse, TextureDestination, PlayerTexture, register_player_texture,
    player_model_texture_roots, copy_player_texture_mips, texture_groups,
    semantic_texture_name, copy_texture_mips, object_texture_roots, is_owned_texture_file
};
use input::{
    UnionFind, load_player_models, load_map_objects, collect_staged_artifacts, collect_files, read_u32, read_json
};
pub use operations_organize_player_item_sets::{
    organize_resource_sets, organize_player_item_sets
};
use operations_normalize_existing_player_item_sets::{
    normalize_existing_player_item_sets, best_matching_player_set,
    appearance_category, appearance_family, artifact_from_player_stage,
    player_reference_json_files, verify_resource_artifact, file_stem, plan_sets,
    common_semantic_suffix, rewrite_json_file_strings, replace_paths, artifact_from_staged,
    artifact_from_live, meaningful_image_name, one_or, strip_variant, allocate_name,
    safe_slug, normalized_components, pretty_json, canonical_directory, canonical_file,
    hash_bytes, invalid
};
pub use operations_normalize_existing_player_item_sets::verify_player_item_sets;
use entities::attach_avatar_texture_routes;
use systems::{
    refresh_map_json_references, refresh_artifact_objects, refresh_staged_artifact_objects
};
use output::{write_new, write_replace, copy_new_or_equal};
use validation::{generated_json_error, invalid_error};
