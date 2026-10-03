use super::*;

pub(super) fn project_entry(
    source_path: &str,
    path: &str,
    kind: ProjectAssetKind,
    bytes: &[u8],
) -> ProjectAssetFile {
    ProjectAssetFile {
        source_path: source_path.to_owned(),
        path: path.to_owned(),
        kind,
        bytes: bytes.len() as u64,
        blake3: blake3::hash(bytes).to_hex().to_string(),
    }
}
