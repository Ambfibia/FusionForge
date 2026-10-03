//! Publish the non-geometry behaviour of every native world tile.
//!
//! The static-world installer publishes what a tile *looks* like: meshes,
//! materials and mesh colliders. The source scenes also carry the behaviour of
//! those same nodes - scripted Infected-Zone elements, effect emitters,
//! billboards, visibility switches, animation players, trigger volumes and
//! rigid bodies. This installer converts the offline
//! `ffone.native-static-world-behaviours.v1` exports into one manifest-bound
//! native document per tile.
//!
//! The conversion is deliberately *semantic*, not a raw field dump: the runtime
//! never sees Unity pointers, script GUIDs or serialized field names. Each
//! record carries its exact source-derived world matrix, validated against the
//! published hierarchy whenever one exists, so a behaviour can be placed
//! without any dependency on conversion metadata.
//! Anything the converter cannot classify is recorded as an explicit blocker
//! and is never silently dropped.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::{PipelineError, Result, error::io_at};

#[cfg(test)]
mod tests;

mod constants;
mod assets;
mod output_install_world_behaviours;
mod output_convert_tile;
mod containers;
mod types;
mod animation;
mod operations;
mod terrain;
mod codec;
mod state;
mod input;
mod validation;

pub use constants::{
    WORLD_BEHAVIOUR_ROOT, WORLD_BEHAVIOUR_OWNERSHIP_SCHEMA, WORLD_BEHAVIOUR_DOCUMENT_SCHEMA
};
use constants::{HIERARCHY_SCHEMA, INSTALLER_ID};
pub use assets::WORLD_BEHAVIOUR_OWNERSHIP_PATH;
use assets::{
    HierarchyIndex,
    relative_hierarchy_path, load_registry
};
pub use output_install_world_behaviours::{
    WORLD_BEHAVIOUR_INSTALL_REPORT_SCHEMA, WorldBehaviourInstallOptions,
    WorldBehaviourInstallReport, install_world_behaviours
};
use output_install_world_behaviours::{
    BehaviourExport, ExportBehaviour
};
use output_convert_tile::{convert_tile, load_previous_install};
use containers::{WORLD_EFFECT_PREFAB_CLOSURE_SCHEMA, ExportEffectPrefabClosure};
pub use types::{WorldBehaviourOwnership, WorldBehaviourTileProof};
use types::{HierarchyNode, Totals};
use animation::{
    ExportAnimationClip, collect_animation_target_paths, build_animation_targets,
    animation_curve_path, native_animation_quaternion, read_packed_animation_bits,
    read_packed_animation_floats, unpack_legacy_animation_quaternion
};
use operations::{
    superseded_script, optional_json_array, required_curve_keys, finite_json, json_vec3,
    json_quaternion, push_strict_time, usize_json, blocker, matrix_json, vector_json,
    number_json, bool_value, hierarchy_vector, hierarchy_matrix, models_in_node_subtrees,
    matches_owned_bytes, source_set_blake3, canonical_directory, pretty_json, hash_bytes,
    unique_token, invalid
};
use terrain::is_builtin_terrain_component;
use codec::{
    decode_world_animation_clip, animation_byte_payload
};
use state::infinity_mode;
use input::{load_hierarchy, parse_json};
use validation::{json_error, invalid_error};
