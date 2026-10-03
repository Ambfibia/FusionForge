use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};

use blake3::Hasher as Blake3Hasher;
#[cfg(test)]
use ffone_content::validate_pack;
use ffone_content::{ContentKind, ContentPackBuilder, Provenance, SourceFingerprint};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};

use super::{
    decode_texture, extract_bundle, extract_mesh, managed::repair_cp1251_mojibake, object_name,
    UnityEnvironment, UnityValue,
};

#[cfg(test)]
mod tests;

mod localization;
mod constants;
mod types;
mod containers;
mod assets;
mod state;
mod validation;
mod output;
mod operations_cook_pack;
mod operations_is_valid_ogg;
mod textures;
mod models;
mod audio;
mod input;
mod codec;

use localization::{DEFAULT_LOCALE, process_localization_overlay, validate_locale};
use constants::COOK_REPORT_SCHEMA;
use types::{LauncherFileInfo, CookReport, CoverageCounters, CookMapping, CookSourceDescriptor};
use containers::{LauncherBundleInfo, BundleCookReport, process_bundle};
use assets::{
    LauncherManifest, NativeAssetEntry, FontIndexEntry, EmittedAsset,
    resolve_launcher_manifest, declared_overlay_manifest, native_asset_identity,
    cook_report_path
};
use state::{
    InventoryEntry, InventoryReportEntry, CookState, build_inventory,
    build_inventory_fingerprint, verify_inventory_entry
};
use validation::{
    ObjectCookError, validate_source_relative, validate_source_filename,
    validate_pack_relative_path, reject_forbidden_json, validate_sha256, validate_uuid,
    require_manifest_format, require_entries, ErrorRoots
};
pub(crate) use output::export_native_pack_cli;
#[cfg(test)]
use operations_cook_pack::cook_ffone_pack;
use operations_cook_pack::{
    cook_pack, cook_font, config_relative,
    ensure_contained_regular_file, contains_legacy_text
};
use operations_is_valid_ogg::{
    looks_like_posix_absolute, semantic_safe, neutral_semantic_label, semantic_or,
    content_kind_name, font_extension, is_valid_ogg, stable_key, blake3_hex,
    sha256_hex, is_uuid, canonical_directory, required_string, json_bytes, pretty_json_bytes,
    cleanup_created
};
use textures::cook_texture;
use models::cook_mesh;
use audio::{cook_audio, process_audio_overlay};
use input::{resolve_overlay_file, read_regular_file, read_json_value, parse_json};
use codec::validate_native_payload;
#[cfg(test)]
use operations_is_valid_ogg::ogg_page_crc;
