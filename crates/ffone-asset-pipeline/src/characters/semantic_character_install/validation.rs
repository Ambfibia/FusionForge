use super::*;

pub(super) fn validate_existing_managed_tree(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
) -> Result<()> {
    let entries = manifest
        .files
        .iter()
        .filter(|entry| is_managed_path(&entry.path))
        .map(|entry| (entry.path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    for (relative, entry) in &entries {
        let path = join_relative(asset_root, relative)?;
        let bytes = read_file(&path)?;
        if bytes.len() as u64 != entry.bytes
            || blake3::hash(&bytes).to_hex().as_str() != entry.blake3
        {
            return invalid(format!(
                "existing semantic character file no longer matches the manifest: {relative:?}"
            ));
        }
    }
    let mut disk = BTreeSet::new();
    for target in MANAGED_TARGETS {
        let path = join_relative(asset_root, target)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(io_at(&path, error)),
        };
        if metadata.file_type().is_symlink() {
            return invalid("existing semantic character target is a symlink");
        }
        if metadata.is_file() {
            disk.insert((*target).to_owned());
        } else if metadata.is_dir() {
            for relative in regular_files(&path)? {
                disk.insert(format!("{target}/{relative}"));
            }
        } else {
            return invalid("existing semantic character target is not regular");
        }
    }
    if disk != entries.keys().cloned().collect::<BTreeSet<_>>() {
        return invalid("existing semantic character files and manifest ownership differ");
    }
    Ok(())
}

pub(super) fn validate_semantic_directories(directories: &[String]) -> Result<String> {
    if directories.is_empty() {
        return invalid("candidate has no semantic directories");
    }
    for directory in directories {
        validate_file_name(directory)?;
    }
    Ok(directories.join("/"))
}

pub(super) fn validate_file_name(value: &str) -> Result<()> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
    {
        return invalid(format!("unsafe semantic path component {value:?}"));
    }
    Ok(())
}

pub(super) fn validate_relative(relative: &str) -> Result<()> {
    if relative.is_empty() || relative.contains('\\') {
        return invalid("relative path must be non-empty and slash-normalized");
    }
    let path = Path::new(relative);
    if path.is_absolute() {
        return invalid("absolute path is forbidden");
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return invalid("relative path contains a non-normal component");
        }
    }
    Ok(())
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::LogicalModelPublish(format!(
        "semantic character install failed: {}",
        message.into()
    ))
}
