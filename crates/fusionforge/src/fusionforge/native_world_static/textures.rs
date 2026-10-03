use super::*;

#[derive(Debug, Clone)]
pub(super) struct TexturePublication {
    pub(super) files_by_id: BTreeMap<String, String>,
    pub(super) document: JsonValue,
    pub(super) mip_count: usize,
}

pub(super) fn validate_png(bytes: &[u8], texture_id: &str, level: u32) -> Result<(), String> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 33 || bytes.get(..8) != Some(PNG_SIGNATURE.as_slice()) {
        return Err(format!(
            "texture {texture_id:?} mip {level} is not a complete PNG"
        ));
    }
    Ok(())
}
