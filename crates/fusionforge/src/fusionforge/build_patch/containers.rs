use super::*;

pub(super) const OBJECT_STRING_KIND: &str = "unity.objectString";

pub(super) fn is_unsupported_object_string_entry(entry: &JsonValue) -> bool {
    if let Some(field_path) = entry.get("fieldPath").and_then(JsonValue::as_str) {
        return is_unsupported_object_string_field_path(field_path);
    }
    entry_path_parts(entry)
        .as_deref()
        .map(is_unsupported_object_string_path_parts)
        .unwrap_or(false)
}

pub(super) fn set_object_field(
    value: &mut UnityValue,
    key: &str,
    new_value: UnityValue,
) -> Result<(), String> {
    value
        .as_object_mut()
        .ok_or_else(|| "value is not an object".to_string())?
        .insert(key.to_string(), new_value);
    Ok(())
}
