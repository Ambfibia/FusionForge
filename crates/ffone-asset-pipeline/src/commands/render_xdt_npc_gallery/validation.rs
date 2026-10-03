use super::*;

pub(super) fn require_file(path: Option<PathBuf>, flag: &str) -> Result<PathBuf, String> {
    canonical_file(&path.ok_or_else(|| format!("{flag} is required"))?)
}

pub(super) fn require_directory(path: Option<PathBuf>, flag: &str) -> Result<PathBuf, String> {
    let path = path.ok_or_else(|| format!("{flag} is required"))?;
    let canonical = fs::canonicalize(&path)
        .map_err(|error| format!("cannot resolve {}: {error}", path.display()))?;
    if !canonical.is_dir() {
        return Err(format!("{} is not a directory", canonical.display()));
    }
    Ok(canonical)
}
