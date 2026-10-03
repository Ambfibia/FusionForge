use super::*;

pub(super) fn copy_verified_ogg(source: &Path, target: &Path) -> Result<(u64, String)> {
    let mut input = fs::File::open(source).map_err(|error| io_at(source, error))?;
    let mut magic = [0_u8; 4];
    input
        .read_exact(&mut magic)
        .map_err(|error| io_at(source, error))?;
    if &magic != b"OggS" {
        return invalid(format!(
            "source is not an Ogg bitstream: {}",
            source.display()
        ));
    }
    drop(input);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let bytes = fs::copy(source, target).map_err(|error| io_at(target, error))?;
    let hash = hash_file(target)?;
    Ok((bytes, hash))
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| io_at(path, error))?;
    file.write_all(bytes).map_err(|error| io_at(path, error))?;
    file.sync_all().map_err(|error| io_at(path, error))
}
