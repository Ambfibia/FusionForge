use super::*;

pub(super) fn verify_table_data_icon_source(
    manifest: &ProjectAssetManifest,
    route: &VerifiedIconRoute,
    bytes: &[u8],
) -> Result<()> {
    let source_entry = manifest
        .files
        .iter()
        .find(|entry| entry.path == route.source)
        .ok_or_else(|| {
            invalid_error(format!(
                "source manifest does not contain exact TableData icon asset {:?}",
                route.source
            ))
        })?;
    if source_entry.kind != ProjectAssetKind::Texture
        || source_entry.blake3 != route.expected_blake3
    {
        return invalid(format!(
            "source manifest identity mismatch for {:?}: kind={:?}, blake3={}",
            route.source, source_entry.kind, source_entry.blake3
        ));
    }
    let actual_blake3 = blake3::hash(bytes).to_hex().to_string();
    if actual_blake3 != route.expected_blake3 || source_entry.bytes != bytes.len() as u64 {
        return invalid(format!(
            "exact TableData icon byte identity mismatch for {:?}: expected blake3={} bytes={}, actual blake3={} bytes={}",
            route.source,
            route.expected_blake3,
            source_entry.bytes,
            actual_blake3,
            bytes.len()
        ));
    }
    Ok(())
}

pub(super) fn invalid<T>(reason: impl Into<String>) -> Result<T> {
    Err(invalid_error(reason))
}
