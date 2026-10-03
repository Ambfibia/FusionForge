use super::*;

pub(super) fn cli_session_dir(path: &Path) -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    let dir = crate::repository_root().to_path_buf()
        .join("work")
        .join("sessions")
        .join("fusionforge-cli")
        .join(format!("{stem}_{nanos:x}"));
    fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    Ok(dir)
}
