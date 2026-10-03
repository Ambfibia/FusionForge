use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, error::io_at,
};

#[cfg(test)]
mod tests;

mod audio_character_creation_audio_routes;
mod audio_stage_semantic_audio;
mod constants;
mod assets;
mod types;
mod output;
mod operations;
mod validation;
mod input;

pub use audio_character_creation_audio_routes::{
    SEMANTIC_AUDIO_CATALOG, SEMANTIC_AUDIO_CATALOG_SCHEMA, RETROBUTION_AUDIO_ASSET_COUNT,
    SemanticAudioInstallOptions, SemanticAudioCategory, SemanticAudioClassificationProof,
    SemanticAudioVariant, SemanticAudioAsset, SemanticAudioCatalogCounts,
    SemanticAudioManifestProof, SemanticAudioCookReportProof, SemanticAudioCatalog,
    SemanticAudioInstallReport, install_semantic_audio
};
use audio_character_creation_audio_routes::{
    PreparedAudio, is_hashed_audio_path,
    classify_character_creation_audio, voice_owner
};
use audio_stage_semantic_audio::{classify_sfx_owner, stage_semantic_audio};
use constants::{
    RETROBUTION_EXACT_20260613_BUILD_UUID, RETROBUTION_EXACT_20260613_COOK_REPORT_SHA256,
    CONTENT_PACK_SCHEMA, COOK_REPORT_SCHEMA, OWNED_DIRECTORIES
};
use assets::{
    RETROBUTION_EXACT_20260613_PACK_MANIFEST_BLAKE3, OWNED_MANIFEST_PREFIXES,
    canonical_path_identity, catalog_counts, join_manifest_path, replace_manifest
};
pub use types::ClassificationCertainty;
use types::{CookReport, CookMapping, MappingGroup, Classification};
use output::{inspect_previous_install, write_new};
use operations::{
    sha256_hex, is_owned_entry, is_original_hashed_ogg, classify, assign_destinations, normalized_identity, portable_component, canonical_provenance, join_relative,
    verify_source_bytes, commit_transaction, invalid
};
use validation::{
    reject_stale_transactions, validate_source_build, validate_mapping, validate_hash,
    validate_hash_suffix, validate_relative_semantic_path,
    validate_manifest_destination_closure, validate_staged_files, validate_manifest_paths,
    invalid_error
};
use input::collect_files;
#[cfg(test)]
use audio_character_creation_audio_routes::{RETROBUTION_EXACT_20260613_AUDIO_ASSET_COUNT, CharacterCreationAudioKind, CharacterCreationAudioRoute, CHARACTER_CREATION_AUDIO_ROUTES, select_audio_profile};
#[cfg(test)]
use types::CookCounts;
#[cfg(test)]
use operations::contains_hash_suffix;
