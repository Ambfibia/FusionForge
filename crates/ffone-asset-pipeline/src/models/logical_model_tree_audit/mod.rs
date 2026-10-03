//! Strict, read-only audit of a staged native logical-model tree.
//!
//! This module deliberately audits the published GLB/PNG/report boundary instead
//! of trusting the in-memory publisher model.  It is therefore suitable as the
//! last gate before an atomic candidate tree replaces `assets/game`.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use ffone_skinned_model::{
    AnimationCurveRecovery, AnimationEvent, AnimationEventObjectParameterProvenance,
    AnimationTimeRecovery, DuplicateAnimationKey, DuplicateTrsBinding, DuplicateTrsKeys,
    EmptyTrsBinding, EmptyTrsBindingKind, EmptyTrsSourceEncoding, MaterialTextureBinding,
    NativeCoordinateContract, SEMANTIC_ROUNDTRIP_PROOF_SCHEMA, SemanticRoundtripProof,
    SerializedEventObjectParameterInterpretation, ShaderLabTextureDefaultProperty,
    exact_native_coordinate_contract, minimal_windows_glb_filename, minimal_windows_png_filename,
    semantic_digests_from_glb, windows_png_filename_preserving_legacy_extension,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    PipelineError, Result,
    error::io_at,
    logical_model_publish::{CoordinateAuditReport, SKINNING_BASIS_PARITY_TOLERANCE},
};

use crate::shared::texture_format_name as audited_texture_format_name;

use crate::shared::has_generated_identity;

#[cfg(test)]
mod tests;

mod models;
mod output;
mod types;
mod animation;
mod operations;
mod validation_audit_meshes;
mod validation_audit_materials_and_images;
mod validation_audit_time_recoveries;
mod validation_audit_publish_report;
mod textures;
mod input;
mod assets;

pub use models::{
    LOGICAL_MODEL_TREE_AUDIT_SCHEMA, LogicalModelFeatureCounts, LogicalModelTreeCounts,
    LogicalModelTreeViolation, LogicalModelFileCounts, LogicalModelFileAudit,
    LogicalModelTreeAuditReport, audit_logical_model_tree
};
use models::{
    parse_glb,
    hierarchy_paths_from_glb_nodes
};
use output::{PUBLISH_REPORT_SCHEMA, PUBLISH_CONTRACT_SCHEMA};
use types::{BufferViewInfo, AccessorInfo};
use animation::{
    AuditedAnimationChannel, audit_animation_metadata, audited_animation_channel,
    valid_current_pose_bind_deviation
};
use operations::{
    walk_files, is_sha256, recovery_sampler_times_match, trs_source_rank,
    same_optional_f64_bits, typed_key_provenance_is_complete, valid_coordinate_bounds,
    json_f64_array, accumulate_file_counts, aliased_field, feature_count_fields,
    attribute_accessor, safe_image_relative, report_sidecar, array_or_empty,
    accessor_component_count, component_size, json_usize, json_usize_default, relative_string,
    push_violation, to_u64
};
use validation_audit_meshes::{audit_binary_layout, audit_hierarchy, audit_skins, audit_meshes};
use validation_audit_materials_and_images::{audit_materials_and_images, audit_animations};
use validation_audit_time_recoveries::{
    audit_curve_recoveries,
    audit_time_recoveries, audit_channel_key_provenance
};
use validation_audit_publish_report::{
    audit_publish_report, validate_true_name,
    tree_error
};
use textures::{
    audit_exact_mip_pngs, audit_report_mip_pngs,
    png_dimensions
};
use input::{read_unsigned, read_f32, read_weight, read_u32, read_be_u32};
use assets::slash_path;
#[cfg(test)]
use models::LOGICAL_MODEL_SCHEMA;
