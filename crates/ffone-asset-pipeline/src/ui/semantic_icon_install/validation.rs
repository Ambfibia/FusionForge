use super::*;

pub(super) fn validate_options(options: &SemanticIconInstallOptions) -> Result<()> {
    if options.source_build.trim().is_empty() {
        return invalid("SOURCE_BUILD must be non-empty");
    }
    let root_meta = fs::symlink_metadata(&options.asset_root)
        .map_err(|error| io_at(&options.asset_root, error))?;
    if !root_meta.is_dir() || root_meta.file_type().is_symlink() {
        return invalid("ASSET_ROOT must be a real directory, not a symlink");
    }
    let table_meta = fs::symlink_metadata(&options.table_set)
        .map_err(|error| io_at(&options.table_set, error))?;
    if !table_meta.is_file() || table_meta.file_type().is_symlink() {
        return invalid("TABLE_SET_JSON must be a real file, not a symlink");
    }
    Ok(())
}

pub(super) fn validate_previous_install(asset_root: &Path, manifest: &ProjectAssetManifest) -> Result<()> {
    let icons_root = asset_root.join(ICON_ROOT);
    let manifest_icons = manifest
        .files
        .iter()
        .filter(|entry| entry.path.starts_with("icons/"))
        .collect::<Vec<_>>();
    for entry in &manifest_icons {
        if !entry.source_path.starts_with(OWNED_SOURCE_PREFIX) {
            return invalid(format!(
                "manifest icon route {:?} is not owned by the semantic-icon installer",
                entry.path
            ));
        }
    }
    if icons_root.exists() {
        let metadata =
            fs::symlink_metadata(&icons_root).map_err(|error| io_at(&icons_root, error))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return invalid("icons root must be a real directory");
        }
        if manifest_icons.is_empty()
            && fs::read_dir(&icons_root)
                .map_err(|error| io_at(&icons_root, error))?
                .next()
                .is_some()
        {
            return invalid("icons directory exists but the manifest has no owned icon routes");
        }
    } else if !manifest_icons.is_empty() {
        return invalid("manifest contains semantic icon routes but assets/game/icons is absent");
    }
    Ok(())
}

pub(super) fn validate_manifest_paths(manifest: &ProjectAssetManifest) -> Result<()> {
    let mut paths = BTreeMap::<String, String>::new();
    for entry in &manifest.files {
        let _ = join_manifest_path(Path::new("root"), &entry.path)?;
        let folded = entry.path.to_ascii_lowercase();
        if let Some(first) = paths.insert(folded, entry.path.clone()) {
            return invalid(format!(
                "manifest path collision after semantic icon install: {first:?} and {:?}",
                entry.path
            ));
        }
    }
    Ok(())
}

pub(super) fn invalid_error(reason: impl Into<String>) -> PipelineError {
    PipelineError::UnsupportedAsset {
        path: ICON_ROOT.to_owned(),
        reason: reason.into(),
    }
}
