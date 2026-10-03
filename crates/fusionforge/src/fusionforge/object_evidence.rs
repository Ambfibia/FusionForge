use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};

use super::unity::{object_name, Asset, ObjectInfo, Pointer, UnityEnvironment, UnityValue};

pub(crate) struct ObjectEvidenceRequest<'a> {
    pub source_alias: &'a str,
    pub relative_container: &'a str,
    pub container_bytes: &'a [u8],
    pub serialized_asset: Option<&'a str>,
    pub expected_type: Option<&'a str>,
    pub path_id: i64,
    pub allow_unresolved_pointers: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceSource<'a> {
    alias: &'a str,
    relative_container: &'a str,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceObject {
    serialized_asset: String,
    #[serde(rename = "type")]
    unity_type: String,
    path_id: i64,
    type_id: i32,
    class_id: i32,
    name: String,
    raw_bytes: u64,
    raw_sha256: String,
    value: JsonValue,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExternalFileEvidence {
    file_id: usize,
    asset_path: String,
    file_path: String,
    guid: String,
    type_id: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ObjectIdentityEvidence {
    serialized_asset: String,
    #[serde(rename = "type")]
    unity_type: String,
    path_id: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PointerEvidence {
    field_path: String,
    file_id: i32,
    path_id: i64,
    status: &'static str,
    target: Option<ObjectIdentityEvidence>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TriageEvidence {
    allow_unresolved_pointers: bool,
    unresolved_pointer_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ObjectEvidenceDocument<'a> {
    schema: &'static str,
    source: EvidenceSource<'a>,
    object: EvidenceObject,
    external_files: Vec<ExternalFileEvidence>,
    pointers: Vec<PointerEvidence>,
    triage: TriageEvidence,
}

fn sha256_hex(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn guid_hex(guid: &[u8; 16]) -> String {
    guid.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn select_object<'a>(
    env: &'a UnityEnvironment,
    serialized_asset: Option<&str>,
    path_id: i64,
) -> Result<(usize, &'a Asset, &'a ObjectInfo), String> {
    if let Some(serialized_asset) = serialized_asset {
        let matching_assets = env
            .assets
            .iter()
            .enumerate()
            .filter(|(_, asset)| asset.name == serialized_asset)
            .collect::<Vec<_>>();
        let (asset_index, asset) = match matching_assets.as_slice() {
            [] => {
                let available = env
                    .assets
                    .iter()
                    .map(|asset| asset.name.as_str())
                    .collect::<Vec<_>>();
                return Err(format!(
                    "serialized asset '{serialized_asset}' was not found; available: {}",
                    available.join(", ")
                ));
            }
            [(asset_index, asset)] => (*asset_index, *asset),
            _ => {
                return Err(format!(
                    "serialized asset name '{serialized_asset}' is duplicated; evidence identity is ambiguous"
                ));
            }
        };
        let info = asset.objects.get(&path_id).ok_or_else(|| {
            format!("pathID {path_id} was not found in exact serialized asset {serialized_asset}")
        })?;
        return Ok((asset_index, asset, info));
    }

    let matches = env
        .assets
        .iter()
        .enumerate()
        .filter_map(|(asset_index, asset)| {
            asset
                .objects
                .get(&path_id)
                .map(|info| (asset_index, asset, info))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(format!("pathID {path_id} was not found")),
        [(asset_index, asset, info)] => Ok((*asset_index, *asset, *info)),
        _ => {
            let candidates = matches
                .iter()
                .map(|(_, asset, info)| format!("{}:{}", asset.name, asset.object_type_name(info)))
                .collect::<Vec<_>>();
            Err(format!(
                "pathID {path_id} is ambiguous across serialized assets: {}; pass --serialized-asset <exact-name>",
                candidates.join(", ")
            ))
        }
    }
}

fn escape_json_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn collect_pointers(value: &UnityValue, path: &str, output: &mut Vec<(String, Pointer)>) {
    match value {
        UnityValue::Pointer(pointer) => {
            output.push((
                if path.is_empty() {
                    "/".to_string()
                } else {
                    path.to_string()
                },
                pointer.clone(),
            ));
        }
        UnityValue::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                collect_pointers(value, &format!("{path}/{index}"), output);
            }
        }
        UnityValue::Object(values) => {
            for (name, value) in values {
                collect_pointers(
                    value,
                    &format!("{path}/{}", escape_json_pointer(name)),
                    output,
                );
            }
        }
        UnityValue::Pair(left, right) => {
            collect_pointers(left, &format!("{path}/0"), output);
            collect_pointers(right, &format!("{path}/1"), output);
        }
        _ => {}
    }
}

fn unity_to_json(value: &UnityValue) -> JsonValue {
    match value {
        UnityValue::Bool(value) => json!(value),
        UnityValue::Int(value) => json!(value),
        UnityValue::UInt(value) => json!(value),
        UnityValue::Float(value) => json!(value),
        UnityValue::String(value) => json!(value),
        UnityValue::Bytes(value) => json!({
            "bytes": value.len(),
            "base64": base64::engine::general_purpose::STANDARD.encode(value),
        }),
        UnityValue::Array(values) => JsonValue::Array(values.iter().map(unity_to_json).collect()),
        UnityValue::Object(values) => JsonValue::Object(
            values
                .iter()
                .map(|(name, value)| (name.clone(), unity_to_json(value)))
                .collect(),
        ),
        UnityValue::Pair(left, right) => {
            JsonValue::Array(vec![unity_to_json(left), unity_to_json(right)])
        }
        UnityValue::Pointer(pointer) => {
            json!({ "fileId": pointer.file_id, "pathId": pointer.path_id })
        }
    }
}

pub(crate) fn build_object_evidence(
    env: &UnityEnvironment,
    request: ObjectEvidenceRequest<'_>,
) -> Result<JsonValue, String> {
    let (asset_index, asset, info) = select_object(env, request.serialized_asset, request.path_id)?;
    let unity_type = asset.object_type_name(info);
    if let Some(expected_type) = request.expected_type {
        if unity_type != expected_type {
            return Err(format!(
                "{} pathID {} has Unity type '{}', expected exact type '{}'",
                asset.name, request.path_id, unity_type, expected_type
            ));
        }
    }

    let body = asset.read_object(asset_index, info).map_err(|error| {
        format!(
            "could not read {}:{} pathID {}: {error}",
            asset.name, unity_type, request.path_id
        )
    })?;
    let raw_object = asset.object_raw_data(info)?;

    let external_files = asset
        .asset_refs
        .iter()
        .enumerate()
        .skip(1)
        .map(|(file_id, asset_ref)| ExternalFileEvidence {
            file_id,
            asset_path: asset_ref.asset_path.clone(),
            file_path: asset_ref.file_path.clone(),
            guid: guid_hex(&asset_ref.guid),
            type_id: asset_ref.type_id,
        })
        .collect::<Vec<_>>();

    let mut raw_pointers = Vec::<(String, Pointer)>::new();
    collect_pointers(&body, "", &mut raw_pointers);
    let mut pointers = Vec::<PointerEvidence>::with_capacity(raw_pointers.len());
    let mut unresolved_pointer_count = 0usize;
    for (field_path, pointer) in raw_pointers {
        if pointer.source_asset != asset_index {
            return Err(format!(
                "PPtr at {field_path} claims source asset {}, but owning serialized asset index is {asset_index}",
                pointer.source_asset
            ));
        }
        if pointer.is_null() {
            pointers.push(PointerEvidence {
                field_path,
                file_id: pointer.file_id,
                path_id: pointer.path_id,
                status: "null",
                target: None,
                error: None,
            });
            continue;
        }

        match env.resolve_pointer_strict(&pointer) {
            Ok(Some(key)) => {
                let target_asset = env.assets.get(key.asset).ok_or_else(|| {
                    format!("strict PPtr target asset index {} disappeared", key.asset)
                })?;
                let target_info = target_asset.objects.get(&key.path_id).ok_or_else(|| {
                    format!(
                        "strict PPtr target {} pathID {} disappeared",
                        target_asset.name, key.path_id
                    )
                })?;
                pointers.push(PointerEvidence {
                    field_path,
                    file_id: pointer.file_id,
                    path_id: pointer.path_id,
                    status: "resolved",
                    target: Some(ObjectIdentityEvidence {
                        serialized_asset: target_asset.name.clone(),
                        unity_type: target_asset.object_type_name(target_info),
                        path_id: key.path_id,
                    }),
                    error: None,
                });
            }
            Ok(None) => {
                return Err(format!(
                    "non-null PPtr at {field_path} unexpectedly resolved as null"
                ));
            }
            Err(error) if request.allow_unresolved_pointers => {
                unresolved_pointer_count += 1;
                pointers.push(PointerEvidence {
                    field_path,
                    file_id: pointer.file_id,
                    path_id: pointer.path_id,
                    status: "unresolved",
                    target: None,
                    error: Some(error),
                });
            }
            Err(error) => {
                return Err(format!(
                    "unresolved PPtr at {field_path} (fileID {}, pathID {}): {error}; use --allow-unresolved-pointers only for explicit triage",
                    pointer.file_id, pointer.path_id
                ));
            }
        }
    }

    let document = ObjectEvidenceDocument {
        schema: "fusionforge.object-evidence.v1",
        source: EvidenceSource {
            alias: request.source_alias,
            relative_container: request.relative_container,
            bytes: request.container_bytes.len() as u64,
            sha256: sha256_hex(request.container_bytes),
        },
        object: EvidenceObject {
            serialized_asset: asset.name.clone(),
            unity_type,
            path_id: request.path_id,
            type_id: info.type_id,
            class_id: info.class_id,
            name: object_name(&body),
            raw_bytes: raw_object.len() as u64,
            raw_sha256: sha256_hex(raw_object),
            value: unity_to_json(&body),
        },
        external_files,
        pointers,
        triage: TriageEvidence {
            allow_unresolved_pointers: request.allow_unresolved_pointers,
            unresolved_pointer_count,
        },
    };
    serde_json::to_value(document).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
