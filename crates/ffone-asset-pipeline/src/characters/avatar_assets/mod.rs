//! Fail-closed native asset resolution for protocol-0104 player avatars.
//! This offline audit module is compiled with the asset pipeline, never with
//! the Bevy client shell.
//!
//! This module does not load Unity or Gamebryo content. It joins the native
//! TableData JSON, the native project manifest, and the offline semantic
//! ownership report. A source route is never promoted to a runnable GLB merely
//! because a flat mesh with a similar name exists.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest,
};
use ffone_protocol::{
    CHARACTER_EQUIP_SLOT_COUNT_0104, CharacterStyle0104, EquippedItem0104, ItemBase0104,
    PcStyle0104,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::normalize_route;

#[cfg(test)]
mod tests;

mod constants;
mod assets_avatar_asset_catalog_open;
mod assets_avatar_asset_catalog_resolve_clothing;
mod validation;
mod types;
mod state;
mod models;
mod operations;
mod input;
mod textures;

pub use constants::AVATAR_RESOLUTION_SCHEMA;
use constants::{TABLE_SET_SCHEMA, SEMANTIC_PLAN_SCHEMA, PROTOCOL_0104};
pub use assets_avatar_asset_catalog_open::{
    AvatarCatalogOptions, NativeAvatarAssetKind, NativeAvatarAssetResolution,
    AvatarCatalogProvenance, AvatarAssetCatalog
};
use assets_avatar_asset_catalog_resolve_clothing::{
    ContentIndex, ContentAsset, index_manifest, project_relative_path,
    read_verified_manifest_file, resolve_evidence_path, make_route
};
pub use validation::AvatarCatalogError;
use validation::{require_input_schema, require_avatar_tables};
pub use types::{
    AvatarGender, AvatarPartKind, AvatarPartParticipation, AvatarPartDisposition,
    VerifiedNativeCandidate, AvatarPartResolution, AvatarResolutionReport
};
use types::{
    HatPolicy, ResolvedItemRow, TableSet, SemanticPlan, PlanInput, PlanEntity, PlanDependency, PlanCandidate
};
pub use state::{
    AvatarStyleSelection, AvatarItemSelection, NativeResolutionStatus, AvatarTableSelection
};
use models::{PlanModel, semantic_model_path};
use operations::{
    canonical_directory, canonical_file, unique_input, verify_evidence,
    int_field, gender_string, source_stem, semantic_category,
    disposition_for_assets, suppress_part, block_part, empty_style_part,
    unresolved_style_part, missing_sources
};
use input::{read_file, parse_json};
use textures::semantic_texture_path;
#[cfg(test)]
use assets_avatar_asset_catalog_open::CONTENT_INDEX_SCHEMA;
