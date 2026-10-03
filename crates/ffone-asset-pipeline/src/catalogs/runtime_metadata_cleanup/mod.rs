use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{self, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, RUNTIME_WORLD_REGISTRY_PATH, RUNTIME_WORLD_REGISTRY_SCHEMA, Result,
    RuntimeWorldRegistry, error::io_at,
};

use crate::shared::transaction_stamp;

#[cfg(test)]
mod tests;

mod assets;
mod state;
mod codec;
mod constants;
mod audio;
mod localization;
mod operations_build_plan;
mod operations_rollback;
mod types;
mod terrain;
mod input;
mod validation;
mod systems;
mod output;

pub use assets::{
    CLEAN_RUNTIME_METADATA_INDEX_SCHEMA, CLEAN_RUNTIME_METADATA_INDEX_FILE,
    RuntimeMetadataRegistryCopy
};
use assets::{
    CLEAN_RUNTIME_METADATA_REVISION_INDEX_SCHEMA, TUTORIAL_STATIC_INSTALL_MANIFEST,
    PROVEN_OFFLINE_CONTENT_INDEX_REASON, ProvenOfflineContentIndexProof,
    proven_offline_content_index_proof, ConversionMetadataArchiveIndex,
    CompletedConversionMetadataArchiveIndex, ConversionMetadataRevisionIndex,
    normalized_asset_path, find_archived_asset_reference, normalize_direct_asset_reference,
    ProvenOfflineContentIndexPlan, plan_proven_offline_content_index, manifest_paths_below, native_path
};
pub use state::{
    CLEAN_RUNTIME_METADATA_REPORT_SCHEMA, CLEAN_RUNTIME_METADATA_REPORT_FILE,
    CleanRuntimeMetadataMode, CleanRuntimeMetadataOptions, ArchivedRuntimeMetadata,
    CleanRuntimeMetadataCounts, CleanRuntimeMetadataReport, clean_runtime_metadata
};
use state::{
    CLEAN_RUNTIME_METADATA_REVISION_REPORT_SCHEMA,
    CLEAN_RUNTIME_METADATA_REVISION_PLAN_SCHEMA, CLEAN_RUNTIME_METADATA_TRANSACTION_SCHEMA,
    CLEAN_RUNTIME_METADATA_TRANSACTION_DIRECTORY_PREFIX, CLEAN_RUNTIME_METADATA_LOCK_FILE,
    CHARACTER_RUNTIME_REGISTRIES, ACTIVE_RUNTIME_CATALOGS, runtime_world_references_root,
    verify_committed_cleanup_state
};
use codec::{
    ARCHIVE_PAYLOAD_DIRECTORY, ORPHAN_WORLD_PAYLOAD_REASON, PROVEN_CONVERSION_PAYLOAD_REASON,
    ProvenConversionPayloadProof, proven_conversion_payload_proof, ProvenOrphanWorldPayload,
    PayloadIdentity, ProvenConversionPayloadStats,
    ProvenConversionPayloadPlan, classify_proven_conversion_payload,
    archived_proven_conversion_payload_paths, OrphanWorldPayloadPlan,
    validate_orphan_payload_proof, collect_payload_tree,
    validate_archive_payload, stage_archive_payload
};
use constants::{
    ARCHIVE_REVISIONS_DIRECTORY, PROVEN_ORPHAN_WORLD_SOURCE_PACK_SCHEMA,
    PROVEN_ORPHAN_WORLD_SOURCE_PACK_BLAKE3, PROVEN_CONVERSION_RAW_NAMES
};
use audio::AUDIO_RUNTIME_REGISTRY;
use localization::{LEGACY_LOCALIZATION_CATALOG, RUNTIME_LOCALIZATION_CATALOG};
use operations_build_plan::{
    has_proven_production_source_pack, build_plan, tutorial_static_tile_id, tutorial_tile_coordinates
};
use operations_rollback::{
    revision_plan_blake3, acquire_cleanup_transaction_lock, record_cleanup_transaction_phase,
    rollback, manifested_file_is_valid, hash_file, is_zero, invalid
};
pub use types::DeferredWorldMetadata;
use types::{
    ConversionMetadataRevisionReport, ConversionMetadataRevisionPlanIdentity,
    CleanupTransactionPlanIdentity, CleanupTransactionJournal, CleanupTransactionLock,
    ArchiveHistory, CleanupPlan, MetadataDisposition, TutorialStaticGate
};
use terrain::proven_conversion_terrain_root;
use input::{
    collect_proven_conversion_payloads,
    resolve_json_reference_targets,
    discover_unregistered_world_roots, collect_json_paths,
    collect_regular_paths_from
};
use validation::{
    reject_archived_asset_references, validate_proven_offline_content_index_contract,
    validate_orphan_anchor, reject_orphan_world_references, validate_tutorial_static_gate,
    validate_completed_archive, validate_plan_manifest_identity, validate_file_identity,
    reject_stale_transactions, validate_blake3, validate_source_build, invalid_error
};
pub(crate) use validation::validate_completed_cleanup_archive;
use systems::apply_plan;
use output::{copy_new_verified, write_json_new};
#[cfg(test)]
use assets::{PROVEN_OFFLINE_CONTENT_INDEX_PATH, PROVEN_OFFLINE_CONTENT_INDEX_SOURCE_PATH, PROVEN_OFFLINE_CONTENT_INDEX_BYTES, PROVEN_OFFLINE_CONTENT_INDEX_BLAKE3, PROVEN_OFFLINE_CONTENT_INDEX_SCHEMA, PROVEN_OFFLINE_CONTENT_INDEX_PROFILE};
#[cfg(test)]
use codec::ProvenConversionPayloadKind;
#[cfg(test)]
use operations_build_plan::{proven_orphan_world_payloads, plan_proven_conversion_payloads, plan_proven_orphan_world_payloads, classify_metadata};
#[cfg(test)]
use systems::apply_plan_inner;
