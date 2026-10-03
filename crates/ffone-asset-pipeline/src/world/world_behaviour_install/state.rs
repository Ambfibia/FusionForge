use super::*;

pub(super) fn infinity_mode(value: &JsonValue, field: &str) -> i64 {
    value.get(field).and_then(JsonValue::as_i64).unwrap_or(0)
}
