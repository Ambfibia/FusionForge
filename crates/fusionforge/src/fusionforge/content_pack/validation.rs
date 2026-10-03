use super::*;

pub(super) enum ObjectCookError {
    Object(String),
    Output(String),
}

pub(super) fn validate_source_relative(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() {
        return Err(format!("path must be non-empty and relative: {value}"));
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(format!("path contains an unsafe component: {value}"));
        }
    }
    Ok(())
}

pub(super) fn validate_source_filename(value: &str) -> Result<(), String> {
    validate_source_relative(value)?;
    if Path::new(value).components().count() != 1 {
        return Err(format!("bundle inventory key is not a filename: {value}"));
    }
    Ok(())
}

pub(super) fn validate_pack_relative_path(value: &str) -> Result<(), String> {
    if !value.is_ascii() || value.contains('\\') || contains_legacy_text(value) {
        return Err(format!("unsafe native pack path: {value}"));
    }
    validate_source_relative(value)
}

pub(super) fn reject_forbidden_json(value: &JsonValue) -> Result<(), String> {
    match value {
        JsonValue::String(value) if contains_legacy_text(value) => {
            Err("native JSON contains a legacy locator".to_string())
        }
        JsonValue::Array(values) => {
            for value in values {
                reject_forbidden_json(value)?;
            }
            Ok(())
        }
        JsonValue::Object(object) => {
            for (key, value) in object {
                let folded = key
                    .chars()
                    .filter(|value| value.is_ascii_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                if matches!(
                    folded.as_str(),
                    "pathid" | "fileid" | "assetindex" | "sourceasset" | "sourcebundle"
                ) {
                    return Err(format!("native JSON contains forbidden field {key}"));
                }
                reject_forbidden_json(value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn validate_sha256(value: &str) -> Result<(), String> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!(
            "invalid SHA-256 digest in launcher manifest: {value}"
        ))
    }
}

pub(super) fn validate_uuid(value: &str) -> Result<(), String> {
    if is_uuid(value) {
        Ok(())
    } else {
        Err(format!("invalid launcher manifest UUID: {value}"))
    }
}

pub(super) fn require_manifest_format(value: &JsonValue, expected: &str, label: &str) -> Result<(), String> {
    let actual = value
        .get("format")
        .and_then(JsonValue::as_str)
        .unwrap_or("<missing>");
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{label} manifest format mismatch: expected {expected}, got {actual}"
        ))
    }
}

pub(super) fn require_entries<'a>(value: &'a JsonValue, label: &str) -> Result<&'a [JsonValue], String> {
    value
        .get("entries")
        .and_then(JsonValue::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{label} manifest has no entries array"))
}

pub(super) struct ErrorRoots<'a> {
    pub(super) project: &'a Path,
    pub(super) build: &'a Path,
    pub(super) work: &'a Path,
}

impl ErrorRoots<'_> {
    pub(super) fn sanitize(&self, value: &str) -> String {
        let mut result = value.to_string();
        for (path, replacement) in [
            (self.project, "<project>"),
            (self.build, "<build>"),
            (self.work, "<work>"),
        ] {
            let path = path.to_string_lossy();
            result = result.replace(path.as_ref(), replacement);
            result = result.replace(&path.replace('\\', "/"), replacement);
        }
        result
    }
}
