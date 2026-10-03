use super::*;

pub(super) fn resolve_overlay_file(
    project: &Path,
    overlay_root: &Path,
    value: &str,
) -> Result<PathBuf, String> {
    validate_source_relative(value)?;
    let from_project = project.join(value.replace('\\', "/"));
    if from_project.is_file() {
        return ensure_contained_regular_file(project, &from_project);
    }
    let from_overlay = overlay_root.join(value.replace('\\', "/"));
    ensure_contained_regular_file(project, &from_overlay)
}

pub(super) fn read_regular_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|err| format!("{}: {err}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("not a regular file: {}", path.display()));
    }
    fs::read(path).map_err(|err| format!("{}: {err}", path.display()))
}

pub(super) fn read_json_value(path: &Path) -> Result<JsonValue, String> {
    let bytes = read_regular_file(path)?;
    parse_json(path, &bytes)
}

pub(super) fn parse_json(path: &Path, bytes: &[u8]) -> Result<JsonValue, String> {
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    serde_json::from_slice(bytes).map_err(|err| format!("{}: {err}", path.display()))
}
