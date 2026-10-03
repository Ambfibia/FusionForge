use std::path::PathBuf;

use serde_json::Value;

use crate::{ContentError, Result};

const LEGACY_SUFFIXES: &[(&str, &str)] = &[
    (".unity3d", "Unity AssetBundle extension"),
    (".resourcefile", "Unity resourceFile extension"),
    (".assets", "Unity serialized assets extension"),
    (".ffclient", "legacy FFClient project extension"),
    (".assetbundle", "Unity AssetBundle extension"),
    (".bundle", "Unity AssetBundle extension"),
    (".ress", "Unity serialized resource companion"),
    (".nif", "legacy Gamebryo mesh extension"),
    (".kfm", "legacy Gamebryo animation-set extension"),
    (".kf", "legacy Gamebryo animation extension"),
    (".cg", "legacy Cg shader extension"),
    (".obj", "intermediate OBJ authoring extension"),
];

pub(crate) fn validate_content_path(path: &str) -> Result<()> {
    if path.is_empty() {
        return Err(unsafe_path(path, "path is empty"));
    }
    if path.starts_with('/') {
        return Err(unsafe_path(path, "absolute paths are forbidden"));
    }
    if !path.bytes().all(|byte| (b' '..=b'~').contains(&byte)) {
        return Err(unsafe_path(
            path,
            "paths must contain printable ASCII characters only",
        ));
    }
    if path.contains('\\') {
        return Err(unsafe_path(path, "only forward slashes are permitted"));
    }
    if path.contains(':') {
        return Err(unsafe_path(
            path,
            "drive, URI, and alternate-stream syntax is forbidden",
        ));
    }
    if path
        .bytes()
        .any(|byte| matches!(byte, b'<' | b'>' | b'"' | b'|' | b'?' | b'*'))
    {
        return Err(unsafe_path(
            path,
            "Windows-invalid filename characters are forbidden",
        ));
    }

    for component in path.split('/') {
        if component.is_empty() {
            return Err(unsafe_path(path, "empty path components are forbidden"));
        }
        if component == "." || component == ".." {
            return Err(unsafe_path(path, "dot path components are forbidden"));
        }
        if component.ends_with('.') || component.ends_with(' ') {
            return Err(unsafe_path(
                path,
                "ambiguous Windows path components are forbidden",
            ));
        }
        reject_windows_reserved_component(path, component)?;
        reject_legacy_component(path, component)?;
    }

    Ok(())
}

pub(crate) fn normalize_content_path(path: &str) -> Result<String> {
    validate_content_path(path)?;
    Ok(path.to_ascii_lowercase())
}

pub(crate) fn reject_legacy_magic(path: &str, prefix: &[u8]) -> Result<()> {
    let signature = if prefix.starts_with(b"UnityFS") {
        Some("UnityFS AssetBundle signature")
    } else if prefix.starts_with(b"UnityRaw") {
        Some("UnityRaw AssetBundle signature")
    } else if prefix.starts_with(b"UnityWeb") {
        Some("UnityWeb AssetBundle signature")
    } else if prefix.starts_with(b"Gamebryo File Format") {
        Some("legacy Gamebryo file signature")
    } else if prefix.starts_with(b"NetImmerse File Format") {
        Some("legacy NetImmerse file signature")
    } else {
        None
    };

    if let Some(reason) = signature {
        return Err(ContentError::ForbiddenLegacy {
            path: path.to_owned(),
            reason,
        });
    }
    Ok(())
}

pub(crate) fn reject_legacy_json(path: &str, bytes: &[u8]) -> Result<()> {
    let value: Value = serde_json::from_slice(bytes).map_err(|source| ContentError::Json {
        path: PathBuf::from(path),
        source,
    })?;
    inspect_json_value(path, None, &value)
}

fn inspect_json_value(path: &str, parent_key: Option<&str>, value: &Value) -> Result<()> {
    match value {
        Value::String(value) => {
            if contains_legacy_locator(value) {
                return Err(ContentError::ForbiddenLegacy {
                    path: path.to_owned(),
                    reason: "legacy locator in native JSON",
                });
            }
            if parent_key.is_some_and(is_path_like_key) && looks_like_absolute_locator(value) {
                return Err(ContentError::ForbiddenLegacy {
                    path: path.to_owned(),
                    reason: "absolute authoring path in native JSON",
                });
            }
            Ok(())
        }
        Value::Array(values) => {
            for value in values {
                inspect_json_value(path, parent_key, value)?;
            }
            Ok(())
        }
        Value::Object(object) => {
            for (key, value) in object {
                let normalized = normalized_json_key(key);
                if matches!(
                    normalized.as_str(),
                    "pathid"
                        | "fileid"
                        | "assetindex"
                        | "sourceasset"
                        | "sourcebundle"
                        | "unitytype"
                ) {
                    return Err(ContentError::ForbiddenLegacy {
                        path: path.to_owned(),
                        reason: "legacy authoring field in native JSON",
                    });
                }
                inspect_json_value(path, Some(&normalized), value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn normalized_json_key(key: &str) -> String {
    key.chars()
        .filter(|value| value.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_path_like_key(key: &str) -> bool {
    key == "path"
        || key == "file"
        || key == "url"
        || key == "directory"
        || key == "root"
        || key.ends_with("path")
        || key.ends_with("file")
        || key.ends_with("url")
        || key.ends_with("directory")
        || key.ends_with("root")
}

fn contains_legacy_locator(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("assetbundle://")
        || lower.contains("archive:/")
        || lower.contains("file:///")
        || lower.starts_with("\\\\?\\")
    {
        return true;
    }

    LEGACY_SUFFIXES
        .iter()
        .any(|(suffix, _)| contains_filename_extension(&lower, suffix))
}

fn contains_filename_extension(value: &str, extension: &str) -> bool {
    value.match_indices(extension).any(|(index, _)| {
        let before = value[..index].bytes().next_back();
        let after = value[index + extension.len()..].bytes().next();
        (index == 0 || before.is_some_and(is_filename_byte))
            && after.is_none_or(|byte| !byte.is_ascii_alphanumeric())
    })
}

fn is_filename_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn looks_like_absolute_locator(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.starts_with('/')
        || value.starts_with("\\\\")
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'\\' | b'/'))
}

fn reject_legacy_component(path: &str, component: &str) -> Result<()> {
    let lower = component.to_ascii_lowercase();
    for (suffix, reason) in LEGACY_SUFFIXES {
        if lower.ends_with(suffix) {
            return Err(ContentError::ForbiddenLegacy {
                path: path.to_owned(),
                reason,
            });
        }
    }

    let reason = if lower == "assetbundle" || lower == "assetbundles" {
        Some("Unity AssetBundle name")
    } else if lower == "globalgamemanagers" || lower == "maindata" {
        Some("Unity serialized player-data name")
    } else if lower.starts_with("cab-") {
        Some("Unity AssetBundle CAB name")
    } else if lower.strip_prefix("level").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        Some("Unity serialized level name")
    } else {
        None
    };

    if let Some(reason) = reason {
        return Err(ContentError::ForbiddenLegacy {
            path: path.to_owned(),
            reason,
        });
    }
    Ok(())
}

fn reject_windows_reserved_component(path: &str, component: &str) -> Result<()> {
    let lower = component.to_ascii_lowercase();
    let stem = lower
        .split_once('.')
        .map_or(lower.as_str(), |(stem, _)| stem)
        .trim_end_matches(' ');
    let numbered_device = stem.len() == 4
        && matches!(&stem[..3], "com" | "lpt")
        && matches!(stem.as_bytes()[3], b'1'..=b'9');
    let reserved = matches!(
        stem,
        "con" | "prn" | "aux" | "nul" | "clock$" | "conin$" | "conout$"
    ) || numbered_device;

    if reserved {
        return Err(unsafe_path(
            path,
            "Windows reserved device names are forbidden",
        ));
    }
    Ok(())
}

fn unsafe_path(path: &str, reason: &'static str) -> ContentError {
    ContentError::UnsafePath {
        path: path.to_owned(),
        reason,
    }
}
