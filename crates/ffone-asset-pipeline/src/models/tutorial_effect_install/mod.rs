//! Exact, fail-closed publication of the effects used by the original
//! FusionFall tutorial.
//!
//! The source accepted here is a lossless `fusionforge dump-object ... all`
//! JSON dump of `Effects.resourceFile` plus the extracted Unity asset that the
//! dump came from.  The installer never translates an unsupported Unity
//! particle component into an invented Bevy substitute.  It publishes the
//! complete serialized dependency closure, byte/hash provenance and the two
//! exact `BulletTable` rows so the runtime can reject unsupported rendering
//! explicitly.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, error::io_at,
};

use crate::shared::canonical_json;

use crate::shared::transaction_stamp as unique_token;

#[cfg(test)]
mod tests;

mod constants;
mod assets;
mod output;
mod state;
mod types;
mod containers;
mod validation;
mod operations;
mod input;
mod codec;

pub use constants::{
    TUTORIAL_EFFECT_ROOT, TUTORIAL_PROJECTILE_ROOT, TUTORIAL_EFFECT_CLOSURE_SCHEMA,
    TUTORIAL_BULLET_ROW_SCHEMA, RETROBUTION_TUTORIAL_BUILD_ID,
    RETROBUTION_TUTORIAL_EFFECT_IDS, RETROBUTION_FUSION_ACTOR_EFFECT_IDS,
    RETROBUTION_CHARACTER_ACTOR_EFFECT_IDS, RETROBUTION_NPC_WARP_EFFECT_IDS,
    RETROBUTION_WORLD_EP_EFFECT_IDS, RETROBUTION_NPC_GAME_ICON_EFFECT_IDS,
    RETROBUTION_WEAPON_EFFECT_IDS, RETROBUTION_TUTORIAL_BULLET_TYPES,
    RETROBUTION_TUTORIAL_PROJECTILE_EFFECT_IDS
};
use constants::{EFFECTS_DEPENDENCY_B4, EFFECTS_DEPENDENCY_BD5};
pub use assets::{
    TUTORIAL_EFFECT_CATALOG_PATH, TUTORIAL_PROJECTILE_CATALOG_PATH,
    TUTORIAL_EFFECT_CATALOG_SCHEMA, TUTORIAL_PROJECTILE_CATALOG_SCHEMA,
    RETROBUTION_TUTORIAL_SOURCE_ASSET_PROOFS, TutorialSourceAssetProof, TutorialEffectCatalog,
    TutorialEffectCatalogEntry, TutorialProjectileCatalog, TutorialBulletCatalogEntry
};
use assets::{
    BULLET_TABLE_ROUTE, PRIMARY_EFFECTS_ASSET, load_source_asset,
    referenced_asset_name, unique_route_root, effect_route, replace_manifest,
    verify_owned_catalog
};
pub use output::{
    TUTORIAL_EFFECT_INSTALL_REPORT_SCHEMA, TutorialEffectInstallOptions,
    TutorialEffectInstallReport, install_tutorial_effects
};
use output::{copy_directory_tree, write_new};
pub use state::RETROBUTION_PLAYER_STATUS_EFFECT_IDS;
use state::{EFFECT_RENDERER_STATUS, PROJECTILE_RENDERER_STATUS};
pub use types::{
    TutorialEffectDependencyInput, TutorialSourceFileProof, TutorialBulletRowFile,
    TutorialBulletParameters, TutorialEffectClosureFile
};
use types::{PreparedFile, OwnedTree, JsonPointer};
pub use containers::TutorialUnityObjectProof;
use containers::{DumpObject, UnityObjectKey, collect_container_routes};
use validation::{
    SourceAudit, audit_source, validate_prepared_paths,
    validate_relative, reject_symlink, json_error
};
use operations::{
    prepare_publication, build_closure, required_i32, required_f64, required_string,
    stage_prepared, stage_preserved_effect_payloads, restore_previous, cleanup_previous, canonical_regular_file, pretty_json, hash, invalid
};
use input::{collect_pointers, parse_bullet_parameters, find_unique_field};
use codec::is_preserved_effect_payload_path;
