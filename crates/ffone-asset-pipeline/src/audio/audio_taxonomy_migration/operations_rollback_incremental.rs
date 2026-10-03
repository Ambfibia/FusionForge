use super::*;

pub(super) fn rollback_incremental(
    audio_moves: &[AudioMoveTarget],
    installed_audio: &[usize],
    backed_up_audio: &[usize],
    metadata_targets: &[TransactionTarget],
    installed_metadata: &[usize],
    backed_up_metadata: &[usize],
) {
    for index in installed_metadata.iter().rev() {
        let item = &metadata_targets[*index];
        let _ = fs::rename(&item.target, &item.staged);
    }
    for index in installed_audio.iter().rev() {
        let item = &audio_moves[*index];
        let _ = fs::rename(&item.target, &item.staged);
    }
    for index in backed_up_metadata.iter().rev() {
        let item = &metadata_targets[*index];
        let _ = fs::rename(&item.backup, &item.target);
    }
    for index in backed_up_audio.iter().rev() {
        let item = &audio_moves[*index];
        if let (Some(source), Some(backup)) = (&item.source, &item.backup) {
            if let Some(parent) = source.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::rename(backup, source);
        }
    }
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|source| io_at(path, source))?;
    if !canonical.is_dir() {
        return invalid(format!("{label} is not a directory: {}", path.display()));
    }
    Ok(canonical)
}

pub(super) fn portable_relative(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        invalid_error(format!(
            "{} is outside recovery root {}",
            path.display(),
            root.display()
        ))
    })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return invalid(format!("unsafe recovery path {}", path.display()));
        };
        parts.push(
            value
                .to_str()
                .ok_or_else(|| invalid_error(format!("non-UTF-8 path {}", path.display())))?,
        );
    }
    Ok(parts.join("/"))
}

pub(super) fn portable_component(value: &str, context: &str) -> Result<String> {
    let normalized = value.nfkc().collect::<String>().to_lowercase();
    let mut output = String::new();
    let mut separator = false;
    for character in normalized.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character);
            separator = false;
        } else if !separator && !output.is_empty() {
            output.push('_');
            separator = true;
        }
    }
    while output.ends_with('_') {
        output.pop();
    }
    if output.is_empty() || matches!(output.as_str(), "." | "..") {
        return invalid(format!("{context} has no portable identity: {value:?}"));
    }
    Ok(output)
}

pub(super) fn alphanumeric_identity(value: &str) -> String {
    value
        .nfkc()
        .flat_map(char::to_lowercase)
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

pub(super) fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn folded(value: &str) -> String {
    value.nfkc().flat_map(char::to_lowercase).collect()
}

pub(super) fn transaction_stamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
