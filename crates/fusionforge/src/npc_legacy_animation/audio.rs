use super::*;

pub(crate) fn classify_metadata_only_sound_event_pointers(
    animations: &mut [JsonValue],
) -> Result<(), String> {
    for animation in animations {
        let asset = animation
            .get("asset")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string();
        let path_id = animation
            .get("pathId")
            .and_then(JsonValue::as_i64)
            .unwrap_or_default();
        let name = animation
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or("<unnamed>")
            .to_string();
        let events = animation
            .get_mut("events")
            .and_then(JsonValue::as_array_mut)
            .ok_or_else(|| format!("AnimationClip {name} has no events array"))?;
        for (index, event) in events.iter_mut().enumerate() {
            let metadata_only_sound = event.get("functionName").and_then(JsonValue::as_str)
                == Some("sound")
                && event
                    .get("stringParameter")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|value| !value.trim().is_empty());
            let legacy_unused =
                is_exact_larry_unused_end_event_pointer(&asset, path_id, &name, index, event);
            let provenance = event
                .get_mut("objectParameterProvenance")
                .and_then(JsonValue::as_object_mut)
                .ok_or_else(|| {
                    format!("AnimationClip {name} event[{index}] has no object provenance")
                })?;
            if provenance.get("interpretation").and_then(JsonValue::as_str)
                == Some("non-null-unresolved")
                && metadata_only_sound
            {
                provenance.insert(
                    "interpretation".to_string(),
                    JsonValue::String("non-null-metadata-only".to_string()),
                );
            } else if provenance.get("interpretation").and_then(JsonValue::as_str)
                == Some("non-null-unresolved")
                && legacy_unused
            {
                provenance.insert(
                    "interpretation".to_string(),
                    JsonValue::String("non-null-legacy-unused".to_string()),
                );
            }
        }
    }
    Ok(())
}
