//! Value-preserving helpers used by conversion and validation paths.
use std::{fs, io::{BufReader, Read}, path::Path, time::{SystemTime, UNIX_EPOCH}};
use serde_json::{Map as JsonMap, Value as JsonValue};
use crate::{Result, error::io_at};

pub(crate) fn hash_file(path: &Path) -> Result<String> {
    let file = fs::File::open(path).map_err(|source| io_at(path, source))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|source| io_at(path, source))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub(crate) fn canonical_json(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let mut canonical = JsonMap::new();
            for key in keys {
                canonical.insert(key.clone(), canonical_json(&object[key]));
            }
            JsonValue::Object(canonical)
        }
        JsonValue::Array(array) => {
            JsonValue::Array(array.iter().map(canonical_json).collect::<Vec<_>>())
        }
        _ => value.clone(),
    }
}

pub(crate) fn transaction_stamp() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{nanos}", std::process::id())
}

pub(crate) fn normalize_route(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
        .to_ascii_lowercase()
}

pub(crate) fn has_generated_identity(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let hash_suffix = lower.rsplit_once("--").is_some_and(|(_, suffix)| {
        suffix.len() >= 8 && suffix.chars().all(|value| value.is_ascii_hexdigit())
    });
    let path_id = lower.find("pathid").is_some_and(|offset| {
        lower[offset + "pathid".len()..]
            .trim_start_matches(['-', '_', '#', ' '])
            .chars()
            .next()
            .is_some_and(|value| value.is_ascii_digit())
    });
    let generated_object = ["mesh#", "transform#", "texture#", "texture2d#", "material#"]
        .iter()
        .any(|prefix| {
            lower.strip_prefix(prefix).is_some_and(|suffix| {
                !suffix.is_empty() && suffix.chars().all(|value| value.is_ascii_digit())
            })
        });
    hash_suffix || path_id || generated_object
}

pub(crate) fn texture_format_name(format: i32) -> Option<&'static str> {
    match format {
        1 => Some("Alpha8"),
        2 => Some("ARGB4444"),
        3 => Some("RGB24"),
        4 => Some("RGBA32"),
        5 => Some("ARGB32"),
        7 => Some("RGB565"),
        10 => Some("DXT1"),
        11 => Some("DXT3"),
        12 => Some("DXT5"),
        13 => Some("RGBA4444"),
        14 => Some("BGRA32"),
        _ => None,
    }
}
