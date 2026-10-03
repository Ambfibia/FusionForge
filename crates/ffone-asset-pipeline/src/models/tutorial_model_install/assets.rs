use super::*;

pub(super) const MANIFEST_NEXT: &str = ".asset-manifest.tutorial-models.next";

pub(super) const MANIFEST_BACKUP: &str = ".asset-manifest.tutorial-models.backup";

pub(super) fn index_batch_mappings(
    mappings: &[LogicalModelBatchMapping],
) -> Result<BTreeMap<String, &LogicalModelBatchMapping>> {
    let mut indexed = BTreeMap::new();
    for mapping in mappings {
        validate_mapping(mapping)?;
        let folded = casefold(&mapping.output_glb);
        if indexed.insert(folded, mapping).is_some() {
            return invalid(format!(
                "candidate batch has a case-folded output collision at {:?}",
                mapping.output_glb
            ));
        }
    }
    Ok(indexed)
}

pub(super) fn exact_index_path<'a>(
    index: &'a BTreeMap<String, String>,
    expected: &str,
) -> Result<Option<&'a str>> {
    let Some(actual) = index.get(&casefold(expected)) else {
        return Ok(None);
    };
    if actual != expected {
        return invalid(format!(
            "evidence path casing differs from exact sidecar: expected {expected:?}, found {actual:?}"
        ));
    }
    Ok(Some(actual))
}

pub(super) fn index_regular_tree(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = BTreeMap::new();
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| io_at(&directory, error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| io_at(&directory, error))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| io_at(&path, error))?;
            if kind.is_symlink() {
                return invalid(format!("installer input tree contains symlink {path:?}"));
            }
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                let relative = relative_slash(root, &path)?;
                validate_relative(&relative)?;
                let folded = casefold(&relative);
                if let Some(previous) = files.insert(folded, relative.clone()) {
                    return invalid(format!(
                        "installer input tree has a case-fold collision: {previous:?} and {relative:?}"
                    ));
                }
            } else {
                return invalid(format!(
                    "installer input tree contains non-regular entry {path:?}"
                ));
            }
        }
    }
    Ok(files)
}

pub(super) fn owned_path(path: &str) -> bool {
    path == TUTORIAL_MODEL_CATALOG_PATH
        || path
            .strip_prefix(TUTORIAL_MODEL_ROOT)
            .is_some_and(|tail| tail.starts_with('/'))
}

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
