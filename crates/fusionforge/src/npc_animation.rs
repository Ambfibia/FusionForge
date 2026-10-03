//! CPU animation sampling for the native NPC preview.
//!
//! The current bundle preview JSON only exposes `AnimationClip` metadata. This
//! module deliberately does not know how that JSON was produced: once a
//! preview contains the optional `animationData`, `skeleton`, and mesh `skin`
//! fields documented on [`NpcAnimationSampler`], it can be prepared once and
//! sampled every frame without reparsing the (potentially large) preview JSON.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;

#[cfg(test)]
mod tests;

mod types;
mod constants;
mod animation;
mod models;
mod assets;
mod input;
mod operations;

use types::{Vec3, Quat, Matrix4, TransformTrack, Vec3Key, QuatKey, Joint, CurveKind, ParentRef};
pub use types::SampledJointTransform;
use constants::EPSILON;
pub use animation::{
    NpcAnimationError, SampledNpcPose, NpcAnimationSampler, sample_preview_animation
};
use animation::{AnimationClip, parse_clip};
pub use models::SampledMeshPositions;
use models::{PreviewMesh, MeshSkin, parse_mesh_skin};
use assets::{JointLookup, normalized_path, parent_path, path_and_ancestors};
use input::{
    parse_clips, parse_split_vec3_curves, parse_split_quat_curves, parse_vec3_keys,
    parse_quat_keys, parse_parent_ref, resolve_joint_parents, parse_meshes,
    parse_usize4_stream, parse_f64x4_stream, parse_matrix_stream, find_track, parse_vec3,
    parse_quat, parse_matrix4
};
use operations::{
    deduplicate_vec3_keys, deduplicate_quat_keys, add_missing_track_joints, skin_positions,
    global_matrices, normalized_sample_time, sample_vec3, sample_quat,
    first_array, matrix_from_flat, finite, identity_matrix, compose_matrix, mat_mul, invert_matrix, normalize_quat
};
