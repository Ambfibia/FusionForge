use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, error::io_at,
};

#[cfg(test)]
mod tests;

mod assets;
mod constants;
mod output;
mod types;
mod validation;
mod operations;
mod input;
mod textures;

pub use assets::{
    SEMANTIC_ICON_CATALOG_SCHEMA, SemanticIconSourceAsset, SemanticIconAsset,
    SemanticIconCatalogProofs, SemanticIconCatalogCounts, SemanticIconCatalog
};
use assets::{
    canonical_source_manifest_hash, manifest_relative_path, join_manifest_path,
    replace_manifest
};
use constants::{
    TABLE_SET_SCHEMA, ICON_ROOT, OWNED_SOURCE_PREFIX, CATEGORY_DIRECTORIES, LEGACY_ICON_KINDS
};
pub use output::{SemanticIconInstallOptions, SemanticIconInstallReport, install_semantic_icons};
pub use types::{
    SemanticIconCategory, SemanticIconUnmatchedReason, SemanticIconTableReference,
    SemanticIconClassificationProof, SemanticIconUnmatched
};
use types::{TableSetDocument, LegacyIconKind, IconReferenceGroup, PreparedIcon};
use validation::{
    validate_options, validate_previous_install, validate_manifest_paths, invalid_error
};
use operations::{
    verify_table_set, classify_references, append_unreferenced_textures, unmatched_reason_counts, reason_count, invalid
};
use input::collect_table_references;
use textures::{build_texture_index, texture_true_name, parse_legacy_texture_name};
#[cfg(test)]
use operations::legacy_kind;
