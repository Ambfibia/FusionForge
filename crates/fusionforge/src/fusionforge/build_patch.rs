use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

use image::GenericImageView;
use serde_json::{json, Value as JsonValue};
use sha1::{Digest, Sha1};

use super::{
    binary::{BinaryReader, Endian},
    managed::{self, ManagedApplyOptions},
    modding::{apply_texture_import, ImportedTexture},
    unity::{object_name, Asset, UnityValue},
};

#[cfg(test)]
mod tests;

mod assets;
mod containers;
mod textures;
mod audio;
mod constants;
mod projects;
mod operations;
mod output;
mod types;
mod input;
mod materials;

use assets::{
    FONT_MANIFEST_FORMAT,
    is_unsupported_object_string_field_path, is_unsupported_object_string_path_parts,
    entry_path_parts, font_manifest_entries,
    required_path
};
pub use assets::{patch_asset_loader_downloads, patch_unity_asset_strings};
use containers::{OBJECT_STRING_KIND, is_unsupported_object_string_entry, set_object_field};
use textures::{
    texture_to_logical_alpha,
    logical_alpha_to_texture_data
};
pub use textures::patch_texture_pngs;
pub use audio::patch_audio_clips;
pub(crate) use audio::audio_duration_seconds;
use constants::RUSSIAN_CODES;
pub use projects::{
    patch_managed_strings, patch_resource_locator_character_bundles,
    patch_binary_reader_unicode_strings, patch_gui_fonts
};
use operations::{
    cp1251_aliases, font_summary, build_font_redirect_map, redirect_font_pointers,
    looks_like_gui_skin, assign_default_font_to_null_gui_styles, rect_pixels, set_rect_uv,
    make_character_rect, try_pack_glyphs, next_power_of_two, build_ttf_font_map,
    normalize_font_family, extract_ttf_family_name, option_value, has_flag, has_text,
    entry_i64
};
pub use output::export_managed_strings;
use output::write_status;
use types::{OggVorbisMetadata, FontSummary, GlyphBitmap};
use input::{parse_ogg_vorbis_metadata, read_json};
#[cfg(windows)]
use materials::render_glyphs;
#[cfg(not(windows))]
use materials::render_glyphs;
