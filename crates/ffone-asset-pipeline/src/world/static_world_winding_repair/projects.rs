use super::*;

pub(super) fn absolute_under_project(project_root: &Path, path: &Path, label: &str) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        project_root.join(path)
    };
    let parent = absolute
        .parent()
        .ok_or_else(|| invalid_error(format!("{label} has no parent")))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    let parent = fs::canonicalize(parent).map_err(|error| io_at(parent, error))?;
    let name = absolute
        .file_name()
        .ok_or_else(|| invalid_error(format!("{label} has no filename")))?;
    let resolved = parent.join(name);
    if !resolved.starts_with(project_root) {
        return invalid(format!("{label} must remain inside the project root"));
    }
    Ok(resolved)
}

pub(super) fn project_relative(project_root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(project_root)
        .map_err(|_| invalid_error(format!("path escaped project root: {}", path.display())))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}
