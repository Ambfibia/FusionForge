use super::*;

pub(super) fn project_entry<'a>(
    manifest: &'a ProjectAssetManifest,
    relative: &str,
) -> Result<&'a ProjectAssetFile> {
    manifest
        .files
        .iter()
        .find(|entry| entry.path == relative)
        .ok_or_else(|| invalid_error(format!("project asset manifest lacks {relative:?}")))
}
