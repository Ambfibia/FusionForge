use super::*;

pub(super) fn require_gui_skin_type(object: &Value, name: &str) -> Result<()> {
    match object.get("type").and_then(Value::as_str) {
        Some("GUISkin") => Ok(()),
        Some(actual) => Err(PipelineError::InvalidManifest(format!(
            "legacy GUI skin candidate selected {name:?} with Unity type {actual:?}; expected exactly GUISkin"
        ))),
        None => Err(PipelineError::InvalidManifest(format!(
            "legacy GUI skin candidate selected {name:?} without a Unity type; expected exactly GUISkin"
        ))),
    }
}
