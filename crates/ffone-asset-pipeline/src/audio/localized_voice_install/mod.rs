use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufReader, Read, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::{
    ASSET_MANIFEST_FILE, ClassificationCertainty, PROJECT_ASSET_SCHEMA, PipelineError,
    ProjectAssetFile, ProjectAssetKind, ProjectAssetManifest, Result, SEMANTIC_AUDIO_CATALOG,
    SEMANTIC_AUDIO_CATALOG_SCHEMA, SemanticAudioCatalog, SemanticAudioCategory,
    SemanticAudioClassificationProof, SemanticAudioCookReportProof, SemanticAudioManifestProof,
    SemanticAudioVariant, error::io_at,
};

use crate::shared::transaction_stamp;

#[cfg(test)]
mod tests;

mod localization;
mod constants;
mod audio;
mod types;
mod assets;
mod validation;
mod input;
mod operations;
mod output;

pub use localization::{
    LOCALIZED_AUDIO_CATALOG_SCHEMA, LOCALIZED_VOICE_REPORT, LOCALIZED_VOICE_REPORT_SCHEMA,
    DEFAULT_VOICE_LOCALE, RUSSIAN_VOICE_LOCALE, LocalizedVoiceInstallOptions,
    LocalizedVoiceInstallReport, LocalizedVoiceMatchMode, LocalizedVoiceSourceProof,
    LocalizedAudioVariant, LocalizedAudioAsset, LocalizedAudioCatalogCounts,
    LocalizedAudioSourceCatalogProof, LocalizedAudioMatchingPolicy, LocalizedAudioCatalog,
    LocalizedVoiceReportCounts, LocalizedVoiceMatchedFile, LocalizedVoiceUnmatchedFile,
    LocalizedVoiceAmbiguousFile, LocalizedVoiceReportDocument, install_localized_voice
};
use localization::PreparedTranslation;
use constants::{OWNED_PREFIXES, OWNED_FILES};
use audio::{
    AUDIO_TARGETS, reclassify_proven_voice, route_english_voice, route_russian_voice
};
use types::{PrimaryKey, FallbackKey, RussianSourceFile, PendingMatch};
use assets::{
    BaseCatalog, resolve_source_catalog, load_base_catalog,
    parse_russian_source_path, catalog_counts, replace_manifest, native_path, path_identity
};
use validation::{
    validate_base_assets, reject_duplicate_asset_matches, validate_staged_files,
    validate_manifest_paths, validate_unowned_collisions, validate_relative_path,
    validate_source_build, reject_stale_transactions, invalid_error
};
use input::{collect_russian_sources, parse_ogg_filename, parse_provenance_file, collect_files};
use operations::{
    match_russian_sources, prepare_translations,
    is_proven_tutorial_dialogue, commit_transaction, hash_file,
    is_owned_entry, canonical_directory, portable_relative, folded, portable_component, ambiguous_record, matching_policy, append_policy_once, invalid
};
use output::{copy_verified_ogg, write_new};
