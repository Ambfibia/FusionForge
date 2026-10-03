//! Lossless Material/Texture2D projection used exclusively by the native
//! `export-logical-model-source` command.
//!
//! This deliberately does not share the visual-preview material pipeline:
//! previewing is allowed to pick a representative map, resize it and repair
//! transparent RGB, while a logical-model source export must retain every
//! serialized slot and every source pixel.

use std::collections::{BTreeMap, BTreeSet};

use base64::{engine::general_purpose::STANDARD, Engine};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder, RgbaImage};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};

use crate::fusionforge::{
    decode_texture, object_name, pair_name_value, value_array, Asset, ObjectInfo, ObjectKey,
    Pointer, UnityEnvironment, UnityValue,
};

macro_rules! for_each_selected_object {
    ($env:expr, $selected:expr, $asset_index:ident, $asset:ident, $info:ident, $body:block) => {{
        if let Some(selected_objects) = $selected {
            for &($asset_index, path_id) in selected_objects {
                let Some($asset) = $env.assets.get($asset_index) else {
                    continue;
                };
                let Some($info) = $asset.objects.get(&path_id) else {
                    continue;
                };
                $body
            }
        } else {
            for ($asset_index, $asset) in $env.assets.iter().enumerate() {
                for $info in $asset.objects.values() {
                    $body
                }
            }
        }
    }};
}

#[cfg(test)]
mod tests;

mod materials;
mod models;
mod textures;
mod operations;
mod codec;
mod containers;

pub(crate) use materials::ExactMaterialExport;
use materials::{
    exact_material,
    material_value_id
};
pub(crate) use models::export_exact_mesh_materials_from_selection;
use textures::{
    exact_movie_texture, mip_level_byte_length
};
pub(crate) use textures::exact_texture;
use operations::{
    exact_named_pairs, resolved_pointer_key, strict_non_null_pointer, pointer_json,
    required_u32, color_hex, sha256_hex
};
use codec::{decode_exact_mip_level, exact_png_payload};
use containers::{
    unity_text, unity_bytes, unity_to_lossless_json, ensure_object_type, source_object_json, source_object_json_from_info, object_id
};
#[cfg(test)]
use materials::{shader_render_state_evidence, shader_declared_name};
#[cfg(test)]
use textures::{exact_mip_layout, exact_texture_setting, derive_mip_count, exact_png_bytes};
#[cfg(test)]
use containers::exact_object_type_name;
