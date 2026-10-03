//! Legacy Unity NPC animation extraction for the native model preview.
//!
//! FusionFall character bundles use Unity's pre-Mecanim `AnimationClip`
//! representation.  Most clips combine ordinary vector curves with
//! `PackedQuatVector` rotation curves, while their meshes keep skin weights in
//! `m_CompressedMesh`.  This module translates those structures into the small
//! JSON schema consumed by `npc_animation::NpcAnimationSampler`.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value as JsonValue};

use crate::fusionforge;

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

mod containers;
mod types;
mod animation_validate_exact_animation_source;
mod animation_collect_clip_keys;
mod audio;
mod validation_validate_curve_recoveries;
mod validation_validate_time_recoveries;
mod commands;
mod operations_keep_decoded_curve;
mod operations_curve_recovery_plans_to_json;
mod input;
mod models;
mod assets;
mod codec_decode_animation_clip_with_curve_recoveries;
mod codec_decode_mesh_skin;

use containers::{
    ObjectKey, parse_unity_matrix, read_object,
    object_type, unity_vec3, unity_quat
};
pub(crate) use containers::{
    UnityLegacyNpcPreview, build_unity_legacy_npc_preview,
    build_unity_legacy_npc_preview_from_selection
};
use types::{
    Vec3, Quat, Matrix4, DecodedCurveTracks, DecodedCurveTrack, DecodedCurveSamples,
    TransformNode, SkinnedRenderer, CurveTimeReference, ConstantCurveReference,
    CurveRecoveryPlan, CharacterRootCandidate, Vec3CurveKind
};
use animation_validate_exact_animation_source::{
    DecodedClip, RawAnimationClip, animation_tracks, resolve_exact_animation_path,
    rest_global_matrix_in_skeleton_space
};
pub(crate) use animation_validate_exact_animation_source::{
    validate_exact_animation_source, mark_unbound_animation_bindings
};
use animation_collect_clip_keys::{
    collect_clip_keys, clip_sample_rate, build_skeleton_preview, declared_clip_duration
};
pub(crate) use audio::classify_metadata_only_sound_event_pointers;
use validation_validate_curve_recoveries::{
    validate_exact_event_object_parameter, validate_exact_event_float_parameter,
    validate_curve_recoveries
};
use validation_validate_time_recoveries::{
    validate_time_recoveries, validate_sampleable_tracks,
    validate_sampleable_float_tracks
};
use commands::{
    is_exact_larry_unused_end_event_pointer, is_exact_larry_walk_end_event_with_stale_float,
    event_field, optional_event_string, optional_event_i64
};
use operations_keep_decoded_curve::{
    exact_curve_count, exact_required_duration, exact_optional_duration,
    exact_duration_matches, durations_equal, raw_trs_source, curve_recovery_rejected_count,
    curve_recovery_rejected_keyed_duration, raw_target_binding_count, exact_time_array,
    exact_nullable_f64, track_payloads_equal_excluding_time, preview_local_matrix,
    character_root, compact_character_name,
    select_preferred_character_roots, meshes_for_character_roots,
    recover_non_strict_curve_times, recover_conflicting_constant_curves, plain_trs_kind,
    constant_key_has_zero_tangents,
    optional_f64_bits_equal, raw_plain_curve_parts, strictly_increasing_finite_times, keep_decoded_curve, duplicate_trs_binding_source_order,
    decoded_curve_track_source_order
};
use operations_curve_recovery_plans_to_json::{
    canonicalize_curve_keys, unpack_legacy_compressed_quaternion, normalize_weights,
    reflect_matrix_x, compose_matrix, mat_mul, common_ancestor, ancestor_chain,
    ancestor_in_set, depth_within, resolved_key, curve_count, curve_tracks_to_json,
    curve_recovery_plans_to_json,
    normalize_quat
};
use input::{
    find_source_track, collect_transforms, collect_character_root_candidates,
    collect_skinned_renderers
};
use models::{build_model_hierarchy, collect_mesh_bindings, collect_rigid_mesh_skins};
use assets::{
    full_transform_path, preferred_path_parts, build_curve_time_recovery_catalog,
    build_constant_curve_recovery_catalog, plain_curve_path, normalized_path
};
use codec_decode_animation_clip_with_curve_recoveries::{
    decode_plain_trs_curve, constant_curve_payload,
    decode_animation_clip_with_curve_recoveries
};
use codec_decode_mesh_skin::{
    decode_event_object_parameter, decode_compressed_quaternion_curves, decode_mesh_skin
};
#[cfg(test)]
use operations_keep_decoded_curve::character_root_preference_score;
#[cfg(test)]
use codec_decode_animation_clip_with_curve_recoveries::{decode_animation_clip, decode_plain_curve, decode_animation_events, decode_animation_events_for_clip};
#[cfg(test)]
use codec_decode_mesh_skin::decode_compressed_skin_streams;
