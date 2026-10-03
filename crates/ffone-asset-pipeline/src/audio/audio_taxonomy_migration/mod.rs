use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, STRICT_AUDIO_CATALOG_PATH, STRICT_AUDIO_CATALOG_SCHEMA,
    STRICT_AUDIO_CATALOG_SCHEMA_V3, STRICT_AUDIO_CATALOG_SCHEMA_V4, SemanticAudioCategory,
    StrictAudioAsset, StrictAudioCatalog, StrictAudioCatalogCounts, StrictAudioFile,
    error::io_at,
    strict_voice_install::{
        editable_catalog_with_disk_identities, serialize_editable_runtime_catalog,
    },
};

use crate::shared::hash_file;

#[cfg(test)]
mod tests;

mod audio;
mod localization;
mod constants;
mod types;
mod assets;
mod operations_build_plan;
mod operations_rollback_incremental;
mod input;
mod codec;
mod validation;
mod systems;
mod output;

pub use audio::{
    AUDIO_TAXONOMY_MIGRATION_SCHEMA, AudioTaxonomyMigrationOptions,
    AudioTaxonomyMigrationReport, migrate_audio_taxonomy
};
use audio::{PROVEN_COMPUTRESS_DIALOGUE_SFX, strict_voice_prefix, AudioMoveTarget};
use localization::{DEFAULT_LOCALE, RUSSIAN_LOCALE, valid_locale_id};
use constants::{PROVEN_NANO_SKILL_OWNERS, RECOVERY_NANO_SKILLS, NANO_IDENTITY_ALIASES};
use types::{PlannedFile, MigrationPlan, RecoveryOgg, TransactionTarget};
use assets::{
    Route, nano_skill_route_from_owner, routed_file_path, recovery_asset,
    normalize_recovery_source_path, catalog_counts, native_path
};
use operations_build_plan::{
    build_plan, nano_owner_from_true_name,
    canonical_skill_owner,
    register_keys, inspect_ogg, stage_plan, commit_plan
};
use operations_rollback_incremental::{
    rollback_incremental, canonical_directory, portable_relative, portable_component,
    alphanumeric_identity, valid_hash, folded, transaction_stamp, invalid
};
use input::{discover_recovery_oggs, read_json};
use codec::ogg_packet_hash;
use validation::{
    validate_source_documents, validate_output_catalog, validate_production_closure,
    validate_exact_recovery_skill_routes, validate_generic_recovery_metadata,
    validate_output_manifest, validate_identity, invalid_error
};
use systems::apply_plan;
use output::{link_or_copy_and_verify, write_new};
#[cfg(test)]
use operations_build_plan::{classify_existing, canonical_current_nano_owner, normalize_recovery_scope};
#[cfg(test)]
use input::parse_named_ogg_stem;
#[cfg(test)]
use codec::ogg_packet_hash_bytes;
