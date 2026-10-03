use super::*;

pub(super) fn validate_published_animations(document: &Value, clips: &[(&str, i64)]) -> Result<()> {
    let animations = document
        .get("animations")
        .and_then(Value::as_array)
        .ok_or_else(|| rig_message("shared skeleton GLB has no animations"))?;
    if animations.len() != clips.len() {
        return rig_error(format!(
            "shared skeleton GLB has {} animations, expected {}",
            animations.len(),
            clips.len()
        ));
    }
    for (animation, (expected, _)) in animations.iter().zip(clips) {
        if animation.get("name").and_then(Value::as_str) != Some(*expected)
            || animation
                .get("channels")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
        {
            return rig_error(format!(
                "shared skeleton GLB animation {expected:?} is absent or empty"
            ));
        }
    }
    Ok(())
}
