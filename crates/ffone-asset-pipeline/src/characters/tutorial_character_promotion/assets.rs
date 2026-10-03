use super::*;

pub(super) fn update_registry_manifest_entry(
    manifest: &mut ProjectAssetManifest,
    registry_bytes: &[u8],
) -> Result<()> {
    let mut matches = manifest
        .files
        .iter_mut()
        .filter(|entry| entry.path == SEMANTIC_CHARACTER_REGISTRY_PATH);
    let entry = matches
        .next()
        .ok_or_else(|| invalid_error("project manifest has no character registry entry"))?;
    if matches.next().is_some() || entry.kind != ProjectAssetKind::Data {
        return invalid("character registry manifest entry is duplicate or not data");
    }
    entry.bytes = registry_bytes.len() as u64;
    entry.blake3 = blake3::hash(registry_bytes).to_hex().to_string();
    Ok(())
}

pub(super) fn index_batch(
    mappings: &[LogicalModelBatchMapping],
) -> Result<BTreeMap<String, LogicalModelBatchMapping>> {
    let mut index = BTreeMap::new();
    for mapping in mappings {
        validate_relative(&mapping.output_glb)?;
        if index
            .insert(mapping.output_glb.clone(), mapping.clone())
            .is_some()
        {
            return invalid(format!(
                "candidate batch has duplicate output {:?}",
                mapping.output_glb
            ));
        }
    }
    Ok(index)
}

pub(super) fn index_manifest(manifest: &ProjectAssetManifest) -> Result<BTreeMap<String, ProjectAssetFile>> {
    let mut index = BTreeMap::new();
    let mut folds = BTreeSet::new();
    for entry in &manifest.files {
        validate_relative(&entry.path)?;
        if !folds.insert(entry.path.to_ascii_lowercase())
            || index.insert(entry.path.clone(), entry.clone()).is_some()
        {
            return invalid(format!("duplicate manifest path {:?}", entry.path));
        }
    }
    Ok(index)
}

pub(super) fn manifest_paths_below(manifest: &ProjectAssetManifest, root: &str) -> BTreeSet<String> {
    let prefix = format!("{root}/");
    manifest
        .files
        .iter()
        .filter(|entry| entry.path.starts_with(&prefix))
        .map(|entry| entry.path.clone())
        .collect()
}

pub(super) fn native_path(path: &str) -> PathBuf {
    path.split('/').collect()
}

pub(super) fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
