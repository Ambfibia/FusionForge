use super::*;

pub(super) fn copy_file_new_verified(
    source: &Path,
    target: &Path,
    expected_bytes: u64,
    expected_blake3: &str,
) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| invalid_error("copy target has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    let mut created = false;
    let result = (|| -> Result<()> {
        let mut input = fs::File::open(source).map_err(|error| io_at(source, error))?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(target)
            .map_err(|error| io_at(target, error))?;
        created = true;
        io::copy(&mut input, &mut output).map_err(|error| io_at(target, error))?;
        output.sync_all().map_err(|error| io_at(target, error))?;
        verify_file_identity(target, expected_bytes, expected_blake3)
    })();
    if result.is_err() && created {
        let _ = fs::remove_file(target);
    }
    result
}

pub(super) fn write_bytes_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("write target has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| io_at(path, error))?;
    file.write_all(bytes).map_err(|error| io_at(path, error))?;
    file.sync_all().map_err(|error| io_at(path, error))
}

pub(super) fn write_json_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = pretty_json_bytes(value, path)?;
    write_bytes_new(path, &bytes)
}
