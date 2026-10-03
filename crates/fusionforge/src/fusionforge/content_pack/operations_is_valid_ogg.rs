use super::*;

pub(super) fn looks_like_posix_absolute(value: &str) -> bool {
    value.starts_with('/') && value[1..].contains('/')
}

pub(super) fn semantic_safe(value: &str) -> String {
    let mut result = String::with_capacity(value.len().min(80));
    let mut last_separator = false;
    for byte in value.bytes() {
        if result.len() >= 72 {
            break;
        }
        let next = if byte.is_ascii_alphanumeric() {
            (byte as char).to_ascii_lowercase()
        } else if matches!(byte, b'-' | b'_') {
            byte as char
        } else {
            '_'
        };
        if next == '_' {
            if last_separator {
                continue;
            }
            last_separator = true;
        } else {
            last_separator = false;
        }
        result.push(next);
    }
    let result = result.trim_matches(['_', '-']).to_string();
    if result.is_empty() {
        "asset".to_string()
    } else {
        result
    }
}

pub(super) fn neutral_semantic_label(value: &str, fallback: &str) -> String {
    let repaired = repair_cp1251_mojibake(value).unwrap_or_else(|| value.to_string());
    let cleaned = repaired
        .chars()
        .map(|value| if value.is_control() { ' ' } else { value })
        .collect::<String>();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() || contains_legacy_text(&cleaned) {
        fallback.to_string()
    } else {
        cleaned
    }
}

pub(super) fn semantic_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

pub(super) fn content_kind_name(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::Table => "table",
        ContentKind::World => "world",
        ContentKind::Mesh => "mesh",
        ContentKind::Material => "material",
        ContentKind::Texture => "texture",
        ContentKind::Animation => "animation",
        ContentKind::Audio => "audio",
        ContentKind::Font => "font",
        ContentKind::Localization => "localization",
        ContentKind::Shader => "shader",
    }
}

pub(super) fn font_extension(bytes: &[u8]) -> Option<&'static str> {
    match bytes.get(..4)? {
        b"OTTO" => Some("otf"),
        [0, 1, 0, 0] | b"true" | b"typ1" | b"ttcf" => Some("ttf"),
        _ => None,
    }
}

pub(super) fn is_valid_ogg(bytes: &[u8]) -> bool {
    let mut offset = 0usize;
    let mut pages = 0usize;
    let mut payload_bytes = 0usize;
    while offset < bytes.len() {
        let Some(header) = bytes.get(offset..offset.saturating_add(27)) else {
            return false;
        };
        if &header[..4] != b"OggS" || header[4] != 0 || header[5] & !0x07 != 0 {
            return false;
        }
        let segment_count = header[26] as usize;
        let Some(lacing) = bytes.get(offset + 27..offset + 27 + segment_count) else {
            return false;
        };
        let payload_size = lacing.iter().map(|value| *value as usize).sum::<usize>();
        let Some(next) = (offset + 27 + segment_count).checked_add(payload_size) else {
            return false;
        };
        if next > bytes.len() || next <= offset {
            return false;
        }
        let page = &bytes[offset..next];
        let expected_crc = u32::from_le_bytes([page[22], page[23], page[24], page[25]]);
        if ogg_page_crc(page) != expected_crc {
            return false;
        }
        payload_bytes += payload_size;
        offset = next;
        pages += 1;
    }
    pages > 0 && payload_bytes > 0 && offset == bytes.len()
}

pub(super) fn ogg_page_crc(page: &[u8]) -> u32 {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table = [0_u32; 256];
        for (index, value) in table.iter_mut().enumerate() {
            let mut crc = (index as u32) << 24;
            for _ in 0..8 {
                crc = if crc & 0x8000_0000 != 0 {
                    (crc << 1) ^ 0x04c1_1db7
                } else {
                    crc << 1
                };
            }
            *value = crc;
        }
        table
    });
    let mut crc = 0_u32;
    for (index, byte) in page.iter().copied().enumerate() {
        let byte = if (22..26).contains(&index) { 0 } else { byte };
        let lookup = ((crc >> 24) as u8 ^ byte) as usize;
        crc = (crc << 8) ^ table[lookup];
    }
    crc
}

pub(super) fn stable_key(domain: &str, parts: &[&[u8]]) -> String {
    let mut hasher = Blake3Hasher::new();
    hasher.update(domain.as_bytes());
    for part in parts {
        hasher.update(&[0]);
        hasher.update(part);
    }
    hasher.finalize().to_hex().to_string()
}

pub(super) fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn is_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|index| bytes[*index] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_hexdigit())
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|err| format!("{label} {}: {err}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "{label} is not a regular directory: {}",
            path.display()
        ));
    }
    fs::canonicalize(path).map_err(|err| format!("{label} {}: {err}", path.display()))
}

pub(super) fn required_string<'a>(value: &'a JsonValue, key: &str, label: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{label} has no non-empty {key}"))
}

pub(super) fn json_bytes(value: &JsonValue) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec(value).map_err(|err| err.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn pretty_json_bytes(value: &JsonValue) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn cleanup_created(path: &Path) {
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else if path.is_file() {
        let _ = fs::remove_file(path);
    }
}
