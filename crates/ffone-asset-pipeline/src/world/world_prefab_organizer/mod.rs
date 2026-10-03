//! Reusable local-space map-object library derived from exact static-world publications.
//!
//! Static-world GLBs deliberately bake an instance world matrix into every vertex. That is the
//! correct flat runtime representation, but it makes a tree, bench, house part, or scripted prop
//! impossible to place on a new tile. This module derives a deduplicated map-object library with
//! the matrix removed from geometry. A map object owns its render and collision parts in one
//! package; a tile owns only terrain-specific data, behaviour and object placements. Every part
//! and placement retains the exact primary-source identities recorded by the static exporter, so
//! this organization layer never turns a filename guess into source authority.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use glam::{DMat3, DMat4, DVec3, DVec4};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::{
    PipelineError, RESOURCE_SET_SCHEMA, ResourceSetCatalogEntry, ResourceSetDocument, Result,
    error::io_at,
};

#[cfg(test)]
mod tests;

mod assets;
mod containers_world_prefab_resource;
mod containers_verify_world_prefab_library;
mod constants;
mod models;
mod collision;
mod types;
mod codec;
mod operations_scan_scope;
mod operations_build_library;
mod operations_accessor_layout;
mod materials;
mod state;
mod terrain;
mod input;
mod output;
mod validation;
mod projects;

pub use assets::{
    WORLD_PREFAB_CATALOG_SCHEMA, WORLD_PREFAB_CATALOG_PATH, WorldPrefabCatalogPrefab,
    MapCompositeObjectCatalogEntry, WorldPrefabCatalog
};
use assets::{map_closure_path, relative_path, path_text, slash_path};
pub use containers_world_prefab_resource::{
    WORLD_PREFAB_SCHEMA, MAP_COMPOSITE_OBJECT_SCHEMA, WORLD_PREFAB_PLACEMENTS_SCHEMA,
    WORLD_PREFAB_ORGANIZER_REPORT_SCHEMA, WORLD_PREFAB_VERIFICATION_SCHEMA, WORLD_PREFAB_TOOL,
    WorldPrefabOrganizerOptions, WorldPrefabOrganizerMode, WorldPrefabOrganizerCounts,
    WorldPrefabOrganizerReport, WorldPrefabResourceKind, WorldPrefabArtifact,
    WorldPrefabBounds, WorldPrefabSourceIdentity, WorldPrefabResource, WorldPrefabPart,
    WorldPrefabDefinition, MapCompositeObjectDefinition, WorldPrefabPlacementSetReference,
    WorldPrefabPlacement, WorldPrefabPlacementDocument, WorldPrefabVerification
};
use containers_world_prefab_resource::{PrefabPartKey, PrefabDraft, ObjectPlan};
pub use containers_verify_world_prefab_library::{
    verify_world_prefab_library, verify_world_prefab_library_to_report
};
use containers_verify_world_prefab_library::{
    CompositeMapObjectSpec, prefab_id, object_base_root
};
pub use constants::MAP_TILE_SCHEMA;
use constants::{
    STATIC_HIERARCHY_SCHEMA, SOURCE_BUILD, MAX_ALIASES, LEGACY_ADDITIVE_BLACK_VISUAL
};
use models::{
    GLB_JSON_CHUNK, GLB_BIN_CHUNK, GLTF_FLOAT, ParsedGlb, resolved_glb_textures,
    validate_direct_mesh_runtime_contract, parse_glb, unbake_glb_geometry,
    relocate_glb_textures, set_glb_resource_metadata
};
use collision::{LEGACY_COLLISION_HELPER_ASSET, LEGACY_COLLISION_HELPER_VISUALS};
pub use types::{MapTileDocument, MapReconstructionProof};
use types::{
    HierarchyDocument, HierarchyNode, ResourceKey, ResourceDraft, PlacementDraft,
    BuiltResource, Taxonomy, AccessorLayout
};
use codec::{
    HierarchyPayload, payload_has_runtime_model, is_legacy_collision_helper_payload,
    payload_aliases, encode_glb
};
use operations_scan_scope::{
    verify_artifact, verify_resource_set_artifact,
    verify_local_resource_contract, geometry_layout
};
pub use operations_scan_scope::organize_world_prefabs;
use operations_build_library::{
    build_library, authored_transform_json, normalized_blake3, taxonomy,
    propagate_visual_aliases_to_colliders, aliases_look_character_like, node_aliases, insert_alias,
    representative_score, limited_samples, resource_id
};
use operations_accessor_layout::{
    accessor_layout, dmat4, dvec3_f32, identity_matrix, identity_string_matrix, string_matrix, matrix_close, pretty_json, canonical_directory, absolute_from, safe_join,
    safe_slug, safe_identifier, append_set_hash, hash_bytes, short_hash, required_u64, le_u32,
    invalid
};
use materials::{MaterialDocument, MaterialName};
use state::Inventory;
use terrain::{
    SharedTerrainFiles, publish_shared_terrain_layers,
    publish_shared_terrain_detail_textures,
    collapse_exact_terrain_base_mip_zero_copies
};
use input::{
    parse_string_matrix, parse_tile_grid, read_vec3_accessor, discover_tutorial_metadata_root,
    collect_files, read_regular_file
};
use output::{
    publish_composite_map_objects, publish_map_tile,
    publish_shared_map_runtime_assets, write_vec3_accessor, write_json_atomic, write_new
};
use validation::{
    validate_replacement_target, validate_relative, validate_relative_path,
    generated_json_error, invalid_error
};
use projects::{absolute_under_project, project_relative};
#[cfg(test)]
use operations_scan_scope::is_legacy_additive_black_visual;
#[cfg(test)]
use terrain::collapse_exact_terrain_base_mip_zero;
