use super::*;

pub(super) const AVATAR_ITEMS_PATH: &str = "data/character_creation/avatar_items.json";

pub(super) const RUNTIME_TEXTURES_PATH: &str = "data/character_creation/runtime_textures.json";

pub(super) const EVIDENCE_PATH: &str = "target/ffone-audits/avatar-equipment-native-donor-repair.json";

pub(super) fn verify_asset(
    asset_root: &Path,
    reference: &CharacterCreationAssetReference,
) -> Result<(), String> {
    let path = checked_asset_path(asset_root, &reference.path)?;
    let bytes =
        fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if bytes.len() as u64 != reference.bytes
        || blake3::hash(&bytes).to_hex().as_str() != reference.blake3
    {
        return Err(format!(
            "native donor byte identity changed: {}",
            reference.path
        ));
    }
    Ok(())
}

pub(super) fn checked_asset_path(asset_root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.is_empty()
        || relative.contains('\\')
        || Path::new(relative).is_absolute()
        || relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!("invalid native asset route: {relative}"));
    }
    let path = fs::canonicalize(asset_root.join(relative))
        .map_err(|error| format!("cannot resolve native asset {relative}: {error}"))?;
    if !path.starts_with(asset_root) || !path.is_file() {
        return Err(format!("native asset is outside root: {relative}"));
    }
    Ok(path)
}
