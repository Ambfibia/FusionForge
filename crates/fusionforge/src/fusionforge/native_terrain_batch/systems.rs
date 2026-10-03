use super::*;

pub(super) fn refresh_publication_plan(
    staging_root: &Path,
    output_root: &Path,
    manifest: &JsonValue,
    publication_plan: &mut JsonValue,
) -> Result<(), String> {
    let exported = manifest
        .get("exported")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "enriched manifest has no exported array".to_string())?
        .iter()
        .map(|entry| {
            Ok((
                (
                    required_json_str(entry, "scope", "manifest exported entry")?.to_string(),
                    required_json_str(entry, "tileId", "manifest exported entry")?.to_string(),
                ),
                entry.clone(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let entries = publication_plan
        .get_mut("entries")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| "publication plan has no entries array".to_string())?;
    for entry in entries {
        let scope = required_json_str(entry, "scope", "publication entry")?.to_string();
        let instance_id = required_json_str(entry, "instanceId", "publication entry")?.to_string();
        let tile_id = instance_id
            .strip_prefix("map_")
            .or_else(|| instance_id.strip_prefix("tile_"))
            .ok_or_else(|| format!("invalid native terrain instanceId {instance_id:?}"))?
            .to_string();
        let exported = exported
            .get(&(scope.clone(), tile_id.clone()))
            .ok_or_else(|| format!("no enriched manifest entry for {scope} {tile_id}"))?;
        let source_root = required_json_str(entry, "sourceRoot", "publication entry")?.to_string();
        let source_root_path = staging_root.join(path_from_forward_slashes(&source_root));
        let terrain_document =
            required_json_str(entry, "terrainDocument", "publication entry")?.to_string();
        let environment_full =
            required_json_str(exported, "environmentDocument", "manifest exported entry")?;
        let environment_relative = environment_full
            .strip_prefix(&format!("{source_root}/"))
            .ok_or_else(|| {
                format!(
                    "environment document {environment_full:?} is outside source root {source_root:?}"
                )
            })?
            .to_string();
        let object = entry
            .as_object_mut()
            .ok_or_else(|| "publication entry is not an object".to_string())?;
        object.insert(
            "terrainDocumentBlake3".to_string(),
            JsonValue::String(hash_output_document(&source_root_path, &terrain_document)?),
        );
        object.insert(
            "environmentDocument".to_string(),
            JsonValue::String(environment_relative.clone()),
        );
        object.insert(
            "environmentDocumentBlake3".to_string(),
            JsonValue::String(hash_output_document(
                &source_root_path,
                &environment_relative,
            )?),
        );
        if let Some(scene_document) = object
            .get("sceneInstanceDocument")
            .and_then(JsonValue::as_str)
            .map(str::to_string)
        {
            object.insert(
                "sceneInstanceDocumentBlake3".to_string(),
                JsonValue::String(hash_output_document(&source_root_path, &scene_document)?),
            );
        }
        object.insert(
            "payloads".to_string(),
            JsonValue::Array(publication_payloads(&source_root_path)?),
        );
    }
    let blockers = manifest
        .get("blocked")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "enriched manifest has no blocked array".to_string())?
        .iter()
        .map(|blocked| {
            let scope = blocked
                .get("scope")
                .and_then(JsonValue::as_str)
                .unwrap_or("batch");
            let tile_id = blocked
                .get("tileId")
                .and_then(JsonValue::as_str)
                .unwrap_or("batch");
            let instance_id = match scope {
                "worldMap" => format!("map_{tile_id}"),
                "tutorial" => format!("tile_{tile_id}"),
                _ => tile_id.to_string(),
            };
            json!({
                "scope": scope,
                "instanceId": instance_id,
                "stage": blocked.get("stage").cloned().unwrap_or(JsonValue::String("enrichment".to_string())),
                "code": blocked.get("code").cloned().unwrap_or(JsonValue::String("unknown".to_string())),
                "message": blocked.get("message").cloned().unwrap_or(JsonValue::String("unknown blocker".to_string())),
                "placementStatus": "blocked",
            })
        })
        .collect::<Vec<_>>();
    publication_plan["status"] = manifest["status"].clone();
    publication_plan["sourceOutputRoot"] =
        JsonValue::String(output_root.to_string_lossy().to_string());
    publication_plan["blocked"] = JsonValue::Array(blockers);
    Ok(())
}
