use super::*;

pub(super) fn exact_named_pairs(value: Option<&UnityValue>, field: &str) -> Result<Vec<JsonValue>, String> {
    value_array(value)
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let (name, value) = pair_name_value(entry)
                .ok_or_else(|| format!("{field}[{index}] is not a named pair"))?;
            Ok(json!({
                "slot": index,
                "name": name,
                "value": unity_to_lossless_json(value),
            }))
        })
        .collect()
}

pub(super) fn resolved_pointer_key(
    env: &UnityEnvironment,
    value: Option<&UnityValue>,
    label: &str,
    required: bool,
) -> Result<Option<(usize, i64)>, String> {
    let Some(value) = value else {
        return if required {
            Err(format!("{label} is missing"))
        } else {
            Ok(None)
        };
    };
    let pointer = match value {
        UnityValue::Pointer(pointer) => pointer,
        _ => return Err(format!("{label} is not a PPtr")),
    };
    if pointer.is_null() {
        return if required {
            Err(format!("{label} is null"))
        } else {
            Ok(None)
        };
    }
    strict_non_null_pointer(env, pointer)
        .map(|key| Some((key.asset, key.path_id)))
        .map_err(|error| {
            format!(
                "{label} ({}/{}) could not be resolved: {error}",
                pointer.file_id, pointer.path_id
            )
        })
}

pub(super) fn strict_non_null_pointer(env: &UnityEnvironment, pointer: &Pointer) -> Result<ObjectKey, String> {
    env.resolve_pointer_strict(pointer)?
        .ok_or_else(|| "null pointer".to_string())
}

pub(super) fn pointer_json(pointer: &Pointer) -> JsonValue {
    json!({
        "sourceAssetIndex": pointer.source_asset,
        "fileId": pointer.file_id,
        "pathId": pointer.path_id,
        "isNull": pointer.is_null(),
    })
}

pub(super) fn required_u32(
    value: &UnityValue,
    field: &str,
    key: ObjectKey,
    env: &UnityEnvironment,
) -> Result<u32, String> {
    let value = value
        .get(field)
        .and_then(UnityValue::as_i64)
        .ok_or_else(|| format!("{} has no {field}", object_id(env, key)))?;
    u32::try_from(value).map_err(|_| format!("{}.{field} is negative", object_id(env, key)))
}

pub(super) fn color_hex(value: &JsonValue) -> Option<String> {
    let object = value.as_object()?;
    let channel = |name: &str| {
        object
            .get(name)
            .and_then(JsonValue::as_f64)
            .map(|value| (value * 255.0).round().clamp(0.0, 255.0) as u8)
    };
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        channel("r")?,
        channel("g")?,
        channel("b")?
    ))
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
