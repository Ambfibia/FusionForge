use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceBundle {
    pub(super) path: String,
    pub(super) blake3: String,
}

pub(super) fn unity_value_to_json(value: &UnityValue) -> JsonValue {
    match value {
        UnityValue::Bool(value) => json!(value),
        UnityValue::Int(value) => json!(value),
        UnityValue::UInt(value) => json!(value),
        UnityValue::Float(value) => json!(value),
        UnityValue::String(value) => json!(value),
        UnityValue::Bytes(value) => json!({ "base64": BASE64_STANDARD.encode(value) }),
        UnityValue::Array(values) => {
            JsonValue::Array(values.iter().map(unity_value_to_json).collect())
        }
        UnityValue::Object(values) => JsonValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), unity_value_to_json(value)))
                .collect(),
        ),
        UnityValue::Pair(left, right) => {
            json!([unity_value_to_json(left), unity_value_to_json(right)])
        }
        UnityValue::Pointer(pointer) => pointer_json(pointer),
    }
}

pub(super) fn resolved_object_raw_blake3(env: &UnityEnvironment, key: ObjectKey) -> Result<String, String> {
    let asset = env
        .assets
        .get(key.asset)
        .ok_or_else(|| format!("resolved object asset index {} is missing", key.asset))?;
    let info = asset
        .objects
        .get(&key.path_id)
        .ok_or_else(|| format!("resolved object {}#{} is missing", asset.name, key.path_id))?;
    let raw = asset.object_raw_data(info).map_err(|err| {
        format!(
            "could not read resolved object {}#{} raw bytes: {err}",
            asset.name, key.path_id
        )
    })?;
    Ok(hash_bytes(raw))
}

pub(super) fn required_object<'a>(
    value: &'a UnityValue,
    key: &str,
    label: &str,
) -> Result<&'a BTreeMap<String, UnityValue>, String> {
    value
        .get(key)
        .and_then(UnityValue::as_object)
        .ok_or_else(|| format!("{label}: missing object {key}"))
}

pub(super) fn canonicalize_unity_heights(width: u32, height: u32, source: &[u16]) -> Result<Vec<u16>, String> {
    let width = width as usize;
    let height = height as usize;
    let expected = width
        .checked_mul(height)
        .ok_or_else(|| "height sample count overflows usize".to_string())?;
    if source.len() != expected {
        return Err(format!(
            "height source has {} samples, expected {expected}",
            source.len()
        ));
    }
    let mut canonical = Vec::with_capacity(expected);
    for z in 0..height {
        for x in 0..width {
            canonical.push(source[x * height + z]);
        }
    }
    Ok(canonical)
}
