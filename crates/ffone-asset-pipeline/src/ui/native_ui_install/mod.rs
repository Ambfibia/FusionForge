use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    ASSET_MANIFEST_FILE, PipelineError, ProjectAssetFile, ProjectAssetKind, ProjectAssetManifest,
    Result,
    error::io_at,
    native_ui_icon_routes::{VERIFIED_ICON_ROUTES, VerifiedIconRoute},
};

#[cfg(test)]
mod tests;

mod constants_archived_gameplay_ui_revision;
mod constants_routes;
mod assets;
mod layout;
mod output;
mod types;
mod models;
mod input;
mod operations;
mod projects;
mod validation;

pub use constants_archived_gameplay_ui_revision::GAMEPLAY_UI_ROOT;
use constants_archived_gameplay_ui_revision::{
    ARCHIVED_GAMEPLAY_UI_REVISION, GUI_SKIN_BYTES, GUI_SKIN_BLAKE3
};
use constants_routes::ROUTES;
pub use assets::{GAMEPLAY_UI_CATALOG, GAMEPLAY_UI_CATALOG_SCHEMA};
use assets::{
    GUI_SKIN_PATH, Route, route, ArchivedCatalog, load_archived_catalog, desired_route_sources, verify_route_source,
    replace_manifest
};
pub use layout::GAMEPLAY_UI_LAYOUT;
use layout::{LAYOUT_BYTES, is_supported_layout_revision};
#[cfg(test)]
use layout::PREVIOUS_LAYOUT_BLAKE3;
pub use output::{
    NativeGameplayUiInstallOptions, NativeGameplayUiInstallReport, install_native_gameplay_ui
};
use output::write_new;
use types::ArchivedRevisionFile;
use models::validate_archived_route_model;
use input::collect_runtime_files;
use operations::{verify_table_data_icon_source, invalid};
use projects::project_entry;
use validation::invalid_error;
#[cfg(test)]
use assets::{STRICT_ROUTE_PROOFS, strict_route_proof};
#[cfg(test)]
use layout::PREVIOUS_LAYOUT_BYTES;
#[cfg(test)]
use output::{NativeGameplayUiInstallFailpoint, install_native_gameplay_ui_with_failpoint};
