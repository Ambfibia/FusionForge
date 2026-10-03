use super::*;

pub(super) const RUNTIME_WORLD_SCHEMA: &str = "ffone.runtime-world.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StaticWorldWindingRepairMode {
    Plan,
    Apply,
}

pub(super) fn rewrite_runtime_world(
    bytes: &[u8],
    path: &Path,
    scene_hashes: &BTreeMap<String, String>,
) -> Result<Vec<u8>> {
    let mut value: JsonValue = parse_json(bytes, path)?;
    if value.get("schema").and_then(JsonValue::as_str) != Some(RUNTIME_WORLD_SCHEMA) {
        return invalid("runtime world registry has the wrong schema");
    }
    let entries = value
        .get_mut("entries")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("runtime world registry has no entries"))?;
    let mut remaining = scene_hashes.keys().cloned().collect::<BTreeSet<_>>();
    for entry in entries {
        let Some(scene) = entry.get_mut("scene").and_then(JsonValue::as_object_mut) else {
            continue;
        };
        let Some(scene_path) = scene
            .get("path")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        let Some(hash) = scene_hashes.get(&scene_path) else {
            continue;
        };
        if !remaining.remove(&scene_path) {
            return invalid(format!(
                "runtime world registry duplicates repaired scene {scene_path:?}"
            ));
        }
        scene.insert("blake3".to_owned(), JsonValue::String(hash.clone()));
    }
    if !remaining.is_empty() {
        return invalid(format!(
            "runtime world registry misses {} repaired scenes; first={:?}",
            remaining.len(),
            remaining.first()
        ));
    }
    pretty_json(&value)
}
