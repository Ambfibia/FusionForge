use crate::Result;
use crate::equipment_logical_model_install::{
    PLAYER_EQUIPMENT_CATALOG_PATH, PLAYER_EQUIPMENT_CATALOG_SCHEMA, PlayerEquipmentCatalog,
    PlayerEquipmentCatalogModel,
};
use crate::error::{PipelineError, io_at};
use crate::manifest::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest,
};
use ffone_skinned_model::{
    NativeSampler, PublishedMipPolicy, SamplerMagFilter, SamplerMinFilter, SamplerWrapMode,
    TextureColorSpace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::shared::texture_format_name as runtime_texture_format_name;

#[cfg(test)]
mod tests;

mod constants;
mod assets;
mod state;
mod output;
mod textures_build_runtime_texture_contracts;
mod textures_repair_exact_creator_texture_aliases;
mod types;
mod models;
mod operations_build_documents;
mod operations_portable_component;
mod validation;
mod containers;
mod collision;
mod input;
mod projects;

pub use constants::{
    CHARACTER_CREATION_ROOT, CHARACTER_CREATION_NAME_WHEEL_SCHEMA,
    CHARACTER_CREATION_APPEARANCE_SCHEMA, CHARACTER_CREATION_AVATAR_ITEMS_SCHEMA
};
use constants::{
    PROTOCOL_0104, TABLE_SET_SCHEMA, EXPECTED_FIRST_NAMES, EXPECTED_MIDDLE_NAMES,
    EXPECTED_LAST_NAMES, EXPECTED_CREATION_ROWS, AVATAR_ITEM_CATEGORIES
};
pub use assets::{
    CHARACTER_CREATION_NAME_WHEEL_PATH, CHARACTER_CREATION_APPEARANCE_PATH,
    CHARACTER_CREATION_AVATAR_ITEMS_PATH, CHARACTER_CREATION_RUNTIME_TEXTURES_PATH,
    CharacterCreationAssetReference, AvatarItemLookup, NativeLookupStatus
};
use assets::{
    CatalogDocuments, effective_equipment_container_route, valid_equipment_route_resolution,
    index_manifest, upsert_manifest_entry, replace_manifest, reserve_path, normalize_path
};
pub use state::{
    CHARACTER_CREATION_RUNTIME_TEXTURES_SCHEMA, CharacterCreationRuntimeTextures,
    register_character_creation_runtime_textures
};
use state::{EXPECTED_CREATOR_RUNTIME_TEXTURES, candidate_status};
pub use output::{
    CHARACTER_CREATION_INSTALL_SCHEMA, CharacterCreationDataInstallOptions,
    CharacterCreationDataInstallReport, install_character_creation_data
};
use output::INSTALL_SEQUENCE;
use textures_build_runtime_texture_contracts::{
    EQUIPMENT_TEXTURE_SOURCE_METADATA_SCHEMA,
    EQUIPMENT_TEXTURE_SOURCE_METADATA_JSON, RUNTIME_TEXTURE_SENTINELS,
    RuntimeTextureSourceMetadataDocument, RuntimeTextureSourceMetadata, EquipmentTextureSourceMetadataDocument,
    EquipmentTextureSourceMetadata, EquipmentTextureRouteRepair,
    EquipmentTextureTrueNameRepair,
    build_runtime_texture_contracts
};
pub use textures_build_runtime_texture_contracts::{
    CharacterTextureRules, CharacterTextureSuffix, CharacterRuntimeTextureContract,
    CharacterRuntimeTextureSource, CharacterRuntimeTextureCoverage, AvatarTextureReference
};
use textures_repair_exact_creator_texture_aliases::{
    validate_equipment_texture_source_metadata, validate_runtime_texture_sentinels,
    runtime_texture_sampler, runtime_texture_coverage, texture_reference,
    required_texture_reference, texture_index, texture_candidates,
    repair_exact_creator_texture_aliases, repair_exact_equipment_texture_routes
};
pub use types::{
    CharacterCreationProvenance, CharacterCreationNameWheel, NameWheelEntry, CharacterGender,
    CharacterAppearanceCategory, CharacterCreationAppearance, CharacterColorContract,
    CharacterPaletteColor, CharacterAppearanceConstraints, CharacterCreationMaxima,
    CharacterCreationRow, CharacterCreationChoice, AvatarItemCategory,
    CharacterCreationAvatarItems, CharacterCreationAvatarItemCounts, AvatarItemVisual,
    AvatarIconReference
};
use types::ItemTable;
pub use models::AvatarModelReference;
use models::{model_alias, equipment_model_index};
pub use operations_build_documents::{avatar_item_counts, build_item_category};
use operations_build_documents::{
    build_documents, valid_equipment_true_name_resolution,
    select_table_root, true_name, native_reference, pretty_json,
    create_stage, replace_character_creation_document, canonical_directory, canonical_file
};
use operations_portable_component::{portable_component, invalid};
use validation::{
    validate_documents, validate_equipment_catalog, validate_relative, invalid_error
};
use containers::object;
use collision::strip_collision_suffix;
use input::read_verified;
use projects::project_relative;
#[cfg(test)]
use textures_repair_exact_creator_texture_aliases::texture_alias;
