use super::*;

pub(super) fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| io_at(&directory, error))? {
            let entry = entry.map_err(|error| io_at(&directory, error))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| io_at(&path, error))?;
            if kind.is_symlink() {
                return invalid(format!(
                    "object package contains a symlink: {}",
                    path.display()
                ));
            }
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn read_file(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} is not a regular file: {}", path.display()));
    }
    fs::read(path).map_err(|error| io_at(path, error))
}

pub(super) fn read_json(path: &Path, label: &str) -> Result<JsonValue> {
    let bytes = read_file(path, label)?;
    serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })
}

pub(super) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| invalid_error("truncated u32"))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}
