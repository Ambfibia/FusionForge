use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, SEMANTIC_AUDIO_CATALOG, SEMANTIC_AUDIO_CATALOG_SCHEMA,
    SemanticAudioCatalog, SemanticAudioCategory,
    error::io_at,
    localized_voice_install::{LOCALIZED_AUDIO_CATALOG_SCHEMA, LocalizedAudioCatalog},
};

use crate::shared::hash_file;

#[cfg(test)]
mod tests;

mod audio;
mod constants;
mod localization;
mod operations;
mod types;
mod assets;
mod output;
mod validation;
mod input;

pub use audio::{
    STRICT_AUDIO_CATALOG_SCHEMA_V3, STRICT_AUDIO_CATALOG_SCHEMA_V4,
    STRICT_AUDIO_CATALOG_SCHEMA, STRICT_AUDIO_CATALOG_PATH, STRICT_VOICE_REPORT_SCHEMA,
    DEFAULT_STRICT_VOICE_REPORT_DIRECTORY, StrictVoiceInstallOptions,
    StrictVoiceInstallReport, StrictAudioCatalog, StrictAudioCatalogCounts, StrictAudioAsset,
    StrictAudioFile
};
use audio::{
    FreshAudio, VoiceSource, VoiceSelection, StrictVoiceImportDocument, EditableAudioCatalog, validate_non_voice_base,
    collect_fresh_audio, group_fresh_audio, group_base_voice,
    voice_source_from_named, voice_source_from_fresh, infer_voice_owner, voice_logical_key, stage_strict_voice, validate_voice_selections, is_semantic_audio_file
};
use constants::{
    EXPECTED_DISCOVERED_RUSSIAN_FILES, EXPECTED_RUSSIAN_SOURCE_FILES,
    EXPECTED_IGNORED_RUSSIAN_FILES, EXPECTED_AMBIGUOUS_KEYS, EXPECTED_QUARANTINED_FILES,
    EXPECTED_NONVOICE_ASSETS, CHARACTER_CREATION_SCOPE
};
use localization::{
    DEFAULT_LOCALE, RUSSIAN_LOCALE, LocalizedKeyProof, valid_locale_id
};
pub use localization::install_strict_localized_voice;
use operations::{
    is_false, group_named_oggs, fresh_matches_binding, unique_named_by_hash,
    unique_fresh_by_hash, canonical_true_name, canonical_owner, canonical_scope,
    plan_nonvoice_destinations, semantic_context_from_provenance,
    commit_transaction, inspect_ogg,
    canonical_directory, portable_relative, portable_component, valid_hash, folded,
    transaction_stamp, invalid
};
use types::{
    CookReport, NamedOgg, AmbiguousKeyProof, QuarantineFileProof,
    IgnoredFileProof, TransactionTarget
};
use assets::{
    BaseAsset, CatalogHeader, strict_catalog_counts, flatten_legacy_variant_path,
    remove_transaction_path, replace_manifest, absolute_output_path, native_path
};
pub(crate) use assets::{
    editable_catalog_with_disk_identities, serialize_editable_runtime_catalog
};
use output::{ImportInputProof, ImportReportCounts, copy_and_verify, write_new};
use validation::{
    validate_cook_report, validate_strict_catalog, validate_installed_catalog_identity,
    validate_manifest, validate_file_identity, validate_relative_path, validate_source_build,
    invalid_error
};
use input::{
    load_base_assets, collect_named_oggs, resolve_clean_english, collect_tree_identities
};
