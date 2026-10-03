use super::*;

pub(super) fn asset_reference_matches(
    reference: &CharacterCreationAssetReference,
    artifact: &ResourceSetArtifact,
) -> bool {
    reference.path == artifact.path
        && reference.bytes == artifact.bytes
        && reference.blake3 == artifact.blake3
}

pub(super) fn checked_artifact_path(asset_root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.is_empty()
        || relative.contains('\\')
        || Path::new(relative).is_absolute()
        || relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!("invalid native artifact route: {relative}"));
    }
    let path = asset_root.join(relative);
    let canonical = fs::canonicalize(&path)
        .map_err(|error| format!("cannot resolve artifact {}: {error}", path.display()))?;
    if !canonical.starts_with(asset_root) || !canonical.is_file() {
        return Err(format!("artifact is outside the asset root: {relative}"));
    }
    Ok(canonical)
}

pub(super) fn update_catalog_proof(
    document: &mut Value,
    artifact: &ResourceSetArtifact,
) -> Result<(), String> {
    let slot = document
        .pointer_mut("/provenance/playerEquipmentCatalog")
        .ok_or_else(|| {
            "character-creation contract has no playerEquipmentCatalog proof".to_owned()
        })?;
    *slot = serde_json::to_value(artifact).map_err(|error| error.to_string())?;
    Ok(())
}
