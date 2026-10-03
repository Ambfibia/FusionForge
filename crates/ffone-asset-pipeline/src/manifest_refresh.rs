//! Compatibility publication verification for pre-domain publisher call sites.
//!
//! The project no longer mutates a global asset manifest. New publishers must
//! publish their domain catalog atomically with its payload. These helpers are
//! retained while older publishers are split into domain transactions; they
//! now only validate and hash the file that was published.

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;

use crate::{PipelineError, ProjectAssetKind, Result, error::io_at};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAssetManifestRefresh {
    pub path: String,
    pub previous_bytes: u64,
    pub bytes: u64,
    pub previous_blake3: String,
    pub blake3: String,
    pub changed: bool,
}

/// Verify a newly published runtime file without touching global metadata.
pub fn refresh_project_asset_manifest_entry(
    asset_root: &Path,
    relative_path: &str,
) -> Result<ProjectAssetManifestRefresh> {
    verify_published_file(asset_root, relative_path)
}

/// Compatibility alias for publishers that previously registered a new route.
/// Route ownership is now enforced by the release dependency graph.
pub fn register_project_asset_manifest_entry(
    asset_root: &Path,
    relative_path: &str,
    _kind: ProjectAssetKind,
) -> Result<ProjectAssetManifestRefresh> {
    verify_published_file(asset_root, relative_path)
}

fn verify_published_file(
    asset_root: &Path,
    relative_path: &str,
) -> Result<ProjectAssetManifestRefresh> {
    let relative = validate_relative_path(relative_path)?;
    let asset_path = asset_root.join(relative);
    let metadata = fs::metadata(&asset_path).map_err(|source| io_at(&asset_path, source))?;
    if !metadata.is_file() {
        return Err(PipelineError::ProjectAssetManifest(format!(
            "published project asset is not a file: {}",
            asset_path.display()
        )));
    }
    let bytes = metadata.len();
    let blake3 = hash_file(&asset_path)?;
    Ok(ProjectAssetManifestRefresh {
        path: relative_path.to_owned(),
        previous_bytes: 0,
        bytes,
        previous_blake3: String::new(),
        blake3,
        changed: true,
    })
}

fn validate_relative_path(path: &str) -> Result<PathBuf> {
    if path.is_empty() || path.contains('\\') {
        return Err(PipelineError::ProjectAssetManifest(format!(
            "invalid project-asset path {path:?}"
        )));
    }
    let parsed = Path::new(path);
    if parsed.is_absolute()
        || parsed
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(PipelineError::ProjectAssetManifest(format!(
            "unsafe project-asset path {path:?}"
        )));
    }
    Ok(path.split('/').fold(PathBuf::new(), |mut output, part| {
        output.push(part);
        output
    }))
}

use crate::shared::hash_file;

#[cfg(test)]
mod tests;
