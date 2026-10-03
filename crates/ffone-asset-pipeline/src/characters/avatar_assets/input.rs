use super::*;

pub(super) fn read_file(path: &Path) -> Result<Vec<u8>, AvatarCatalogError> {
    fs::read(path).map_err(|source| AvatarCatalogError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(super) fn parse_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    bytes: &[u8],
) -> Result<T, AvatarCatalogError> {
    serde_json::from_slice(bytes).map_err(|source| AvatarCatalogError::Json {
        path: path.to_owned(),
        source,
    })
}
