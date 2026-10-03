use super::*;

pub(super) fn portable_relative_parts(path: &Path, label: &str) -> Result<Vec<String>, String> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(format!(
            "{label} must be a non-empty portable relative path"
        ));
    }
    let text = path
        .to_str()
        .ok_or_else(|| format!("{label} must be valid Unicode"))?
        .replace('\\', "/");
    let bytes = text.as_bytes();
    if text.starts_with("//")
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
    {
        return Err(format!(
            "{label} must be a non-empty portable relative path"
        ));
    }
    let mut parts = Vec::new();
    for component in Path::new(&text).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => parts.push(
                value
                    .to_str()
                    .ok_or_else(|| format!("{label} must be valid Unicode"))?
                    .to_string(),
            ),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("{label} may not contain root or parent traversal"));
            }
        }
    }
    if parts.is_empty() {
        return Err(format!("{label} resolves to an empty path"));
    }
    Ok(parts)
}

pub(super) fn preflight_output(
    output: Option<&Path>,
    request: &Path,
    managed_evidence: &Path,
    managed_payload: Option<&Path>,
) -> Result<(), String> {
    let Some(output) = output else {
        return Ok(());
    };
    let output_identity = target_identity(output, "report output")?;
    let request_identity =
        fs::canonicalize(request).map_err(|error| format!("{}: {error}", request.display()))?;
    let evidence_identity = fs::canonicalize(managed_evidence)
        .map_err(|error| format!("{}: {error}", managed_evidence.display()))?;
    if same_target(&output_identity, &request_identity) {
        return Err("report output must not overwrite the request JSON".to_string());
    }
    if same_target(&output_identity, &evidence_identity) {
        return Err("report output must not overwrite managed assembly evidence".to_string());
    }
    if let Some(payload) = managed_payload {
        let payload_identity =
            fs::canonicalize(payload).map_err(|error| format!("{}: {error}", payload.display()))?;
        if same_target(&output_identity, &payload_identity) {
            return Err("report output must not overwrite the managed payload".to_string());
        }
    }
    Ok(())
}

pub(super) fn target_identity(path: &Path, label: &str) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() || path.is_dir() {
        return Err(format!("{label} must name a file"));
    }
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(format!("{label} may not contain parent traversal"));
    }
    if path.exists() {
        return fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()));
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let mut existing = absolute.as_path();
    let mut missing = Vec::<std::ffi::OsString>::new();
    while !existing.exists() {
        let name = existing
            .file_name()
            .ok_or_else(|| format!("{label} has no existing ancestor"))?;
        missing.push(name.to_os_string());
        existing = existing
            .parent()
            .ok_or_else(|| format!("{label} has no existing ancestor"))?;
    }
    let mut identity =
        fs::canonicalize(existing).map_err(|error| format!("{}: {error}", existing.display()))?;
    for part in missing.iter().rev() {
        identity.push(part);
    }
    Ok(identity)
}

pub(super) fn same_target(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}
