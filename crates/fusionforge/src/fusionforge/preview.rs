use std::collections::{BTreeSet, HashMap};

use base64::{engine::general_purpose::STANDARD, Engine};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder, RgbaImage};
use serde_json::{json, Value as JsonValue};

pub use super::coordinates::Vec3;
use super::coordinates::{
    native_coordinate_contract_json, unity_to_native_quaternion, unity_to_native_scale,
    unity_to_native_vec3,
};
use super::unity::{
    object_name, pair_name_value, pointer_summary, read_packed_bits, read_packed_floats,
    value_array, vector, vector2, ObjectKey, Pointer, UnityEnvironment, UnityValue,
};

#[cfg(test)]
mod compressed_mesh_tests;

#[cfg(test)]
mod coordinate_contract_tests;

#[cfg(test)]
mod material_render_state_tests;

#[cfg(test)]
mod texture_mip_tests;

mod textures;
mod terrain;
mod types;
mod operations;
mod codec;
mod containers;
mod materials;
mod state;
mod systems;
mod assets;
mod models;
mod input;
mod collision;

use textures::{
    MATERIAL_TEXTURE_PREVIEW_MAX_SIZE, texture_mip_encoded_len, texture_image_data,
    rgb565_to_rgba, dxt_color_block, texture_slot_priority, is_alpha_texture_slot
};
pub use textures::{DecodedTexture, DecodedTextureMip, texture_to_data_url};
use terrain::TerrainSplatLayerPreview;
pub use terrain::terrain_to_document;
pub use types::Matrix4;
pub use operations::{
    identity_matrix, mat_mul, transform_point, transform_normal, converted_position,
    converted_scale, converted_quaternion, quaternion_json, compose_matrix, color_to_hex,
    image_to_data_url, average_image_color, pointer_id, summarize_transform
};
use operations::{
    round, vec_json, vec_sub, vec_cross, vec_dot, color_alpha,
    repair_transparent_rgb, put_pixel, lerp_color, saved_float, image_has_alpha,
    image_has_partial_alpha, sampled_mask_alpha, has_black_key_coverage,
    pointer_ref_loaded_for_preview, sample_alpha, sample_layer,
    hex_color_to_rgb, primary_compressed_uv_channel, compressed_submesh_start,
    strip_to_triangles
};
pub use codec::{decode_texture, decode_texture_mips_exact};
use containers::unity_value_text;
pub use containers::object_key_from_pointer;
pub use materials::material_preview;
use state::alpha_mode_hint;
use systems::{apply_alpha_mask, should_apply_black_key_alpha, apply_black_key_alpha};
use assets::normalized_preview_asset_ref_name;
pub use models::{
    MeshData, mesh_to_obj, mesh_to_preview, mesh_to_exact_source, extract_mesh,
    mesh_vertex_count
};
use input::read_normals;
pub use collision::summarize_collider;
#[cfg(test)]
pub use codec::decode_texture_mips;
#[cfg(test)]
use codec::{decode_rgba4444, decode_argb4444};
