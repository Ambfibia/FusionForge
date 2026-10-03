use super::*;

pub(super) const ASSET_ROOT_RELATIVE: &str = "assets/game";

pub(super) fn build_next_manifest(
    manifest: &ProjectAssetManifest,
    promotions: &[PreparedPromotion],
) -> Result<(ProjectAssetManifest, u64)> {
    let removed = promotions
        .iter()
        .flat_map(|promotion| manifest_paths_below(manifest, &promotion.source_root))
        .collect::<BTreeSet<_>>();
    let expected_removed = promotions
        .iter()
        .map(|promotion| promotion.source_tree.len())
        .sum::<usize>();
    if removed.len() != expected_removed {
        return invalid("promotion source closures overlap or are incomplete in the manifest");
    }
    let preserved_entries = manifest
        .files
        .iter()
        .filter(|entry| !removed.contains(&entry.path))
        .cloned()
        .collect::<Vec<_>>();
    let mut next = manifest.clone();
    next.files = preserved_entries.clone();
    let mut folds = next
        .files
        .iter()
        .map(|entry| entry.path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    for promotion in promotions {
        for entry in &promotion.target_entries {
            if !folds.insert(entry.path.to_ascii_lowercase()) {
                return invalid(format!("promotion manifest collision at {:?}", entry.path));
            }
            next.files.push(entry.clone());
        }
    }
    next.files.sort_by(|left, right| left.path.cmp(&right.path));

    let preserved_after = next
        .files
        .iter()
        .filter(|entry| {
            !promotions.iter().any(|promotion| {
                entry
                    .path
                    .starts_with(&format!("{}/", promotion.target_root))
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    if preserved_entries != preserved_after {
        return invalid("manifest relocation changed entries outside the exact prop closures");
    }
    Ok((next, preserved_entries.len() as u64))
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
