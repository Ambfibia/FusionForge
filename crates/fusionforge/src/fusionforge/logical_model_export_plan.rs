//! Fail-closed offline ownership planning for legacy KFM/NIF models.
//!
//! The plan never emits a GLB. A KFM can become a logical root only when its
//! catalog owner and route are unambiguous and its ownership is proven. NIF
//! references discovered in that payload become owned parts and are therefore
//! never proposed as standalone models.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};

use super::logical_model_catalog::{
    catalog_logical_models_from_path, normalize_logical_model_route, LogicalModelCatalog,
    LogicalModelKind, LogicalModelOccurrence, LogicalModelOwner, LogicalModelRouteGroup,
};

#[cfg(test)]
mod tests;

mod models;
mod validation;
mod assets;
mod containers;
mod codec;
mod types;
mod operations;
mod input;
mod animation;

pub use models::{
    LOGICAL_MODEL_EXPORT_PLAN_SCHEMA, LogicalModelExportPlanOptions, LogicalModelExportPlan,
    LogicalModelExportPlanScope, LogicalModelExportPlanCounts, LogicalModelRouteProvenance,
    LogicalModelPhysicalFingerprint, LogicalModelPhysicalAliasProof,
    NifPointerOwnershipEvidence, LogicalModelRootStatus, LogicalModelRootPlan,
    LogicalModelOwnedPart, LogicalModelStandaloneNif, LogicalModelStandaloneProof,
    LogicalModelUnresolvedReference, LogicalModelPlanBlocker,
    plan_logical_model_exports_from_path, build_logical_model_export_plan
};
use models::{
    LogicalModelResolvedRoute, LogicalModelOccurrenceFingerprint, NifContainerOccurrence,
    NifContainerGroup
};
use validation::{
    PhysicalOccurrenceResolutionError, validate_sparse_preload_hierarchy_membership
};
pub use validation::KfmResolveError;
use assets::{
    PhysicalEnvironmentRouteIndex, sole_route_provenance, resolved_route_for_group,
    project_dir_for_bundle_index, normalized_filesystem_path,
    route_group_blocker, resolve_physical_route_group, build_physical_environment_route_index, cached_serialized_asset_paths, is_unity_system_asset_ref
};
pub use assets::PointerAssetRefEvidence;
use containers::{
    PhysicalIndexedContainerOccurrence, ExactContainerOccurrence, exact_archive_bundle_paths,
    resolved_object_key_evidence,
    exact_object_body, prove_self_contained_game_object, unity_value_kind,
    is_retained_unity_system_pointer
};
pub use containers::{SelfContainedGameObjectEvidence, ResolvedObjectKeyEvidence};
pub use codec::KfmPayloadEvidence;
use codec::exact_kfm_text_asset_payload;
pub use types::{
    PreloadResolvedPointerEvidence, UnresolvedPointerEvidence,
    PreloadUnresolvedPointerEvidence, PreloadRetainedExternalPointerEvidence,
    ClosureUnresolvedPointerEvidence, ClosureRetainedExternalPointerEvidence,
    DependencyArchiveResolutionEvidence, KfmPreloadOwnershipEvidence,
    KfmPointerTraversalEvidence
};
use types::{RealKfmResolver, ExactComponentRecord};
use operations::{
    blocker_for_provenance, physical_fingerprint_for_pointer,
    physical_fingerprint_for_occurrence, exact_nonnegative_usize_field,
    exact_required_pointer_key, exact_nullable_pointer_key, exact_component_class_and_pointer,
    exact_pointer_closure, exact_typed_dependency, unresolved_pointer_evidence
};
use input::resolve_kfm_from_environment;
use animation::exact_animation_clip_paths;
#[cfg(test)]
use models::build_logical_model_export_plan_with_resolved_routes;
#[cfg(test)]
use assets::{PhysicalRouteIndexBuildStats, PhysicalNormalizedRouteIndex, PhysicalOwnerAssetRouteIndex, build_physical_owner_route_index};
