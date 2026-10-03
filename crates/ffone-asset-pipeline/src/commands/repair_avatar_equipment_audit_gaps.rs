use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use ffone_runtime_contracts::{
    AvatarItemCategory, AvatarItemLookup, AvatarItemVisual, AvatarTextureReference,
    CharacterCreationAssetReference, CharacterCreationAvatarItemCounts,
    CharacterCreationAvatarItems, CharacterCreationRuntimeTextures, NativeLookupStatus,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

mod assets;
mod validation;
mod models;
mod textures;
mod types;
mod constants;
mod operations;
mod systems;
mod state;
mod input;
mod output;

use assets::{
    AVATAR_ITEMS_PATH, RUNTIME_TEXTURES_PATH, EVIDENCE_PATH, verify_asset
};
use validation::GPU_AUDIT_PATH;
use models::{MODEL_REPAIRS, apply_model_repair};
use textures::{
    TEXTURE_REPAIRS, apply_texture_repair, avatar_texture_index,
    verify_published_texture, require_verified_texture
};
use types::{WhiteSlotRepair, RepairGender};
use constants::WHITE_SLOT_REPAIRS;
pub(super) use operations::run;
use operations::{
    item, visual, visual_mut
};
use systems::apply_white_slot_repair;
use state::refresh_runtime_coverage;
use input::{read_value, read_typed};
use output::write_replace;
