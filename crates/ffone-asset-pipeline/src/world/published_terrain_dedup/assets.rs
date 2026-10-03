use super::*;

pub(super) fn ensure_manifest_file(files: &[JsonValue], expected: &JsonValue, label: &str) -> Result<()> {
    if !files.contains(expected) {
        return invalid(format!(
            "tile manifest {label} artifact is absent from files"
        ));
    }
    Ok(())
}

pub(super) fn read_asset_from_roots(
    asset_root: &Path,
    map_root: &Path,
    route: &str,
    label: &str,
) -> Result<Vec<u8>> {
    let path = if let Some(relative) = route.strip_prefix("map/") {
        checked_join(map_root, relative)?
    } else {
        checked_join(asset_root, route)?
    };
    read_file(&path, label)
}

pub(super) fn verify_source_catalog(map_root: &Path, expected: &str) -> Result<()> {
    let path = map_root.join("catalog.json");
    if hash(&read_file(&path, "live map catalog")?) != expected {
        return invalid("map catalog changed after the terrain dedup plan was built");
    }
    Ok(())
}

pub(super) fn checked_report_path(
    project_root: &Path,
    asset_root: &Path,
    path: &Path,
    label: &str,
) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        project_root.join(path)
    };
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error(format!("{label} has no parent")))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    let parent = fs::canonicalize(parent).map_err(|error| io_at(parent, error))?;
    let output = parent.join(
        path.file_name()
            .ok_or_else(|| invalid_error(format!("{label} has no filename")))?,
    );
    if output.starts_with(asset_root) {
        return invalid(format!("{label} must remain outside assets/game"));
    }
    Ok(output)
}

pub(super) fn asset_relative(root: &Path, path: &Path) -> Result<String> {
    Ok(slash_path(path.strip_prefix(root).map_err(|_| {
        invalid_error(format!("asset escaped root: {}", path.display()))
    })?))
}

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
