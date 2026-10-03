use super::*;

#[derive(Debug)]
pub(super) struct ExactSceneObject {
    pub(super) key: ObjectKey,
    pub(super) asset_name: String,
    pub(super) class_id: i32,
    pub(super) type_id: i32,
    pub(super) type_name: String,
    pub(super) body: UnityValue,
    pub(super) raw: Vec<u8>,
    pub(super) script_pointer: Option<Pointer>,
    pub(super) script_true_name: Option<String>,
}

pub(super) fn exact_scene_object_from_body(
    asset_index: usize,
    asset: &super::super::unity::Asset,
    info: &super::super::unity::ObjectInfo,
    body: UnityValue,
    script_pointer: Option<Pointer>,
    script_true_name: Option<String>,
) -> Result<ExactSceneObject, String> {
    let raw = asset.object_raw_data(info).map_err(|err| {
        format!(
            "could not read raw object {}#{}: {err}",
            asset.name, info.path_id
        )
    })?;
    Ok(ExactSceneObject {
        key: ObjectKey {
            asset: asset_index,
            path_id: info.path_id,
        },
        asset_name: asset.name.clone(),
        class_id: info.class_id,
        type_id: info.type_id,
        type_name: asset.object_type_name(info).to_string(),
        body,
        raw: raw.to_vec(),
        script_pointer,
        script_true_name,
    })
}

pub(super) fn exact_scene_object_candidates(
    env: &UnityEnvironment,
    objects: &[ExactSceneObject],
    stem: &str,
    sidecars: &mut Vec<SceneSidecar>,
) -> Result<Vec<JsonValue>, String> {
    objects
        .iter()
        .enumerate()
        .map(|(index, object)| {
            let candidate_stem = if objects.len() == 1 {
                stem.to_string()
            } else {
                format!("{stem}-candidate-{index:02}")
            };
            exact_scene_object_document(env, object, &candidate_stem, sidecars)
        })
        .collect()
}

pub(super) fn exact_scene_object_document(
    env: &UnityEnvironment,
    object: &ExactSceneObject,
    stem: &str,
    sidecars: &mut Vec<SceneSidecar>,
) -> Result<JsonValue, String> {
    let raw_path = format!("environment/source/{stem}.raw.bin");
    let parsed_path = format!("environment/source/{stem}.parsed.json");
    let mut parsed = serde_json::to_vec_pretty(&unity_to_json(&object.body))
        .map_err(|err| format!("could not encode {}: {err}", object.key.path_id))?;
    parsed.push(b'\n');
    sidecars.push(SceneSidecar {
        relative_path: raw_path.clone(),
        bytes: object.raw.clone(),
    });
    sidecars.push(SceneSidecar {
        relative_path: parsed_path.clone(),
        bytes: parsed.clone(),
    });
    let game_object = object
        .body
        .get("m_GameObject")
        .and_then(UnityValue::as_pointer)
        .map(|pointer| resolved_game_object_evidence(env, pointer))
        .transpose()?;
    Ok(json!({
        "assetName": object.asset_name,
        "pathId": object.key.path_id,
        "classId": object.class_id,
        "typeId": object.type_id,
        "typeName": object.type_name,
        "scriptPointer": object.script_pointer.as_ref().map(pointer_json),
        "scriptTrueName": object.script_true_name,
        "gameObject": game_object,
        "rawData": {
            "path": raw_path,
            "byteLength": object.raw.len(),
            "blake3": hash_bytes(&object.raw),
        },
        "parsedData": {
            "path": parsed_path,
            "blake3": hash_bytes(&parsed),
        },
    }))
}

pub(super) fn resolved_game_object_evidence(
    env: &UnityEnvironment,
    pointer: &Pointer,
) -> Result<JsonValue, String> {
    let key = env
        .resolve_pointer(pointer)
        .map_err(|err| format!("could not resolve m_GameObject pointer: {err}"))?;
    let body = env.read_object(key).map_err(|err| {
        format!(
            "could not read m_GameObject asset {} pathId {}: {err}",
            key.asset, key.path_id
        )
    })?;
    Ok(json!({
        "pointer": pointer_json(pointer),
        "resolvedAssetName": env.assets[key.asset].name,
        "resolvedPathId": key.path_id,
        "trueName": object_name(&body),
    }))
}

pub(super) fn raw_object_blake3(env: &UnityEnvironment, key: ObjectKey) -> Result<String, String> {
    let asset = env
        .assets
        .get(key.asset)
        .ok_or_else(|| format!("asset index {} is missing", key.asset))?;
    let info = asset
        .objects
        .get(&key.path_id)
        .ok_or_else(|| format!("{}#{} is missing", asset.name, key.path_id))?;
    Ok(hash_bytes(asset.object_raw_data(info)?))
}

pub(super) fn resolved_object_name(env: &UnityEnvironment, pointer: &Pointer) -> Option<String> {
    let key = env.resolve_pointer(pointer).ok()?;
    let body = env.read_object(key).ok()?;
    Some(object_name(&body))
}

pub(super) fn unity_to_json(value: &UnityValue) -> JsonValue {
    match value {
        UnityValue::Bool(value) => json!(value),
        UnityValue::Int(value) => json!(value),
        UnityValue::UInt(value) => json!(value),
        UnityValue::Float(value) => json!(value),
        UnityValue::String(value) => json!(value),
        UnityValue::Bytes(value) => json!({
            "base64": BASE64_STANDARD.encode(value),
        }),
        UnityValue::Array(values) => JsonValue::Array(values.iter().map(unity_to_json).collect()),
        UnityValue::Object(values) => JsonValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), unity_to_json(value)))
                .collect(),
        ),
        UnityValue::Pair(left, right) => json!([unity_to_json(left), unity_to_json(right)]),
        UnityValue::Pointer(pointer) => pointer_json(pointer),
    }
}

pub(super) fn unity_rotation_to_native(value: [f64; 4]) -> [f64; 4] {
    [value[0], -value[1], -value[2], value[3]]
}

pub(super) fn replace_json_object_field(path: &Path, field: &str, value: JsonValue) -> Result<(), String> {
    let bytes =
        fs::read(path).map_err(|err| format!("could not read {}: {err}", path.display()))?;
    let mut document: JsonValue = serde_json::from_slice(&bytes)
        .map_err(|err| format!("could not parse {}: {err}", path.display()))?;
    let object = document
        .as_object_mut()
        .ok_or_else(|| format!("{} root is not an object", path.display()))?;
    if object.insert(field.to_string(), value).is_some() {
        return Err(format!(
            "{} already contains {field}; refusing enrichment overwrite",
            path.display()
        ));
    }
    replace_json_hardlink_safe(path, &document)
}
