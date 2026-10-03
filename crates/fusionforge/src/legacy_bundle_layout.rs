//! Graph-based repacking for the original mixed FusionFall resource bundles.
//!
//! This module deliberately works from the already-patched build directory.  Audio,
//! texture and string replacements therefore become the source of truth and cannot be
//! lost by falling back to the pristine source build.  It is kept separate from the
//! small `bundle_layout` character-pack migration so the caller can stage and validate
//! the complete legacy layout before removing any input bundle.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha1::{Digest, Sha1};
use sha2::Sha256;

use crate::fusionforge::{
    object_name, pair_name_value, value_array, Asset, AssetRef, ObjectInfo, Pointer, TypeMetadata,
    TypeTree, UnityValue,
};
use crate::legacy_semantic_index::{OwnerHint as SemanticOwnerHint, SemanticFamily, SemanticIndex};

#[cfg(test)]
mod tests;

mod types;
mod containers;
mod constants;
mod assets;
mod audio;
mod validation;
mod state;
mod operations_classify_root;
mod operations_pack_dependency_units;
mod operations_build_plan;
mod operations_build_part;
mod operations_try_restore_layout_cache;
mod systems;
mod codec;
mod input;
mod output;

use types::{
    NodeKey, SourcePointerKey, LegacyConfig, Family, Node, Root, Part, OutputLocation, Plan,
    CompactOwner, CompactUsage, PackingUnit, PackedUnitsPart, LayoutCacheFile,
    LayoutCacheReceipt
};
pub(crate) use types::LegacyLayoutPlan;
use containers::{
    DanglingObjectPointer, SourceBundle, dong_tile_from_bundle, COMPAT_BUNDLE_NAMES, strict_read_object, source_bundle_name,
    retained_bundle_paths
};
pub(crate) use containers::{LegacyBundleDependency, LegacyRoutedBundle};
use constants::{
    DEFAULT_MAX_PART_BYTES, PART_METADATA_RESERVE_BYTES, EXACT_LEAF_TYPES, USE_CORE,
    USE_WORLD, USE_NPC, USE_NANO, USE_PLAYER, USE_ITEMS, USE_ICONS, USE_HNPC,
    LAYOUT_CACHE_FORMAT, LAYOUT_CACHE_SEQUENCE, LAYOUT_CACHE_STALE_SECONDS
};
use assets::{
    SourceAsset, AssetBundleTemplate, normalized_path,
    source_caching_manifest_sections, path_id_candidates, asset_has_path,
    source_asset_for_ref, legacy_route_rank, conservative_asset_ref_bytes, ReadyUnitIndex,
    source_name_index, asset_ref_key, extract_single_staged_asset, layout_report_path
};
use audio::{
    AudioObjectIdentity, audio_object_identity, USE_TUTORIAL_AUDIO, USE_UI_AUDIO,
    USE_NPC_VOICE, patched_audio_nodes
};
pub(crate) use audio::PatchedAudioIdentity;
pub(crate) use validation::LegacyRouteConflict;
use validation::{
    reject_oversized_atomic_units, validate_inner_serialized_size,
    validate_physical_bundle_size, validate_staged_outputs, validate_retained_consumers
};
use state::{
    Inventory, inventory, runtime_sections_for_family,
    normalize_root_runtime_classes
};
use operations_classify_root::{
    json_string, config, sha1_hex, hash_len, semantic_value_sha1, internal_ref_name,
    type_tree_key, tree_signature, pair_value_mut, set_i64, set_string, preload_range,
    zero_pointers,
    classify_root_with_semantics, preserved_external_ref, proven_missing_source_target,
    referenced_unreadable_errors, canonical_key, merge_owner, merge_usage, logical_owner_ids, root_compact_owner, propagate_compact_usage, family_label
};
use operations_pack_dependency_units::{
    duplicate_analysis, root_scope, merge_root_preloads,
    output_part_name, output_internal_name, strongly_connected_components, align4_estimate,
    synthetic_root_estimated_bytes, literal_dong_source_sections, contract_exact_dong_units,
    propagate_dependency_sections, lifecycle_safe_family, pack_dependency_units
};
use operations_build_plan::{
    build_plan, dependency_parts, root_owner_part, dependency_order
};
use operations_build_part::{
    build_part, retained_opaque_file,
    retained_repack_roots,
    build_retained_consumers
};
use operations_try_restore_layout_cache::{
    output_reference_name, custom_root_sections, disabled_plan,
    remove_layout_cache_dir, store_layout_cache
};
pub(crate) use operations_try_restore_layout_cache::{
    result_cache_key, restore_result_cache
};
use systems::{update_semantic_hash, update_sha256_field};
use codec::audio_payload;
use input::{
    collect_pointers, discover_resource_bundles, resolve_source_pointer,
    resolve_compact_family, resolve_staged_pointer
};
use output::{transactional_publish, LayoutCacheWriteGuard, publish_layout_report};
#[cfg(test)]
use containers::{harmless_object_size_mismatch, object_roundtrip_is_lossless, invalid_serialized_object_name};
#[cfg(test)]
use state::runtime_sections_for_root;
#[cfg(test)]
use operations_classify_root::classify_root;
#[cfg(test)]
use operations_build_plan::dependency_cycle_components;
#[cfg(test)]
use operations_build_part::{clear_recorded_dangling_pointers, assign_output_type, pointer_value};
#[cfg(test)]
use operations_try_restore_layout_cache::{sha256_file, safe_layout_cache_name, cached_plan_file_names};
