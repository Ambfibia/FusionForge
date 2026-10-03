use super::*;

pub(super) fn absolute_under_project(project_root: &Path, path: &Path, label: &str) -> Result<PathBuf> {
    let path = absolute_from(project_root, path);
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error(format!("{label} has no parent")))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    let parent = canonical_directory(parent, "report parent")?;
    let file_name = path
        .file_name()
        .ok_or_else(|| invalid_error(format!("{label} has no filename")))?;
    let path = parent.join(file_name);
    if !path.starts_with(project_root) {
        return invalid(format!("{label} escaped project root"));
    }
    Ok(path)
}

pub(super) fn project_relative(project_root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(project_root)
        .map_err(|_| invalid_error("path escaped project root"))?;
    Ok(slash_path(relative))
}
