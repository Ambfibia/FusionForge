//! Strict `ffone.logical-model-source.v1` to native GLB publication.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use ffone_skinned_model::{
    AnimationBindingPointer, AnimationChannel, AnimationClip, AnimationCurveRecovery,
    AnimationCurveRecoveryCanonicalTrack, AnimationCurveRecoveryProof,
    AnimationCurveRecoveryReason, AnimationCurveRecoveryReference, AnimationCurveRecoverySource,
    AnimationCurveRecoveryTrack, AnimationEvent, AnimationEventFloatParameterProvenance,
    AnimationEventObjectParameterProvenance, AnimationMetadata, AnimationTimeRecovery,
    AnimationTimeRecoveryProof, AnimationTimeRecoveryReason, AnimationTimeRecoveryReference,
    DuplicateAnimationKey, DuplicateKeyRelation, DuplicateTrsBinding, DuplicateTrsKeys,
    DuplicateTrsRelation, DynamicTextureBinding, DynamicTexturePointer, EmptyTrsBinding,
    EmptyTrsBindingKind, EmptyTrsSourceEncoding, ExactQuaternionKey, ExactQuaternionKeyPayload,
    ExactTrsKeyPayload, ExactVec3Key, ExactVec3KeyPayload, FloatCurve, Interpolation,
    MaterialBlendFactor, MaterialColorProperty, MaterialFloatProperty, MaterialTextureBinding,
    MaterialTextureSamplerBinding, ModelMesh, ModelNode, ModelPrimitive, ModelSkin,
    NativeCoordinateContract, NativeMaterial, NativeModel, NativeSampler, NativeTexture,
    NativeTextureMipLevel, PublishedMipPolicy, PublishedPixelTransform, SamplerMagFilter,
    SamplerMinFilter, SamplerWrapMode, SemanticRoundtripProof, SerializedCurveOverwriteProof,
    SerializedCurveOverwriteRule, SerializedEventObjectParameterInterpretation, TextureColorSpace,
    TextureMipProvenance, TextureSourceMipLayout, TrackValues, encode_glb,
    exact_native_coordinate_contract, minimal_windows_glb_filename, minimal_windows_png_filename,
    model_relative_path, prove_semantic_roundtrip, triangle_winding_opposes_normals, validate,
    windows_png_filename_preserving_legacy_extension,
};
use glam::{Mat4, Quat, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::io_at;
use crate::{
    MODEL_PUBLISH_SCHEMA, ModelFeatureCounts, ModelPublishContract, PipelineError, Result,
};

#[path = "../../native_texture_reuse.rs"]
mod native_texture_reuse;
pub use native_texture_reuse::index_native_textures;

use crate::shared::texture_format_name as exact_texture_format_name;

#[cfg(test)]
mod tests;

mod models;
mod textures;
mod constants;
mod types;
mod validation_validate_source_curve_recoveries;
mod validation_coordinate_audit;
mod animation;
mod materials;
mod codec;
mod containers;
mod commands;
mod state;
mod operations_build_textures;
mod operations_build_animations;
mod operations_source_feature_counts;
mod output;
mod assets;
mod input;

pub use models::{
    LOGICAL_MODEL_SOURCE_SCHEMA, LOGICAL_MODEL_PUBLISH_REPORT_SCHEMA,
    LogicalModelPublishOptions, LogicalModelPublishReport, convert_logical_model_bytes,
    prepare_direct_model, publish_logical_model
};
use models::{
    SourceExactMeshSelectionProof, SourceMesh, ConvertedModel, model_global_transforms, model_node_paths
};
pub(crate) use models::{
    prepare_logical_model, write_prepared_logical_model,
    verify_or_write_prepared_logical_model
};
#[cfg(test)]
pub(crate) use models::logical_model_source_test_fixture;
pub use textures::{
    ReviewedTextureRebindReport,
    PublishedTextureReport, PublishedTextureMipLevelReport
};
use textures::{
    SourceTextureEnvironment, SourceTexture, TexturePublication,
    apply_reviewed_texture_rebinds, validate_dynamic_texture_source,
    native_dynamic_texture_binding, validate_texture_source,
    texture_color_space
};
pub use constants::SKINNING_BASIS_PARITY_TOLERANCE;
pub use types::{SourceGeometryFilterReport, CoordinateBounds};
use types::{
    SourceDocument, SourceNode,
    SourceJoint, SourceGroup, SourceBinding, SourceSkin, SourceSavedProperties, SourceVec2, SourcePointer,
    SourceSampler, SourceNullableDuration,
    SourceCurveCounts, SourceFloatTrack, SourceEmptyTrsBinding,
    SourceVec3Track, SourceQuatTrack, SourceVec3Key, SourceQuatKey, SourceDuplicateTrsBinding,
    SourceDuplicateTrsKeys, PreparedPublicationFile, BuiltTextures, SourceRecoveryTrackRef,
    TrackKind, SourceCanonicalTrack, Vec3TrackValues, MatrixIdentityDeviation,
    BoundsAccumulator
};
pub use validation_validate_source_curve_recoveries::CoordinateAuditReport;
use validation_validate_source_curve_recoveries::{
    validate_renderer_slots, validate_source_object, validate_pointer_identity,
    validate_source_header, validate_hierarchy_path, validate_binding_root,
    validate_rigid_binding, validate_source_event_object_parameter,
    validate_source_curve_recoveries, validate_source_recovery_times,
    validate_source_recovery_interpolation_vec3,
    validate_source_recovery_interpolation_quaternion,
    validate_finite_source_values, validate_source_identity, validate_source_track_keys,
    validate_vec3_key_provenance, validate_quaternion_key_provenance
};
use validation_coordinate_audit::{
    validate_key_identity_coverage, validate_duplicate_trs_source, invalid_error
};
pub(crate) use validation_coordinate_audit::coordinate_audit;
pub use animation::CurrentPoseBindIdentityDeviation;
use animation::{
    SourceSkeleton, SourceAnimation,
    SourceAnimationCurveRecoveryCanonicalTrack, SourceAnimationCurveRecoveryTrack, SourceDuplicateAnimationKey, validate_skeleton,
    legacy_npc_clip_is_additive, reconcile_animation_curve_counts,
    validate_animation_duration_provenance, convert_duplicate_animation_keys,
    animation_node_path, validate_unbound_animation_node_path
};
pub use materials::{MaterialPublishReport, PublishedMaterialReport};
use materials::{
    SourceMaterial, SourceRendererMaterialBinding, ResolvedMaterialSlot, legacy_material_compositor_rank,
    build_material_conversion, exact_renderer_material_binding, validate_material_source
};
use codec::{
    SourceDynamicTexturePayload, SourceTextureChainPayload, SourceTexturePayload,
    SourceExactTrsKeyPayload, SourceVec3KeyPayload, SourceQuatKeyPayload, decode_exact_png,
    decode_exact_png_payload, source_track_constant_payload,
    source_constant_curve_payload, source_animation_has_model_payload
};
use containers::SourceObjectReference;
use commands::{SourceEvent, is_exact_larry_unused_end_event};
use state::SourceVisualSelection;
use operations_build_textures::{
    reuse_identical_static_materials, skinned_renderer_output_node,
    assign_legacy_renderer_orders, select_source_visuals, build_textures, is_sha256, build_materials, sha256_hex,
    build_hierarchy, binding_component_key
};
use operations_build_animations::{
    build_skin, build_primitives, build_animations, rebase_legacy_npc_additive_clips, source_tracks, source_track_at, source_track_encoding,
    source_canonical_recovery_matches, source_keys_len, source_target_binding_count,
    source_curve_field
};
use operations_source_feature_counts::{
    same_optional_source_f64_bits, track_kind, raw_trs_source, same_optional_duration,
    same_duration, vec3_channel, quat_channel, exact_keys_last_time, tangent_modes, source_feature_counts, feature_counts,
    matrix_max_difference, matrix_identity_deviation, native_matrix, vec3_values, vec2_values,
    u64_count, u32_count, add_count, invalid
};
use output::{
    convert_source, convert_float_curve, convert_duplicate_keys,
    convert_curve_recovery_canonical, convert_curve_recovery_track, write_publication
};
use assets::{
    normalized_hierarchy_path_component, source_track_path, source_track_index,
    exact_binding_node_index, exact_node_path, suffix_node_path, normalized_route, slash_path,
    u32_index
};
use input::{parse_interpolation, resolve_global_transform};
#[cfg(test)]
use models::logical_model_uses_legacy_npc_additive_deltas;
#[cfg(test)]
use validation_coordinate_audit::skinning_basis_parity_error;
#[cfg(test)]
use materials::FUSION_EFFECT_SHADER;
#[cfg(test)]
use operations_build_textures::{legacy_renderer_sort_key, is_biped_display_name};
