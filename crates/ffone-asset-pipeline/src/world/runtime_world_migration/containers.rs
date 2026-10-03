use super::*;

pub(super) fn object_mut<'a>(value: &'a mut Value, path: &str) -> Result<&'a mut Map<String, Value>> {
    value
        .as_object_mut()
        .ok_or_else(|| invalid_error(format!("{path:?} root is not an object")))
}

pub(super) fn child_object_mut<'a>(
    parent: &'a mut Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a mut Map<String, Value>> {
    parent
        .get_mut(key)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid_error(format!("{path:?} has no object {key:?}")))
}
