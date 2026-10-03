use super::*;

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if fs::symlink_metadata(path).is_ok() {
        return invalid(format!("resource-set output already exists: {path:?}"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("resource-set output has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    fs::write(path, bytes).map_err(|error| io_at(path, error))
}

pub(super) fn write_replace(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary, bytes).map_err(|error| io_at(&temporary, error))?;
    if path.is_file() {
        fs::remove_file(path).map_err(|error| io_at(path, error))?;
    }
    fs::rename(&temporary, path).map_err(|error| io_at(path, error))
}

pub(super) fn copy_new_or_equal(source: &Path, destination: &Path) -> Result<()> {
    let bytes = fs::read(source).map_err(|error| io_at(source, error))?;
    if destination.is_file() {
        let existing = fs::read(destination).map_err(|error| io_at(destination, error))?;
        if existing == bytes {
            return Ok(());
        }
        return invalid(format!(
            "different resource bytes collide at {destination:?}"
        ));
    }
    write_new(destination, &bytes)
}
