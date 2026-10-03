use super::*;

pub(super) fn link_or_copy_and_verify(source: &PlannedFile, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    if fs::hard_link(&source.source_absolute, target).is_err() {
        fs::copy(&source.source_absolute, target).map_err(|error| io_at(target, error))?;
    }
    validate_identity(target, source.bytes, &source.blake3)
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_at(parent, source))?;
    }
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|source| io_at(path, source))?;
    file.write_all(bytes)
        .map_err(|source| io_at(path, source))?;
    file.sync_all().map_err(|source| io_at(path, source))
}
