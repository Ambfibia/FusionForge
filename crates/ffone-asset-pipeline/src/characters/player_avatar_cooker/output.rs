use super::*;

pub(super) fn write_new_output(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        return player_error(format!(
            "player cook evidence is immutable and will not be overwritten: {}",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_at(path, error))?;
    file.write_all(bytes).map_err(|error| io_at(path, error))?;
    file.sync_all().map_err(|error| io_at(path, error))
}
