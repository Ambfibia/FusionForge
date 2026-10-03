use super::*;

pub(super) fn write_json_replace(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = pretty_json(value)?;
    let temporary = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("json")
    ));
    if temporary.exists() {
        fs::remove_file(&temporary).map_err(|error| io_at(&temporary, error))?;
    }
    write_new(&temporary, &bytes)?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| io_at(path, error))?;
    }
    fs::rename(&temporary, path).map_err(|error| io_at(path, error))
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options.open(path).map_err(|error| io_at(path, error))?;
    use std::io::Write;
    file.write_all(bytes).map_err(|error| io_at(path, error))?;
    file.sync_all().map_err(|error| io_at(path, error))
}
