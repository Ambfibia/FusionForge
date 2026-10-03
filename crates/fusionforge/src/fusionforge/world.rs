use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use serde_json::{json, Value as JsonValue};

use super::{
    coordinates::native_coordinate_contract_json,
    extract_bundle, inspect_bundle,
    preview::{
        compose_matrix, converted_position, converted_quaternion, converted_scale, decode_texture,
        identity_matrix, image_to_data_url, material_preview, mesh_to_preview, mesh_vertex_count,
        object_key_from_pointer, quaternion_json, summarize_collider, summarize_transform,
        terrain_to_document, transform_point, Matrix4,
    },
    unity::{
        collect_archive_dependencies, normalize_bundle_name, object_name, pointer_summary,
        value_array, Asset, ObjectKey, UnityEnvironment, UnityValue,
    },
};

#[cfg(test)]
mod tests;

mod constants;
mod collision;
mod models;
mod types;
mod operations_build_scene_preview;
mod operations_build_scene_document;
mod projects;
mod assets;
mod containers;
mod textures;
mod terrain;
mod codec;
mod materials;
mod input;

use constants::{WORLD_PREVIEW_MAX_MESHES, WORLD_PREVIEW_MAX_VERTICES};
use collision::WORLD_PREVIEW_MAX_COLLIDER_VERTICES;
use models::{
    WORLD_PREVIEW_MAX_SINGLE_MESH_VERTICES, mesh_preview_vertex_count,
    append_prefab_mesh_previews
};
pub use types::WorldInspectOptions;
pub use operations_build_scene_preview::inspect_world_bundles;
use operations_build_scene_preview::{
    display_size, summarize_named, is_water_candidate
};
use operations_build_scene_document::{
    build_scene_document, component_pointer, mat_mul_safe,
    format_signed_decimal_component, map_tile_id
};
pub(crate) use operations_build_scene_document::extract_archives;
use projects::session_id;
pub(crate) use assets::{normalize_path, build_archive_candidate_index, build_archive_index};
use assets::{
    inspect_asset, note_missing_asset, tile_id_from_scene_asset_name,
    find_manifest_bundle_for_archive
};
pub(crate) use containers::extract_bundle_to_session;
use containers::{
    summarize_game_object, parse_map_bundle_name, find_map_bundle,
    infer_resource_bundle, direct_bundle_files, find_bundle_for_archive
};
use textures::summarize_texture;
use terrain::summarize_terrain;
use codec::limit_scene_preview_payload;
use materials::collect_mesh_material_ids;
use input::{parse_signed_decimal_component, find_file_recursive, collect_neighbor_map_bundles};
#[cfg(test)]
use operations_build_scene_document::transform_component_json;
