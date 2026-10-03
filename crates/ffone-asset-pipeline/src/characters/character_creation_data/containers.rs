use super::*;

pub(super) fn object<'a>(parent: &'a Map<String, Value>, key: &str) -> Result<&'a Map<String, Value>> {
    parent
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_error(format!("missing object {key}")))
}
