use super::*;

pub(super) fn build_texture_index(
    manifest: &ProjectAssetManifest,
) -> Result<BTreeMap<String, Vec<ProjectAssetFile>>> {
    let mut index = BTreeMap::<String, Vec<ProjectAssetFile>>::new();
    let mut seen = BTreeSet::new();
    for entry in &manifest.files {
        let folded = entry.path.to_ascii_lowercase();
        if !seen.insert(folded) {
            return invalid(format!(
                "case-insensitive manifest path collision at {:?}",
                entry.path
            ));
        }
        if entry.kind != ProjectAssetKind::Texture || !entry.path.starts_with("textures/") {
            continue;
        }
        if let Some(stem) = texture_true_name(&entry.path) {
            index.entry(stem).or_default().push(entry.clone());
        }
    }
    for candidates in index.values_mut() {
        candidates.sort_by(|left, right| left.path.cmp(&right.path));
    }
    Ok(index)
}

pub(super) fn texture_true_name(path: &str) -> Option<String> {
    let name = path.strip_prefix("textures/")?.strip_suffix(".png")?;
    if name.contains('/') {
        return None;
    }
    let (stem, suffix) = name.rsplit_once("--")?;
    if suffix.len() != 16 || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(stem.to_ascii_lowercase())
}

pub(super) fn parse_legacy_texture_name(stem: &str) -> Option<(&'static LegacyIconKind, u32)> {
    for kind in LEGACY_ICON_KINDS {
        let Some(digits) = stem
            .strip_prefix(kind.prefix)
            .and_then(|suffix| suffix.strip_prefix('_'))
        else {
            continue;
        };
        if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) {
            if let Ok(number) = digits.parse::<u32>() {
                return Some((kind, number));
            }
        }
    }
    None
}
