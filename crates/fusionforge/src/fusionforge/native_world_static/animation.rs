use super::*;

/// Resolve and publish every AnimationClip referenced by a legacy Animation
/// component. The serialized curve payload remains exact source evidence;
/// semantic TRS/material cooking happens in FFOne's asset pipeline.
pub(super) fn resolve_animation_clip_identity(
    env: &UnityEnvironment,
    value: Option<&UnityValue>,
    clips: &mut BTreeMap<String, JsonValue>,
) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let pointer = value
        .as_pointer()
        .ok_or_else(|| "Animation clip reference is not a PPtr".to_string())?;
    if pointer.is_null() {
        return Ok(None);
    }
    let key = env
        .resolve_pointer(pointer)
        .map_err(|err| format!("Animation clip pointer cannot resolve: {err}"))?;
    if object_type(env, key)? != "AnimationClip" {
        return Err(format!(
            "Animation clip pointer resolves to {}, expected AnimationClip",
            object_type(env, key)?
        ));
    }
    let id = source_id(env, key);
    if !clips.contains_key(&id) {
        let body = env
            .read_object(key)
            .map_err(|err| format!("could not read AnimationClip {id}: {err}"))?;
        let fields = body
            .as_object()
            .map(|object| {
                object
                    .iter()
                    .filter(|(name, _)| name.as_str() != "m_Name")
                    .map(|(name, value)| (name.clone(), unity_value_to_json(value)))
                    .collect::<JsonMap<String, JsonValue>>()
            })
            .unwrap_or_default();
        clips.insert(
            id.clone(),
            json!({
                "id": id,
                "name": object_name(&body),
                "source": source_json(env, key)?,
                "fields": fields,
            }),
        );
    }
    Ok(Some(id))
}
