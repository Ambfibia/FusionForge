use super::*;

pub(super) fn project_relative(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        invalid_error(format!(
            "native source {} is outside asset root {}",
            path.display(),
            root.display()
        ))
    })?;
    let normalized = normalize_path(relative);
    validate_relative(&normalized)?;
    Ok(normalized)
}
