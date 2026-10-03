use super::*;

pub(super) fn hash_regular_file(path: &Path, label: &str) -> Result<(u64, String)> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return invalid(format!(
            "{label} must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    let mut file = fs::File::open(path).map_err(|error| io_at(path, error))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut bytes = 0_u64;
    loop {
        let count = file.read(&mut buffer).map_err(|error| io_at(path, error))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        bytes = bytes.saturating_add(count as u64);
    }
    Ok((bytes, hasher.finalize().to_hex().to_string()))
}

pub(super) fn pretty_json(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(generated_json_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative(relative)?;
    let mut path = root.to_path_buf();
    for component in relative.split('/') {
        path.push(component);
    }
    Ok(path)
}

pub(super) fn is_windows_device_name(component: &str) -> bool {
    let stem = component
        .split_once('.')
        .map_or(component, |(stem, _)| stem)
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || stem.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || stem.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}

pub(super) fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
