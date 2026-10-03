//! Entity-first, plan-only organization of already extracted native assets.
//!
//! Only JSON evidence is read. Unity bundles and production assets are never opened or mutated.

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use crate::{PipelineError, Result, error::io_at};

use crate::shared::normalize_route;

#[cfg(test)]
mod tests;

mod assets;
mod constants;
mod models;
mod types;
mod commands;
mod materials;
mod containers;
mod operations;
mod input;
mod validation;
mod textures;
mod output;

pub use assets::{
    SEMANTIC_ASSET_ORGANIZATION_SCHEMA, SemanticAssetOrganizerOptions,
    SemanticAssetOrganizationReport, NativeAssetReference, RouteScaleUsage, SharedNativeAsset,
    UnresolvedRoute
};
use assets::{
    ASSET_MANIFEST_SCHEMA, CONTENT_INDEX_SCHEMA, PlanRouteBlockers, NativeIndex,
    build_native_index, FieldRoute, route_from_field, make_route, asset_stem, route_directory,
    positive_index, display_path, absolute_path, same_absolute_path
};
use constants::{
    COOK_REPORT_SCHEMA, TABLE_SET_SCHEMA, NPC_SETUP_AUTHORITY, NANO_SETUP_AUTHORITY,
    PLAYER_SETUP_AUTHORITY, EQUIPMENT_ATTACHMENT_AUTHORITY
};
use models::{LOGICAL_MODEL_PLAN_SCHEMA, face_head_model_aliases};
pub use models::{ModelProposal, ModelFeatureClosure, ModelFeatureClosureStatus};
pub use types::{
    InputEvidence, OrganizerCounts, SemanticCategory, EntityProposal, TableOwner,
    SpawnRootPolicy, SpawnRootPolicyKind, DependencyProposal, DependencyKind,
    DependencyResolution, NativeOwnership, BlockerCode, OrganizationBlocker
};
use types::{EvidenceDocument, ReadyRoot, Builder};
pub use commands::RootTrsAction;
pub use materials::{ModelMaterialPlan, ModelMaterialStatus};
pub use containers::SerializedObjectIdentity;
pub use operations::plan_semantic_assets;
use operations::{
    alias_base, clean_value,
    portable_true_name, value_array, int_field, u64_pointer,
    string_field, normalize_slashes
};
use input::{read_evidence, read_logical_plan};
use validation::{require_bool, semantic_error};
use textures::face_head_texture_aliases;
use output::write_atomic_create_new;
#[cfg(test)]
use operations::{taxonomy, add_npc_entities, add_equipment_entities};
