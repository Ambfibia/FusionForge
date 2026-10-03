use super::*;

pub(super) fn require_bool(value: &Value, key: &str, expected: bool, context: &str) -> Result<()> {
    if value.get(key).and_then(Value::as_bool) != Some(expected) {
        return semantic_error(format!(
            "{context} {key:?} must be {expected} for plan-only native evidence"
        ));
    }
    Ok(())
}

pub(super) fn semantic_error<T>(reason: impl Into<String>) -> Result<T> {
    Err(PipelineError::SemanticAssetOrganizer(reason.into()))
}
