//! Native shared player-rig publication from an already extracted object dump.
//!
//! This module never opens a Unity bundle. It consumes the immutable JSON object
//! dump produced by the offline extraction stage, publishes one semantic GLB per
//! gender, and proves that every default creator part skin resolves to the exact
//! legacy `ActorWearIndexTable.transformIndicesM/F` actor-bone palette.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use ffone_skinned_model::{
    AnimationChannel, AnimationClip, AnimationEvent, AnimationEventObjectParameterProvenance,
    AnimationMetadata, EmptyTrsSourceEncoding, Interpolation,
    MissingEventObjectParameterInterpretation, ModelNode, NativeModel, TrackValues, encode_glb,
    exact_native_coordinate_contract,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};

use crate::{
    ASSET_MANIFEST_FILE, AvatarItemCategory, CHARACTER_CREATION_APPEARANCE_PATH,
    CHARACTER_CREATION_APPEARANCE_SCHEMA, CHARACTER_CREATION_AVATAR_ITEMS_PATH,
    CHARACTER_CREATION_AVATAR_ITEMS_SCHEMA, CharacterAppearanceCategory,
    CharacterCreationAppearance, CharacterCreationAvatarItems, CharacterGender,
    LogicalModelPublishOptions, NativeLookupStatus, PROJECT_ASSET_SCHEMA, PipelineError,
    PlayerEquipmentCatalog, ProjectAssetFile, ProjectAssetKind, ProjectAssetManifest, Result,
    error::io_at,
    logical_model_publish::{prepare_logical_model, verify_or_write_prepared_logical_model},
};

#[cfg(test)]
mod tests;

mod animation_publish_player_rig_animations;
mod animation_commit_player_rig_animation_outputs_with_gat;
mod containers;
mod assets;
mod state;
mod types;
mod constants;
mod models;
mod output;
mod input;
mod operations;
mod codec;
mod validation;
mod localization;
mod retarget;
mod launcher;
pub use launcher::encode_player_rig_clip_additions;
pub use retarget::rebuild_male_emote_payloads;

pub use animation_publish_player_rig_animations::{
    PLAYER_SHARED_RIG_SCHEMA, PLAYER_SHARED_RIG_REPORT_SCHEMA,
    PLAYER_SHARED_RIG_CONTRACT_PATH, MALE_SHARED_SKELETON_GLB_PATH,
    FEMALE_SHARED_SKELETON_GLB_PATH, PlayerSharedRigPublishOptions,
    PlayerRigAnimationPublishOptions, PlayerRigSupplementalCreatorSource, PlayerRigGender,
    PlayerRigSourceIdentity, PlayerRigNode, PlayerRigClipContract, PlayerRigSkinRemap,
    PlayerRigPartContract, PlayerRigCreatorChoiceContract, PlayerGenderRigContract,
    PlayerSharedRigContract, PlayerSharedRigPublishReport, export_player_rig_clip_additions,
    publish_player_rig_animations
};
use animation_publish_player_rig_animations::{
    CustomRuntimeClipSpec, custom_runtime_clip, runtime_clip_loops, runtime_clip_status,
    RigOutput, PLAYER_RIG_ANIMATION_STAGE_SUFFIX,
    PLAYER_RIG_ANIMATION_BACKUP_SUFFIX, player_rig_animation_sidecar
};
use animation_commit_player_rig_animation_outputs_with_gat::{
    commit_player_rig_animation_outputs_with_gate, validate_animation_component,
    rebase_legacy_additive_clip, rig_error, rig_message
};
use containers::{DumpObject, game_object_transform};
use assets::{
    DumpCatalog, actor_skin_combiner_clothes_index, extract_route_remaps, component_path_ids,
    curve_path, pointer_path_id
};
use state::{MALE_RUNTIME_CLIPS, FEMALE_RUNTIME_CLIPS, CUSTOM_RUNTIME_CLIPS};
use types::{GenderSpec, Vec3Track, LegacySkinRemap, BitReader};
use constants::{MALE, FEMALE};
use models::{
    CreatorModelCandidate, audit_glb_remaps, parse_glb_json
};
pub use output::publish_player_shared_rigs;
use input::{resolve_creator_models, collect_transforms, read_packed_bits, read_packed_floats};
use operations::{
    extract_actor_nodes, quat_conjugate, quat_multiply, unique_target,
    unpack_legacy_compressed_quaternion, native_rotation,
    native_quat_tangent, vec3, quat, finite, usize_number, array_field, optional_array
};
use codec::{
    decode_retargeted_custom_clip, decode_clip,
    byte_payload
};
use validation::validate_published_animations;
use localization::native_translation;
#[cfg(test)]
use animation_publish_player_rig_animations::{prepare_player_rig_animation_manifest, commit_player_rig_animation_outputs};
#[cfg(test)]
use codec::decode_compressed_rotation_curves;
