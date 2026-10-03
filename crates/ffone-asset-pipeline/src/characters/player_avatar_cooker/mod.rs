//! Fail-closed native player-avatar cooking evidence.
//!
//! The legacy editor may produce an offline object dump and exact logical-model
//! source documents. This module never opens a Unity bundle and never accepts
//! old runtime formats as Bevy assets. It proves exact route targets, extracts
//! the shared player skeleton in native coordinates, and binds every skinned
//! renderer to the authoritative `ActorSkinCombiner.actorBones` palette.
//!
//! It deliberately does not emit a fake assembled avatar. Until all animation
//! curves and combined skins pass native GLB roundtrip audits, the output is a
//! readable skeleton source plus a machine blocker report.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File, OpenOptions},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

use ffone_skinned_model::{NativeCoordinateContract, exact_native_coordinate_contract};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};

use crate::{PipelineError, Result, error::io_at};

#[cfg(test)]
mod tests;

mod constants;
mod animation;
mod assets;
mod types;
mod state;
mod containers;
mod operations;
mod output;
mod localization;
mod validation;

pub use constants::PLAYER_AVATAR_COOK_REPORT_SCHEMA;
use constants::{TEST_SER_PROFILE, REPORT_OUTPUT, PARTS};
pub use animation::{
    PLAYER_SKELETON_SOURCE_SCHEMA, SkeletonOutputEvidence
};
use animation::{SKELETON_OUTPUT, extract_male_skeleton};
use assets::{ACTOR_ROUTE, DumpCatalog, component_path_ids, pointer_path_id};
pub use assets::RouteOwnership;
pub use types::{
    PlayerAvatarCookOptions, FileEvidence, RendererRemapEvidence, PlayerPartEvidence,
    TestSerEquipment, PlayerAvatarCookReport
};
use types::PartSpec;
pub use state::TestSerSelection;
use state::test_ser_selection;
use containers::{DumpObject, classified_unity_alias_warning};
pub use operations::cook_player_avatar;
use operations::{
    array_field, optional_array_len, usize_field,
    vec3_field, quat_field, native_rotation, player_message
};
use output::write_new_output;
use localization::native_translation;
use validation::{validate_true_name, player_error};
