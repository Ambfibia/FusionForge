use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{Cursor, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use image::{DynamicImage, GrayImage, ImageBuffer, ImageFormat, Luma, RgbaImage};
use serde::Serialize;
use serde_json::{json, Value as JsonValue};

use super::{
    preview::{decode_texture_mips_exact, DecodedTextureMip},
    unity::{object_name, value_array, ObjectKey, Pointer, UnityEnvironment, UnityValue},
};

#[cfg(test)]
mod tests;

mod output;
mod terrain;
mod constants;
mod models;
mod collision;
mod assets;
mod containers;
mod types;
mod operations;
mod validation;
mod textures;
mod codec;

use output::{
    EXPORT_SCHEMA, atomic_publish_directory, export_optional_lightmap, export_detail_database, write_json_new, write_new, write_new_or_verify
};
use terrain::{
    ExportedTerrain, TerrainVertexShift
};
pub use terrain::{
    TerrainSelection,
    export_native_terrain_exact_in_caller_staging, export_native_terrain
};
use constants::{
    HEIGHT_NORMALIZATION_DENOMINATOR, NATIVE_UV_SHIFT_FORMULA, SCENE_HEIGHT_FORMULA,
    ATOMIC_RENAME_ATTEMPTS
};
use models::{NATIVE_VERTEX_FORMULA, NATIVE_VERTEX_SHIFT_FORMULA, exact_vertex_shifts};
use collision::NATIVE_CELL_TRIANGLE_ORDER;
use assets::{ExportManifest, fresh_staging_path};
use containers::{
    SourceBundle, unity_value_to_json, resolved_object_raw_blake3, required_object,
    canonicalize_unity_heights
};
use types::{WeightChannel, StagingDirectory};
use operations::{
    retry_transient_rename, required_nonnegative_usize, exact_byte_array,
    graph_closure_is_blocked, pointer_json, pointer_provenance, required_dimension, exact_i64,
    exact_u16_array, required_vec3, required_vec2, required_pointer,
    required_true_name, semantic_component, register_semantic_name, weight_channel,
    u16_le_bytes, hash_bytes
};
use validation::is_transient_windows_rename_error;
use textures::{
    write_texture_mip_chain, texture_import_contract, preload_texture_atlas_contract,
    ResolvedTexture, ExportedDetailTexture, DetailTextureNameProof, resolve_texture,
    register_unique_detail_texture_name, detail_texture_reference, canonicalize_rgba_flip_y
};
use codec::{encode_gray8_png, encode_gray16_png, encode_rgba8_png};
#[cfg(test)]
use terrain::splat_mode_with_provenance;
