//! Lossless static-world publication for legacy FusionFall map scenes.
//!
//! This is deliberately separate from the interactive scene preview.  Preview
//! budgets, rounded vertices, resized textures, best-effort pointer handling,
//! and partially resolved dependency graphs are all invalid inputs here.
//! Publication succeeds only after every selected renderer/collider has a
//! complete mesh closure, every material texture has an exact decoded mip
//! chain, and the fresh output tree verifies byte-for-byte.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use serde_json::{json, Map as JsonMap, Value as JsonValue};

use crate::logical_model_material::{self, ExactMaterialExport};

use super::{
    preview::{
        compose_matrix, converted_position, converted_quaternion, converted_scale, extract_mesh,
        mat_mul, transform_normal, transform_point, Matrix4, MeshData,
    },
    unity::{
        collect_archive_dependencies, object_name, value_array, ObjectKey, UnityEnvironment,
        UnityValue,
    },
    world,
};

#[cfg(test)]
mod tests;

mod output_export_native_static_world;
mod output_export_behaviours;
mod constants;
mod materials;
mod assets;
mod types;
mod containers;
mod codec;
mod textures;
mod models;
mod operations_verify_complete_output;
mod operations_effective_scene_active;
mod input;
mod animation;
mod validation;

use output_export_native_static_world::EXPORT_SCHEMA;
pub use output_export_native_static_world::{
    NativeStaticWorldExportOptions, NativeStaticWorldExportReport, ExportCounts,
    export_native_static_world, NativeStaticBehaviourExportOptions,
    export_native_static_behaviours
};
use output_export_behaviours::{export_behaviours, copy_tree_exact, write_new_file};
use constants::{
    HIERARCHY_SCHEMA, BEHAVIOUR_SCHEMA, SCENE_SCHEMA, IDENTITY_TRANSFORM,
    BEHAVIOUR_COMPONENT_TYPES, BEHAVIOUR_OWNERSHIP_KEYS
};
use materials::{
    MATERIAL_SCHEMA, renderer_material_slots, gltf_material_from_exact
};
use assets::{
    CATALOG_SCHEMA, MANIFEST_SCHEMA, append_index_accessor,
    verify_static_scene_against_catalog, finalize_report_and_manifest,
    slash_path
};
use types::{
    JsonTransform, TransformRecord, ComponentRecord, SourceIdentity, SceneExtraction,
    BehaviourComponent, ScratchDirectory, BehaviourCounts, TileLayout
};
use containers::{
    GameObjectRecord, add_game_object_payloads, collect_prefab_instance,
    collect_effect_prefab_closure, unity_value_to_json,
    parse_map_bundle_identity, object_type, object_key_from_optional_pointer
};
use codec::{PayloadKind, PendingPayload, PublishedPayload, encode_glb};
use textures::{TexturePublication, validate_png};
use models::{
    read_exact_mesh, build_single_root_glb, validate_single_root_glb, parse_glb,
    required_mesh_pointer, ensure_mesh_type
};
use operations_verify_complete_output::{
    extract_world_environment, published_visual_winding_needs_reversal, append_vec3_accessor,
    append_vec2_accessor, pad_four, f64_vec3_to_f32, color_factor, json_lossless_number,
    source_json, enrich_native_scene, verified_existing_static_publication,
    matches_published_bytes, verify_complete_output, output_file_kind,
    canonical_directory, canonical_file, blake3_hex, unique_nonce, join_relative,
    replace_regular_file, pretty_json_bytes, json_string, safe_name, nonempty_name, source_id
};
use operations_effective_scene_active::{
    source_identity, component_pointer, enabled_component, authored_transform,
    world_matrix_for_transform, effective_scene_active, ensure_finite_matrix,
    required_json_string
};
#[cfg(windows)]
use operations_effective_scene_active::metadata_is_reparse_point;
#[cfg(not(windows))]
use operations_effective_scene_active::metadata_is_reparse_point;
use input::{
    collect_scene, read_u32, resolve_script_identity,
    collect_regular_files, resolve_tile_layout, resolve_case_exact
};
use animation::resolve_animation_clip_identity;
use validation::{reject_links_recursive, validate_fresh_output, validate_relative_path};
#[cfg(test)]
use containers::{scripted_prefab_runtime_active, scripted_prefab_runtime_transform};
