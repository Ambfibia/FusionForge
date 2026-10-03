use super::*;

pub(super) fn unity_text(value: &UnityValue) -> Option<String> {
    value
        .as_bytes()
        .map(|bytes| String::from_utf8_lossy(bytes).to_string())
        .or_else(|| value.as_str().map(str::to_string))
}

pub(super) fn unity_bytes(value: &UnityValue) -> Option<Vec<u8>> {
    value
        .as_bytes()
        .map(Vec::from)
        .or_else(|| value.as_str().map(|value| value.as_bytes().to_vec()))
}

pub(super) fn unity_to_lossless_json(value: &UnityValue) -> JsonValue {
    match value {
        UnityValue::Bool(value) => json!(value),
        UnityValue::Int(value) => json!(value),
        UnityValue::UInt(value) => json!(value),
        UnityValue::Float(value) => json!(value),
        UnityValue::String(value) => json!(value),
        UnityValue::Bytes(value) => json!({
            "$bytes": {
                "encoding": "base64",
                "byteLength": value.len(),
                "sha256": sha256_hex(value),
                "data": STANDARD.encode(value),
            }
        }),
        UnityValue::Array(value) => {
            JsonValue::Array(value.iter().map(unity_to_lossless_json).collect())
        }
        UnityValue::Object(value) => JsonValue::Object(
            value
                .iter()
                .map(|(name, value)| (name.clone(), unity_to_lossless_json(value)))
                .collect(),
        ),
        UnityValue::Pair(left, right) => json!({
            "$pair": [unity_to_lossless_json(left), unity_to_lossless_json(right)]
        }),
        UnityValue::Pointer(pointer) => pointer_json(pointer),
    }
}

pub(super) fn ensure_object_type(
    env: &UnityEnvironment,
    key: ObjectKey,
    expected: &str,
    role: &str,
) -> Result<(), String> {
    let asset = env
        .assets
        .get(key.asset)
        .ok_or_else(|| format!("{role} asset index {} is missing", key.asset))?;
    let info = asset
        .objects
        .get(&key.path_id)
        .ok_or_else(|| format!("{role} {}#{} is missing", asset.name, key.path_id))?;
    let actual = exact_object_type_name(asset, info);
    if actual != expected {
        return Err(format!(
            "{role} {}#{} has type {}, expected {}",
            asset.name, key.path_id, actual, expected
        ));
    }
    Ok(())
}

/// Unity 3.x stores both a type-tree id and the serialized class id.  Most
/// assets use the same numeric value for both, but a few imported wearables
/// retain a type-tree id that names another built-in class.  The class id is
/// authoritative for the object payload (for example classId 21 carries a
/// complete Material even when typeId 20 happens to spell "Camera").
pub(super) fn exact_object_type_name(asset: &Asset, info: &ObjectInfo) -> String {
    match info.class_id {
        21 => "Material".to_string(),
        // Retro_shared textures retain typeId 27 (Texture) but classId 28.
        28 => "Texture2D".to_string(),
        _ => asset.object_type_name(info),
    }
}

pub(super) fn source_object_json(env: &UnityEnvironment, key: ObjectKey) -> JsonValue {
    let Some(asset) = env.assets.get(key.asset) else {
        return json!({
            "id": format!("missing:{}:{}", key.asset, key.path_id),
            "assetIndex": key.asset,
            "pathId": key.path_id,
        });
    };
    let (object_type, serialized_type_name, class_id, type_id) = asset
        .objects
        .get(&key.path_id)
        .map(|info| {
            (
                exact_object_type_name(asset, info),
                asset.object_type_name(info),
                info.class_id,
                info.type_id,
            )
        })
        .unwrap_or_else(|| ("Missing".to_string(), "Missing".to_string(), 0, 0));
    json!({
        "id": object_id(env, key),
        "assetIndex": key.asset,
        "asset": asset.name,
        "pathId": key.path_id,
        "type": object_type,
        "serializedTypeName": serialized_type_name,
        "classId": class_id,
        "typeId": type_id,
    })
}

pub(super) fn source_object_json_from_info(
    env: &UnityEnvironment,
    asset_index: usize,
    info: &crate::fusionforge::ObjectInfo,
) -> JsonValue {
    let Some(asset) = env.assets.get(asset_index) else {
        return json!({
            "id": format!("missing:{}:{}", asset_index, info.path_id),
            "assetIndex": asset_index,
            "pathId": info.path_id,
        });
    };
    json!({
        "id": format!("{}:{}", asset.name, info.path_id),
        "assetIndex": asset_index,
        "asset": asset.name,
        "pathId": info.path_id,
        "type": asset.object_type_name(info),
        "classId": info.class_id,
        "typeId": info.type_id,
    })
}

pub(super) fn object_id(env: &UnityEnvironment, key: ObjectKey) -> String {
    format!("{}:{}", env.asset_name(key.asset), key.path_id)
}
