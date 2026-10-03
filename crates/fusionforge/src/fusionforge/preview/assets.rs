
pub(super) fn normalized_preview_asset_ref_name(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}
