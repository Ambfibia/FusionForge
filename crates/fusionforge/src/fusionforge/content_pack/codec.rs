use super::*;

pub(super) fn validate_native_payload(extension: &str, bytes: &[u8]) -> Result<(), String> {
    if extension.eq_ignore_ascii_case("ogg") && !is_valid_ogg(bytes) {
        return Err("native audio payload is not a valid Ogg stream".to_string());
    }
    if extension.eq_ignore_ascii_case("png") && !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("native texture payload is not a PNG".to_string());
    }
    if matches!(extension.to_ascii_lowercase().as_str(), "ttf" | "otf")
        && font_extension(bytes) != Some(&*extension.to_ascii_lowercase())
    {
        return Err("native font extension does not match its payload".to_string());
    }
    if extension.eq_ignore_ascii_case("json") {
        let value: JsonValue = serde_json::from_slice(bytes)
            .map_err(|err| format!("generated native JSON is invalid: {err}"))?;
        reject_forbidden_json(&value)?;
    }
    Ok(())
}
