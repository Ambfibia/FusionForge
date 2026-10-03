use std::{
    collections::{BTreeMap, BTreeSet, HashSet, VecDeque},
    env, fs,
    fs::OpenOptions,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use image::{codecs::png::PngEncoder, ColorType, ImageEncoder, RgbaImage};
use serde_json::{json, Value as JsonValue};

use super::{
    build_patch, content_pack,
    modding::{apply_mesh_import, ImportedMesh},
    preview::{decode_texture, mesh_to_obj},
    unity::{
        object_name, pair_name_value, value_array, Asset, ObjectInfo, ObjectKey, Pointer,
        UnityEnvironment, UnityValue,
    },
    world::{inspect_world_bundles, WorldInspectOptions},
};

#[cfg(test)]
mod session_storage_tests;

mod operations_run_cli;
mod operations_inspect_world;
mod models;
mod commands;
mod assets;
mod types;
mod input;
mod animation;
mod projects;
mod containers_merge_character_bundle_assets;
mod containers_export_object;
mod state;
mod validation;
mod textures;
mod terrain;
mod output;

pub use operations_run_cli::run_cli_from_env;
use operations_run_cli::{
    run_large_stack_task, cli_metadata_preload_range,
    cli_merge_assetbundle_replacements_to_primary,
    pointer_label, local_pointer_exists,
    gameobject_transform_pointer, resolved_body, transform_relative_paths,
    snapshot_key_type, clear_readonly_recursive, add_snapshot_key,
    snapshot_pointer_candidate_keys
};
pub(super) use operations_run_cli::{run_cli, scoped_evidence_batch};
use operations_inspect_world::{
    npc_snapshot_expected_types, inspect_world, show_gameobject, value_to_bytes, safe_name,
    fix_extension_for_type, color_for_type, dot_escape
};
use models::{
    publish_native_logical_model, convert_native_model, plan_logical_model_exports,
    preview_container_model, export_logical_model_source, export_logical_model_source_batch,
    export_equipment_model_sources, export_logical_model_sources,
    audit_logical_model_root_transforms, replace_mesh
};
use commands::normalize_command;
use assets::{
    catalog_logical_models, cli_normalized_asset_path,
    patch_caching_manifest_bundles, remove_caching_manifest_bundles, show_caching_manifest,
    collect_pointers_with_path, asset_ref_label, index_client_project_command,
    npc_snapshot_container_output_path, npc_snapshot_dependency_output_path,
    resolved_object_index_info, required_path, fix_export_path_for_type
};
use types::{LoadedInput, ExtractOptions};
use input::{
    load_input, load_input_with_sibling_dependencies, collect_transform_relative_paths,
    parse_extract_options, find_first_type, find_gameobject_root, collect_pointers, parse_i64
};
pub(super) use animation::animation_clip_names;
use animation::validate_animation_component;
use projects::cli_session_dir;
use containers_merge_character_bundle_assets::{
    cli_set_unity_object_i64, cli_assetbundle_value_has_container_prefix,
    merge_character_bundle_assets,
    resolve_source_relative_container, dump_object_evidence,
    dump_object, unity_extract, resolved_object_key_for_snapshot,
    resolve_container_export_target
};
pub(crate) use containers_merge_character_bundle_assets::{
    snapshot_npc_bundle, snapshot_npc_bundle_quiet
};
use containers_export_object::{
    cli_container_expected_export_types, export_object, container_entries,
    resolved_object_info, find_object, unity_to_json
};
use state::cli_rewrite_selected_external_pointers_to_local;
use validation::{
    validate_bundle_refs, validate_object_sizes,
    validate_evidence_source_alias, audit_world_transform_contract
};
use textures::{
    export_exact_texture, export_exact_texture_batch, snapshot_material_texture_refs,
    write_snapshot_png_output, write_png, write_png_output
};
use terrain::{dump_terrain, export_native_terrain, replace_terrain};
use output::{
    export_native_static_world, export_native_static_behaviours, export_native_terrains,
    export_logical_prop_sources, write_snapshot_output, write_graph_node, write_or_print_json,
    write_output
};
