use super::*;

pub(super) fn retry_transient_rename<R, D>(mut rename: R, mut delay: D) -> std::io::Result<()>
where
    R: FnMut() -> std::io::Result<()>,
    D: FnMut(usize),
{
    for attempt in 1..=ATOMIC_RENAME_ATTEMPTS {
        match rename() {
            Ok(()) => return Ok(()),
            Err(error)
                if attempt < ATOMIC_RENAME_ATTEMPTS
                    && is_transient_windows_rename_error(&error) =>
            {
                delay(attempt);
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("bounded rename loop always returns")
}

pub(super) fn required_nonnegative_usize(
    object: &BTreeMap<String, UnityValue>,
    key: &str,
    label: &str,
) -> Result<usize, String> {
    object
        .get(key)
        .and_then(exact_i64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("{label}: {key} must be a nonnegative usize"))
}

pub(super) fn exact_byte_array(value: Option<&UnityValue>, label: &str) -> Result<Vec<u8>, String> {
    match value {
        Some(UnityValue::Bytes(bytes)) => Ok(bytes.clone()),
        Some(UnityValue::Array(values)) => values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                exact_i64(value)
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| format!("{label}[{index}] is not a u8"))
            })
            .collect(),
        _ => Err(format!("{label} is not a byte payload")),
    }
}

pub(super) fn graph_closure_is_blocked(value: &JsonValue) -> bool {
    value.get("status").and_then(JsonValue::as_str) == Some("rootObjectExportedGraphClosureBlocked")
}

pub(super) fn pointer_json(pointer: &Pointer) -> JsonValue {
    json!({
        "fileId": pointer.file_id,
        "pathId": pointer.path_id,
    })
}

pub(super) fn pointer_provenance(pointer: &Pointer, resolved: &ResolvedTexture) -> JsonValue {
    json!({
        "pointerFileId": pointer.file_id,
        "pointerPathId": pointer.path_id,
        "resolvedAssetName": resolved.asset_name,
        "resolvedPathId": resolved.key.path_id,
    })
}

pub(super) fn required_dimension(
    object: &BTreeMap<String, UnityValue>,
    key: &str,
    label: &str,
) -> Result<u32, String> {
    let value = object
        .get(key)
        .and_then(exact_i64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{label}: {key} must be a positive u32"))?;
    Ok(value)
}

pub(super) fn exact_i64(value: &UnityValue) -> Option<i64> {
    match value {
        UnityValue::Int(value) => Some(*value),
        UnityValue::UInt(value) => i64::try_from(*value).ok(),
        _ => None,
    }
}

pub(super) fn exact_u16_array(
    value: Option<&UnityValue>,
    expected_len: usize,
    label: &str,
) -> Result<Vec<u16>, String> {
    let values = value_array(value);
    if values.len() != expected_len {
        return Err(format!(
            "{label}: expected {expected_len} samples, found {}",
            values.len()
        ));
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let integer = exact_i64(value)
                .ok_or_else(|| format!("{label}[{index}] is not an exact integer"))?;
            u16::try_from(integer).map_err(|_| format!("{label}[{index}]={integer} is outside u16"))
        })
        .collect()
}

pub(super) fn required_vec3(value: Option<&UnityValue>, label: &str) -> Result<[f64; 3], String> {
    let object = value
        .and_then(UnityValue::as_object)
        .ok_or_else(|| format!("{label} is not an object"))?;
    Ok([
        required_number(object, "x", label)?,
        required_number(object, "y", label)?,
        required_number(object, "z", label)?,
    ])
}

pub(super) fn required_vec2(value: Option<&UnityValue>, label: &str) -> Result<[f64; 2], String> {
    let object = value
        .and_then(UnityValue::as_object)
        .ok_or_else(|| format!("{label} is not an object"))?;
    Ok([
        required_number(object, "x", label)?,
        required_number(object, "y", label)?,
    ])
}

pub(super) fn required_number(
    object: &BTreeMap<String, UnityValue>,
    key: &str,
    label: &str,
) -> Result<f64, String> {
    object
        .get(key)
        .and_then(UnityValue::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{label}.{key} must be finite"))
}

pub(super) fn required_pointer<'a>(value: &'a UnityValue, label: &str) -> Result<&'a Pointer, String> {
    value
        .as_pointer()
        .ok_or_else(|| format!("{label} must be a non-null pointer"))
}

pub(super) fn required_true_name(
    body: &UnityValue,
    object_type: &str,
    path_id: i64,
) -> Result<String, String> {
    let name = object_name(body);
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        return Err(format!(
            "{object_type} pathId {path_id} has no safe true m_Name"
        ));
    }
    Ok(name)
}

pub(super) fn semantic_component(value: &str) -> Result<String, String> {
    if value == "." || value == ".." || value.len() > 180 {
        return Err(format!("unsafe semantic asset name: {value:?}"));
    }
    if value
        .chars()
        .any(|character| character.is_control() || r#"<>:"/\|?*"#.contains(character))
        || value.ends_with(['.', ' '])
    {
        return Err(format!("unsafe semantic asset name: {value:?}"));
    }
    let reserved = value
        .split('.')
        .next()
        .unwrap_or(value)
        .trim_end()
        .to_ascii_lowercase();
    if matches!(
        reserved.as_str(),
        "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "com2"
            | "com3"
            | "com4"
            | "com5"
            | "com6"
            | "com7"
            | "com8"
            | "com9"
            | "lpt1"
            | "lpt2"
            | "lpt3"
            | "lpt4"
            | "lpt5"
            | "lpt6"
            | "lpt7"
            | "lpt8"
            | "lpt9"
    ) {
        return Err(format!("reserved semantic asset name: {value:?}"));
    }
    Ok(value.to_string())
}

pub(super) fn register_semantic_name(
    names: &mut BTreeMap<String, String>,
    component: &str,
    true_name: &str,
) -> Result<(), String> {
    let folded = component.to_lowercase();
    if let Some(existing) = names.get(&folded) {
        if existing != true_name {
            return Err(format!(
                "case-insensitive semantic name collision: {existing:?} and {true_name:?}"
            ));
        }
    } else {
        names.insert(folded, true_name.to_string());
    }
    Ok(())
}

pub(super) fn weight_channel(layer_index: usize) -> WeightChannel {
    const CHANNELS: [&str; 4] = ["r", "g", "b", "a"];
    WeightChannel {
        map_index: layer_index / 4,
        channel_index: layer_index % 4,
        channel: CHANNELS[layer_index % 4],
    }
}

pub(super) fn u16_le_bytes(values: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 2);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub(super) fn hash_bytes(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}
