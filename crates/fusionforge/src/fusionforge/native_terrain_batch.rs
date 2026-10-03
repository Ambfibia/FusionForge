use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fs::{self, OpenOptions},
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use image::{DynamicImage, GrayImage, ImageFormat};
use serde::Serialize;
use serde_json::{json, Value as JsonValue};

use super::{
    extract_bundle,
    native_terrain::export_native_terrain_exact_in_caller_staging,
    unity::{
        collect_archive_dependencies, normalize_bundle_name, object_name, pair_name_value,
        value_array, ObjectKey, Pointer, UnityEnvironment, UnityValue,
    },
    world::{build_archive_index, extract_archives, normalize_path as normalize_filesystem_path},
};

#[cfg(test)]
mod tests;

mod constants;
mod terrain_exact_terrain_placement_audit;
mod terrain_resolve_scene_owned_terrain_source;
mod types;
mod containers;
mod output;
mod operations_enrich_native_terrains_batch;
mod operations_preflight_build;
mod operations_link_scene_instance;
mod operations_hard_link_tree_exact;
mod validation;
mod input;
mod state;
mod assets;
mod codec;
mod localization;
mod systems;
mod entities;
mod projects;

use constants::{
    BATCH_SCHEMA, SCENE_INSTANCE_SCHEMA, PUBLICATION_PLAN_SCHEMA, ENVIRONMENT_SCHEMA,
    ENRICHMENT_SCHEMA, TUTORIAL_RESOURCE_FILE
};
pub use terrain_exact_terrain_placement_audit::{
    NativeTerrainBatchOptions, NativeTerrainBatchManifest, NativeTerrainEnrichmentSummary
};
use terrain_exact_terrain_placement_audit::{
    TerrainIdentity, ResolvedTerrainSource, PublishedTerrainIdentity, TerrainScope,
    published_terrain_identities, sibling_terrain_render_components,
    exact_terrain_placement_audit, terrain_detail_runtime_contract,
    terrain_graph_closure_blockers,
    exact_owned_terrain_identity
};
use terrain_resolve_scene_owned_terrain_source::{
    resolve_scene_owned_terrain_source, terrain_tile_id_from_container_route,
    terrain_dimensions, terrain_render_contract, terrain_collider_contract
};
pub use types::{
    BatchScope, BatchSource, BatchCounts, BatchExported, BatchBlocked, SourceFileEvidence,
    SceneInstanceSummary, ProvenanceFinding
};
use types::{
    TileSource, Preflight, LoadedTile, LoadedMapScene, SceneLink, SceneSidecar,
    EnvironmentScan, EnvironmentScanOutcome, TileOutcome
};
use containers::{
    ExactSceneObject, exact_scene_object_from_body, exact_scene_object_candidates,
    exact_scene_object_document, raw_object_blake3,
    resolved_object_name, unity_to_json, unity_rotation_to_native, replace_json_object_field
};
pub use output::export_native_terrains_batch;
use output::{write_json_new, write_bytes_new};
pub use operations_enrich_native_terrains_batch::enrich_native_terrains_batch;
use operations_preflight_build::{
    process_tiles_parallel,
    directory_has_dong_resources, preflight_build, normalized_dependency_requests,
    scene_dependency_alias_required
};
use operations_link_scene_instance::{
    link_scene_instance, pointer_json, any_pointer, component_pointer, strict_number,
    source_evidence, build_publication_plan
};
use operations_hard_link_tree_exact::{
    publication_payloads, hash_output_document, hash_bytes, valid_tile_component,
    required_json_str, hard_link_tree_exact, replace_json_hardlink_safe, blocker_sort_key,
    blocked_tile, canonical_string, unique_nonce
};
#[cfg(not(windows))]
use operations_hard_link_tree_exact::metadata_is_reparse_point;
use validation::{validate_enrichment_source_documents, reject_existing_output};
use input::{
    load_map_scene, find_exact_scene_script_objects, parse_tile_coordinates,
    resolve_effective_build_root, load_tile, parse_dong_tile_name
};
use state::runtime_ambience_contract;
use assets::{tile_session_path, path_from_forward_slashes, replace_manifest_blockers};
use codec::{exact_byte_payload, encode_gray8_png, publication_payload_role};
use localization::unity_translation_to_native;
use systems::refresh_publication_plan;
use entities::attach_gameplay_attributes;
use projects::SessionDirectory;
#[cfg(test)]
use terrain_exact_terrain_placement_audit::terrain_graph_closure_blockers_from_document;
#[cfg(test)]
use operations_enrich_native_terrains_batch::strict_color;
#[cfg(test)]
use operations_link_scene_instance::{component_sidecar_stem, register_case_folded_sidecar_stem, publication_instance_id};
