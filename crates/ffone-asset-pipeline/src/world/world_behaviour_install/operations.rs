use super::*;

/// Script classes that are fully represented by an already published asset and
/// therefore need no runtime record of their own.
pub(super) fn superseded_script(class_name: &str) -> Option<&'static str> {
    match class_name {
        "Combiner" | "CombineChildren" => Some("static batching is baked into the published GLBs"),
        "TerrainDetailManager" => Some("terrain detail is published by the terrain exporter"),
        "MapAttributeTable" => Some("map attributes are published as terrain gameplay attributes"),
        "DongColorSetup" => {
            Some("tile fog, light and sky colour are published as environment.json")
        }
        _ => None,
    }
}

pub(super) fn optional_json_array<'a>(value: Option<&'a JsonValue>, label: &str) -> Result<&'a [JsonValue]> {
    match value {
        None => Ok(&[]),
        Some(JsonValue::Array(values)) => Ok(values),
        Some(_) => invalid(format!("{label} is not an array")),
    }
}

pub(super) fn required_curve_keys<'a>(
    curve: &'a JsonValue,
    clip_id: &str,
    label: &str,
) -> Result<&'a [JsonValue]> {
    optional_json_array(
        curve.get("m_Curve"),
        &format!("AnimationClip {clip_id} {label} keys"),
    )
}

pub(super) fn finite_json(value: Option<&JsonValue>, label: &str) -> Result<f64> {
    let value = value
        .and_then(JsonValue::as_f64)
        .ok_or_else(|| invalid_error(format!("{label} is not numeric")))?;
    if !value.is_finite() {
        return invalid(format!("{label} is not finite"));
    }
    Ok(value)
}

pub(super) fn json_vec3(value: Option<&JsonValue>, label: &str) -> Result<[f64; 3]> {
    let value = value.ok_or_else(|| invalid_error(format!("{label} is absent")))?;
    Ok([
        finite_json(value.get("x"), label)?,
        finite_json(value.get("y"), label)?,
        finite_json(value.get("z"), label)?,
    ])
}

pub(super) fn json_quaternion(value: Option<&JsonValue>, label: &str) -> Result<[f64; 4]> {
    let value = value.ok_or_else(|| invalid_error(format!("{label} is absent")))?;
    Ok([
        finite_json(value.get("x"), label)?,
        finite_json(value.get("y"), label)?,
        finite_json(value.get("z"), label)?,
        finite_json(value.get("w"), label)?,
    ])
}

pub(super) fn push_strict_time(times: &mut Vec<f64>, time: f64, clip_id: &str, label: &str) -> Result<()> {
    if times.last().is_some_and(|previous| *previous >= time) {
        return invalid(format!(
            "AnimationClip {clip_id} {label} key times are not strict"
        ));
    }
    times.push(time);
    Ok(())
}

pub(super) fn usize_json(value: Option<&JsonValue>, label: &str) -> Result<usize> {
    value
        .and_then(JsonValue::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| invalid_error(format!("{label} is not a non-negative usize")))
}

pub(super) fn blocker(node: &str, id: &str, code: &str, message: &str) -> JsonValue {
    serde_json::json!({
        "node": node,
        "component": id,
        "code": code,
        "message": message,
    })
}

pub(super) fn matrix_json(matrix: &[[f64; 4]; 4]) -> JsonValue {
    JsonValue::Array(
        matrix
            .iter()
            .map(|row| {
                JsonValue::Array(
                    row.iter()
                        .map(|value| JsonValue::from(*value))
                        .collect::<Vec<_>>(),
                )
            })
            .collect(),
    )
}

pub(super) fn vector_json(value: Option<&JsonValue>) -> JsonValue {
    let component = |key: &str| {
        value
            .and_then(|value| value.get(key))
            .and_then(JsonValue::as_f64)
            .unwrap_or(0.0)
    };
    JsonValue::Array(vec![
        JsonValue::from(component("x")),
        JsonValue::from(component("y")),
        JsonValue::from(component("z")),
    ])
}

pub(super) fn number_json(value: Option<&JsonValue>) -> JsonValue {
    match value {
        Some(JsonValue::Number(number)) => JsonValue::Number(number.clone()),
        Some(JsonValue::Bool(flag)) => JsonValue::from(u8::from(*flag)),
        _ => JsonValue::from(0),
    }
}

pub(super) fn bool_value(value: Option<&JsonValue>) -> Option<bool> {
    value.and_then(|value| {
        value
            .as_bool()
            .or_else(|| value.as_i64().map(|value| value != 0))
            .or_else(|| value.as_u64().map(|value| value != 0))
    })
}

pub(super) fn hierarchy_vector(value: Option<&JsonValue>, length: usize, label: &str) -> Result<Vec<f64>> {
    let values = value
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("{label} is not an array")))?;
    if values.len() != length {
        return invalid(format!(
            "{label} has {} entries, expected {length}",
            values.len()
        ));
    }
    values
        .iter()
        .map(|value| finite_json(Some(value), label))
        .collect()
}

pub(super) fn hierarchy_matrix(value: &JsonValue, label: &str) -> Result<[[f64; 4]; 4]> {
    let rows = value
        .as_array()
        .ok_or_else(|| invalid_error(format!("{label} world matrix is not an array")))?;
    if rows.len() != 4 {
        return invalid(format!("{label} world matrix has {} rows", rows.len()));
    }
    let mut matrix = [[0.0; 4]; 4];
    for (row_index, row) in rows.iter().enumerate() {
        let values = hierarchy_vector(Some(row), 4, label)?;
        matrix[row_index].copy_from_slice(&values);
    }
    Ok(matrix)
}

pub(super) fn models_in_node_subtrees(
    parent_by_node: &BTreeMap<String, String>,
    direct_models: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    let mut result = BTreeMap::<String, Vec<String>>::new();
    for (node, models) in direct_models {
        let mut current = Some(node.as_str());
        let mut visited = BTreeSet::<String>::new();
        while let Some(owner) = current {
            if !visited.insert(owner.to_owned()) {
                break;
            }
            result
                .entry(owner.to_owned())
                .or_default()
                .extend(models.iter().cloned());
            current = parent_by_node.get(owner).map(String::as_str);
        }
    }
    for models in result.values_mut() {
        models.sort();
        models.dedup();
    }
    result
}

pub(super) fn matches_owned_bytes(bytes: &[u8], expected_len: u64, expected_blake3: &str) -> bool {
    if bytes.len() as u64 == expected_len && hash_bytes(bytes) == expected_blake3 {
        return true;
    }
    // Git may materialize checked JSON with CRLF on Windows even though the
    // install proof owns the canonical LF bytes. Accept only that reversible
    // text transform; lone CR bytes or any semantic drift still fail closed.
    let mut canonical = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' {
            if bytes.get(index + 1) != Some(&b'\n') {
                return false;
            }
            canonical.push(b'\n');
            index += 2;
        } else {
            canonical.push(bytes[index]);
            index += 1;
        }
    }
    canonical.len() as u64 == expected_len && hash_bytes(&canonical) == expected_blake3
}

pub(super) fn source_set_blake3(tiles: &[WorldBehaviourTileProof]) -> String {
    let mut hasher = blake3::Hasher::new();
    for tile in tiles {
        hasher.update(tile.tile_id.as_bytes());
        hasher.update(&[0]);
        hasher.update(tile.export_blake3.as_bytes());
        hasher.update(&[0]);
        hasher.update(tile.source_archive_blake3.as_bytes());
        hasher.update(&[0]);
    }
    hasher.finalize().to_hex().to_string()
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_dir() {
        return invalid(format!("{label} is not a directory: {}", path.display()));
    }
    fs::canonicalize(path).map_err(|error| io_at(path, error))
}

pub(super) fn pretty_json(value: &JsonValue) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(json_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub(super) fn unique_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
