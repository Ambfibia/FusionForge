use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use crate::{
    PLAYER_ITEM_SCHEMA, PLAYER_ITEM_SET_CATALOG_SCHEMA, PlayerItemCatalogModel,
    PlayerItemDefinition, PlayerItemSetCatalog, ResourceSetArtifact, ResourceSetCatalogEntry,
    ResourceSetDocument, ResourceSetMember, verify_player_item_sets,
};
use ffone_runtime_contracts::{
    AvatarItemCategory, CharacterCreationAssetReference, CharacterCreationAvatarItems,
    CharacterCreationRuntimeTextures, NativeLookupStatus,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

mod constants;
mod types;
mod models;
mod operations;
mod assets;
mod textures;
mod codec;
mod output;
mod systems;
mod state;
mod input;

use constants::{SOURCE_ALIAS, SOURCE_BUILD, SETS};
use types::SetSpec;
use models::ModelSpec;
pub(super) use operations::run;
use operations::{
    artifact, pretty_json, sha256_hex
};
use assets::{asset_reference_matches, checked_artifact_path, update_catalog_proof};
use textures::{validate_texture_report, add_runtime_texture_contract, update_texture_audit};
use codec::decode_exact_mips;
use output::{copy_support_tree, write_new, write_replace};
use systems::update_avatar_visual;
use state::{sort_runtime_textures, refresh_runtime_coverage};
use input::{read_json_value, read_json_typed};
