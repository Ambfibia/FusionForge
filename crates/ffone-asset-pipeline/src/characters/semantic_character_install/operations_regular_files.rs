use super::*;

pub(super) fn reserve_destination(
    retained: &BTreeSet<String>,
    planned: &mut BTreeSet<String>,
    destination: &str,
) -> Result<()> {
    validate_relative(destination)?;
    let folded = destination.to_ascii_lowercase();
    if retained.contains(&folded) || !planned.insert(folded) {
        return invalid(format!(
            "semantic character destination collision: {destination:?}"
        ));
    }
    Ok(())
}

pub(super) fn regular_files(root: &Path) -> Result<Vec<String>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| io_at(&directory, error))? {
            let entry = entry.map_err(|error| io_at(&directory, error))?;
            let file_type = entry
                .file_type()
                .map_err(|error| io_at(entry.path(), error))?;
            if file_type.is_symlink() {
                return invalid("managed semantic character tree contains a symlink");
            }
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                files.push(relative_path(root, &entry.path())?);
            } else {
                return invalid("managed semantic character tree contains a non-regular file");
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    if !canonical.is_dir() {
        return invalid(format!("{label} is not a directory: {canonical:?}"));
    }
    Ok(canonical)
}

pub(super) fn join_relative(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative(relative)?;
    Ok(relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component)))
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
