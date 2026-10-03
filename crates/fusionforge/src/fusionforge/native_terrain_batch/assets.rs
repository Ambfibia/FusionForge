use super::*;

pub(super) fn tile_session_path(work_root: &Path, scope: TerrainScope, tile_id: &str) -> PathBuf {
    work_root
        .join(scope.label())
        .join(format!("tile_{tile_id}"))
}

pub(super) fn path_from_forward_slashes(value: &str) -> PathBuf {
    value.split('/').collect()
}

pub(super) fn replace_manifest_blockers(
    manifest: &mut JsonValue,
    enrichment_blockers: Vec<BatchBlocked>,
) -> Result<(), String> {
    let new_values = enrichment_blockers
        .into_iter()
        .map(|blocker| {
            serde_json::to_value(blocker)
                .map_err(|err| format!("could not serialize enrichment blocker: {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let replacement_scene_keys = new_values
        .iter()
        .filter(|value| value.get("stage").and_then(JsonValue::as_str) == Some("scene-link"))
        .filter_map(|value| {
            Some((
                value.get("scope")?.as_str()?.to_string(),
                value.get("tileId")?.as_str()?.to_string(),
            ))
        })
        .collect::<BTreeSet<_>>();
    let existing = manifest
        .get_mut("blocked")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| "source batch manifest has no blocked array".to_string())?;
    existing.retain(|value| {
        if value.get("code").and_then(JsonValue::as_str) != Some("terrainColliderLinkage") {
            return true;
        }
        let key = value
            .get("scope")
            .and_then(JsonValue::as_str)
            .zip(value.get("tileId").and_then(JsonValue::as_str))
            .map(|(scope, tile)| (scope.to_string(), tile.to_string()));
        !key.is_some_and(|key| replacement_scene_keys.contains(&key))
    });
    existing.extend(new_values);
    existing.sort_by(|left, right| {
        blocker_sort_key(left)
            .cmp(&blocker_sort_key(right))
            .then_with(|| left.to_string().cmp(&right.to_string()))
    });
    existing.dedup();
    Ok(())
}
