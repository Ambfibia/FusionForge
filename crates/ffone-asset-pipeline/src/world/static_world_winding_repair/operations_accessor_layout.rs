use super::*;

pub(super) fn accessor_layout(parsed: &ParsedGlb, index: usize, path: &str) -> Result<AccessorLayout> {
    let accessors = required_array(&parsed.document, "accessors", path)?;
    let accessor = accessors
        .get(index)
        .ok_or_else(|| invalid_error(format!("{path:?} accessor {index} is absent")))?;
    if accessor.get("sparse").is_some()
        || accessor.get("normalized") == Some(&JsonValue::Bool(true))
    {
        return invalid(format!(
            "{path:?} accessor {index} uses unsupported sparse/normalized data"
        ));
    }
    let view_index = required_u64(accessor.get("bufferView"), "bufferView", path)? as usize;
    let views = required_array(&parsed.document, "bufferViews", path)?;
    let view = views
        .get(view_index)
        .ok_or_else(|| invalid_error(format!("{path:?} bufferView {view_index} is absent")))?;
    if view.get("buffer").and_then(JsonValue::as_u64) != Some(0) {
        return invalid(format!(
            "{path:?} bufferView {view_index} does not use GLB buffer 0"
        ));
    }
    let component_type = required_u64(accessor.get("componentType"), "componentType", path)?;
    let component_width = component_width(component_type).ok_or_else(|| {
        invalid_error(format!(
            "{path:?} accessor {index} has unsupported component type"
        ))
    })?;
    let element_components = match accessor.get("type").and_then(JsonValue::as_str) {
        Some("SCALAR") => 1,
        Some("VEC3") => 3,
        other => {
            return invalid(format!(
                "{path:?} accessor {index} has unsupported type {other:?}"
            ));
        }
    };
    let count = required_u64(accessor.get("count"), "count", path)? as usize;
    let element_width = component_width * element_components;
    let stride = view
        .get("byteStride")
        .and_then(JsonValue::as_u64)
        .map(|value| value as usize)
        .unwrap_or(element_width);
    if stride < element_width {
        return invalid(format!("{path:?} accessor {index} has a short byteStride"));
    }
    let view_offset = view
        .get("byteOffset")
        .and_then(JsonValue::as_u64)
        .unwrap_or(0) as usize;
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(JsonValue::as_u64)
        .unwrap_or(0) as usize;
    let relative = view_offset
        .checked_add(accessor_offset)
        .ok_or_else(|| invalid_error(format!("{path:?} accessor offset overflow")))?;
    let end = if count == 0 {
        relative
    } else {
        relative
            .checked_add((count - 1).saturating_mul(stride))
            .and_then(|value| value.checked_add(element_width))
            .ok_or_else(|| invalid_error(format!("{path:?} accessor range overflow")))?
    };
    if end > parsed.binary_len {
        return invalid(format!("{path:?} accessor {index} exceeds BIN chunk"));
    }
    Ok(AccessorLayout {
        offset: parsed.binary_start + relative,
        count,
        stride,
        component_type,
        element_components,
    })
}

pub(super) fn component_width(component_type: u64) -> Option<usize> {
    match component_type {
        GLTF_UNSIGNED_BYTE => Some(1),
        GLTF_UNSIGNED_SHORT => Some(2),
        GLTF_UNSIGNED_INT | GLTF_FLOAT => Some(4),
        _ => None,
    }
}

pub(super) fn required_array<'a>(value: &'a JsonValue, field: &str, path: &str) -> Result<&'a [JsonValue]> {
    value
        .get(field)
        .and_then(JsonValue::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid_error(format!("{path:?} field {field:?} is not an array")))
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

pub(super) fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

pub(super) fn static_fields_blake3(scene: &JsonValue) -> Result<String> {
    let object = scene
        .as_object()
        .ok_or_else(|| invalid_error("scene static fields require an object"))?;
    let mut fields = JsonMap::new();
    for key in ["coverage", "models", "visuals", "colliders"] {
        fields.insert(
            key.to_owned(),
            object
                .get(key)
                .ok_or_else(|| invalid_error(format!("scene lacks {key:?}")))?
                .clone(),
        );
    }
    let bytes = serde_json::to_vec(&canonical_json(&JsonValue::Object(fields)))
        .map_err(generated_json_error)?;
    Ok(hash_bytes(&bytes))
}

pub(super) fn initialize_backup_root(backup_root: &Path, project_root: &Path) -> Result<()> {
    fs::create_dir_all(backup_root).map_err(|error| io_at(backup_root, error))?;
    let marker = backup_root.join("marker.json");
    let expected = pretty_json(&serde_json::json!({
        "schema": "ffone.static-world-winding-repair-backup.v1",
        "projectRoot": path_text(project_root),
        "tool": STATIC_WORLD_WINDING_REPAIR_TOOL,
    }))?;
    if marker.exists() {
        let actual = read_regular_file(&marker, "repair backup marker")?;
        if actual != expected {
            return invalid("existing repair backup belongs to another operation");
        }
    } else {
        write_new(&marker, &expected)?;
    }
    Ok(())
}

pub(super) fn backup_and_replace(
    project_root: &Path,
    backup_root: &Path,
    target: &Path,
    before: &[u8],
    after: &[u8],
) -> Result<()> {
    if before == after {
        return Ok(());
    }
    let relative = target.strip_prefix(project_root).map_err(|_| {
        invalid_error(format!(
            "replacement target escaped project: {}",
            target.display()
        ))
    })?;
    let backup = backup_root.join(relative);
    if backup.exists() {
        let existing = read_regular_file(&backup, "repair backup")?;
        if existing != before {
            // A resumed operation sees the repaired target but retains its
            // exact source backup. Accept that source form explicitly.
            if hash_bytes(&existing) == hash_bytes(after) {
                return invalid(format!(
                    "repair backup unexpectedly contains result bytes at {}",
                    backup.display()
                ));
            }
        }
    } else {
        write_new(&backup, before)?;
    }
    let temporary = target.with_extension(format!(
        "{}.winding-repair-tmp",
        target
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("bin")
    ));
    if temporary.exists() {
        return invalid(format!(
            "stale winding repair temporary exists at {}",
            temporary.display()
        ));
    }
    write_new(&temporary, after)?;
    if hash_bytes(&read_regular_file(&temporary, "repair temporary")?) != hash_bytes(after) {
        return invalid(format!(
            "repair temporary failed verification at {}",
            temporary.display()
        ));
    }
    fs::remove_file(target).map_err(|error| io_at(target, error))?;
    if let Err(error) = fs::rename(&temporary, target) {
        let _ = fs::copy(&backup, target);
        return Err(io_at(target, error));
    }
    Ok(())
}

pub(super) fn append_set_hash(hasher: &mut blake3::Hasher, path: &str, hash: &str) {
    hasher.update(&(path.len() as u64).to_le_bytes());
    hasher.update(path.as_bytes());
    hasher.update(&(hash.len() as u64).to_le_bytes());
    hasher.update(hash.as_bytes());
}

pub(super) fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative.is_empty() || relative.contains('\\') || relative.contains(':') {
        return invalid(format!("unsafe relative path {relative:?}"));
    }
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("unsafe relative path {relative:?}"));
    }
    let joined = root.join(path);
    if !joined.starts_with(root) {
        return invalid(format!("relative path escaped root: {relative:?}"));
    }
    Ok(joined)
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return invalid(format!("{label} must be a non-symlink directory"));
    }
    fs::canonicalize(path).map_err(|error| io_at(path, error))
}

pub(super) fn pretty_json(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(generated_json_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
