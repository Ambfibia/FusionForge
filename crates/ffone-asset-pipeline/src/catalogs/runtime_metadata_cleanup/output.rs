use super::*;

pub(super) fn copy_new_verified(
    source: &Path,
    target: &Path,
    expected_bytes: u64,
    expected_blake3: &str,
) -> Result<()> {
    let mut created = false;
    let result = (|| -> Result<()> {
        let mut input = fs::File::open(source).map_err(|error| io_at(source, error))?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(target)
            .map_err(|error| io_at(target, error))?;
        created = true;
        let copied = io::copy(&mut input, &mut output).map_err(|error| io_at(target, error))?;
        output.sync_all().map_err(|error| io_at(target, error))?;
        let actual_blake3 = hash_file(target)?;
        if copied != expected_bytes || actual_blake3 != expected_blake3 {
            return invalid(format!(
                "runtime registry identity mismatch after copying {} to {}",
                source.display(),
                target.display()
            ));
        }
        Ok(())
    })();
    if result.is_err() && created {
        let _ = fs::remove_file(target);
    }
    result
}

pub(super) fn write_json_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })?;
    bytes.push(b'\n');
    let mut created = false;
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|error| io_at(path, error))?;
        created = true;
        file.write_all(&bytes).map_err(|error| io_at(path, error))?;
        file.sync_all().map_err(|error| io_at(path, error))
    })();
    if result.is_err() && created {
        let _ = fs::remove_file(path);
    }
    result
}
