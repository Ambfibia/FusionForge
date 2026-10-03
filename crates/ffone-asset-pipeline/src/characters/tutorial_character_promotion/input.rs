use super::*;

pub(super) fn collect_tree(root: &Path, omit_publish_reports: bool) -> Result<BTreeMap<String, FileIdentity>> {
    let metadata = fs::symlink_metadata(root).map_err(|source| io_at(root, source))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return invalid(format!(
            "package root is not a regular directory: {}",
            root.display()
        ));
    }
    let mut output = BTreeMap::new();
    collect_tree_at(root, root, omit_publish_reports, &mut output)?;
    if output.is_empty() {
        return invalid(format!("package is empty: {}", root.display()));
    }
    Ok(output)
}

pub(super) fn collect_tree_at(
    root: &Path,
    current: &Path,
    omit_publish_reports: bool,
    output: &mut BTreeMap<String, FileIdentity>,
) -> Result<()> {
    let mut entries = fs::read_dir(current)
        .map_err(|source| io_at(current, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| io_at(current, source))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| io_at(&path, source))?;
        if metadata.file_type().is_symlink() {
            return invalid(format!("symlink in promoted package: {}", path.display()));
        }
        if metadata.is_dir() {
            collect_tree_at(root, &path, omit_publish_reports, output)?;
            continue;
        }
        if !metadata.is_file() {
            return invalid(format!("unsupported package entry: {}", path.display()));
        }
        let relative = slash_path(path.strip_prefix(root).map_err(|_| {
            invalid_error(format!("package file escaped root: {}", path.display()))
        })?);
        if omit_publish_reports && relative.ends_with(".publish.json") {
            continue;
        }
        validate_relative(&relative)?;
        let bytes = read_regular(&path)?;
        let identity = FileIdentity {
            bytes: bytes.len() as u64,
            blake3: blake3::hash(&bytes).to_hex().to_string(),
        };
        if output.insert(relative.clone(), identity).is_some() {
            return invalid(format!("duplicate package path {relative:?}"));
        }
    }
    Ok(())
}

pub(super) fn read_regular(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_at(path, source))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("expected regular file: {}", path.display()));
    }
    fs::read(path).map_err(|source| io_at(path, source))
}
