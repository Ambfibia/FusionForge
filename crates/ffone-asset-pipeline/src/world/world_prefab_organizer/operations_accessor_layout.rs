use super::*;

pub(super) fn accessor_layout(document: &JsonValue, index: usize, path: &str) -> Result<AccessorLayout> {
    let accessor = document
        .get("accessors")
        .and_then(JsonValue::as_array)
        .and_then(|accessors| accessors.get(index))
        .ok_or_else(|| invalid_error(format!("{path:?} accessor {index} is absent")))?;
    if accessor.get("componentType").and_then(JsonValue::as_u64) != Some(GLTF_FLOAT)
        || accessor.get("type").and_then(JsonValue::as_str) != Some("VEC3")
        || accessor.get("sparse").is_some()
    {
        return invalid(format!("{path:?} accessor {index} is not dense FLOAT VEC3"));
    }
    let view_index = required_u64(accessor.get("bufferView"), "bufferView", path)? as usize;
    let view = document
        .get("bufferViews")
        .and_then(JsonValue::as_array)
        .and_then(|views| views.get(view_index))
        .ok_or_else(|| invalid_error(format!("{path:?} buffer view is absent")))?;
    if view.get("buffer").and_then(JsonValue::as_u64) != Some(0) {
        return invalid(format!("{path:?} accessor does not use GLB buffer 0"));
    }
    let offset = view
        .get("byteOffset")
        .and_then(JsonValue::as_u64)
        .unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(JsonValue::as_u64)
            .unwrap_or(0) as usize;
    let count = required_u64(accessor.get("count"), "count", path)? as usize;
    let stride = view
        .get("byteStride")
        .and_then(JsonValue::as_u64)
        .unwrap_or(12) as usize;
    if stride < 12 {
        return invalid(format!("{path:?} accessor has a short stride"));
    }
    Ok(AccessorLayout {
        offset,
        count,
        stride,
    })
}

pub(super) fn dmat4(rows: [[f64; 4]; 4]) -> DMat4 {
    DMat4::from_cols(
        DVec4::new(rows[0][0], rows[1][0], rows[2][0], rows[3][0]),
        DVec4::new(rows[0][1], rows[1][1], rows[2][1], rows[3][1]),
        DVec4::new(rows[0][2], rows[1][2], rows[2][2], rows[3][2]),
        DVec4::new(rows[0][3], rows[1][3], rows[2][3], rows[3][3]),
    )
}

pub(super) fn dvec3_f32(value: DVec3, path: &str) -> Result<[f32; 3]> {
    let converted = [value.x as f32, value.y as f32, value.z as f32];
    if !value.is_finite() || converted.iter().any(|value| !value.is_finite()) {
        return invalid(format!("{path:?} local geometry exceeds finite f32"));
    }
    Ok(converted)
}

pub(super) fn identity_matrix() -> [[f64; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub(super) fn identity_string_matrix() -> [[String; 4]; 4] {
    string_matrix(identity_matrix())
}

pub(super) fn string_matrix(matrix: [[f64; 4]; 4]) -> [[String; 4]; 4] {
    matrix.map(|row| row.map(format_f64))
}

pub(super) fn format_f64(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else {
        value.to_string()
    }
}

pub(super) fn matrix_close(left: [[f64; 4]; 4], right: [[f64; 4]; 4]) -> bool {
    left.iter()
        .flatten()
        .zip(right.iter().flatten())
        .all(|(left, right)| {
            let scale = left.abs().max(right.abs()).max(1.0);
            (left - right).abs() <= scale * 1.0e-10
        })
}

pub(super) fn pretty_json(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(generated_json_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let path = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    if !path.is_dir() {
        return invalid(format!("{label} is not a directory: {}", path.display()));
    }
    Ok(path)
}

pub(super) fn absolute_from(project_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

pub(super) fn safe_join(root: &Path, relative: impl AsRef<Path>) -> Result<PathBuf> {
    validate_relative_path(relative.as_ref())?;
    Ok(root.join(relative))
}

pub(super) fn safe_slug(value: &str, fallback: &str, maximum: usize) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('_');
            }
            slug.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
        if slug.len() >= maximum {
            break;
        }
    }
    while slug.ends_with('_') {
        slug.pop();
    }
    if slug.is_empty() {
        fallback.to_owned()
    } else {
        slug
    }
}

pub(super) fn safe_identifier(value: &str, fallback: &str, maximum: usize) -> String {
    let identifier = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(maximum)
        .collect::<String>();
    if identifier.is_empty() {
        fallback.to_owned()
    } else {
        identifier
    }
}

pub(super) fn append_set_hash(hasher: &mut blake3::Hasher, path: &str, hash: &str) {
    hasher.update(path.as_bytes());
    hasher.update(&[0]);
    hasher.update(hash.as_bytes());
    hasher.update(&[0xff]);
}

pub(super) fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub(super) fn short_hash(bytes: &[u8]) -> String {
    hash_bytes(bytes)[..12].to_owned()
}

pub(super) fn required_u64(value: Option<&JsonValue>, field: &str, path: &str) -> Result<u64> {
    value
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid_error(format!("{path:?} field {field:?} is not u64")))
}

pub(super) fn le_u32(bytes: &[u8], offset: usize, path: &str) -> Result<u32> {
    bytes
        .get(offset..offset + 4)
        .map(|value| u32::from_le_bytes(value.try_into().expect("fixed slice")))
        .ok_or_else(|| invalid_error(format!("{path:?} truncated u32 at {offset}")))
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
