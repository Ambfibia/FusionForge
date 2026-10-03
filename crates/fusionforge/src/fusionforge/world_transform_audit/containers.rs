use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyBundleProof {
    pub path: String,
    pub blake3: String,
    pub extracted_assets: Vec<String>,
}

pub(super) fn game_object_name(env: &UnityEnvironment, body: &UnityValue) -> String {
    let Some(pointer) = body.get("m_GameObject").and_then(UnityValue::as_pointer) else {
        return String::new();
    };
    let Ok(key) = env.resolve_pointer(pointer) else {
        return String::new();
    };
    env.read_object(key)
        .ok()
        .map(|value| object_name(&value))
        .unwrap_or_default()
}

pub(super) fn dependency_bundle_proof(side: &JsonValue) -> Option<(String, DependencyBundleProof)> {
    let path = side.get("path")?.as_str()?.to_string();
    let mut extracted_assets = side
        .pointer("/buildtool/extractedFiles")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(|file| file.get("name").and_then(JsonValue::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    extracted_assets.sort();
    extracted_assets.dedup();
    let blake3 = blake3_file(Path::new(&path)).unwrap_or_default();
    let key = normalize_path(Path::new(&path));
    Some((
        key,
        DependencyBundleProof {
            path: readable_absolute_path(Path::new(&path)),
            blake3,
            extracted_assets,
        },
    ))
}

pub(super) fn is_map_bundle(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("unity3d"))
        && path
            .file_stem()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.starts_with("Map_"))
}
