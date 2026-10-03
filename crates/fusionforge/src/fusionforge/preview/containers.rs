use super::*;

pub(super) fn unity_value_text(value: &UnityValue) -> Option<String> {
    value
        .as_bytes()
        .map(|bytes| String::from_utf8_lossy(bytes).to_string())
        .or_else(|| value.as_str().map(str::to_string))
}

pub fn object_key_from_pointer(
    env: &UnityEnvironment,
    pointer: Option<&UnityValue>,
) -> Option<ObjectKey> {
    let pointer = pointer?.as_pointer()?;
    env.resolve_pointer(pointer).ok()
}
