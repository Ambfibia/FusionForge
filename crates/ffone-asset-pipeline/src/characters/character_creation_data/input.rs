use super::*;

pub(super) fn read_verified(asset_root: &Path, entry: &ProjectAssetFile) -> Result<Vec<u8>> {
    let path = asset_root.join(&entry.path);
    let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
    let hash = blake3::hash(&bytes).to_hex().to_string();
    if bytes.len() as u64 != entry.bytes || hash != entry.blake3 {
        return invalid(format!("manifest verification failed for {:?}", entry.path));
    }
    Ok(bytes)
}
