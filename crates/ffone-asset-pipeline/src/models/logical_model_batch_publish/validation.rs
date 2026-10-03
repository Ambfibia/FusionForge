use super::*;

pub(super) fn validate_windows_component(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || matches!(value, "." | "..")
        || value.trim() != value
        || value.ends_with('.')
        || value
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
        || value.encode_utf16().count() > 255
    {
        return batch_error(format!("{label} component is not Windows-safe: {value:?}"));
    }
    let device_stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(device_stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || device_stem.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || device_stem.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        });
    if reserved {
        return batch_error(format!(
            "{label} component is a reserved Windows device name: {value:?}"
        ));
    }
    Ok(())
}

pub(super) fn batch_error<T>(message: impl Into<String>) -> Result<T> {
    Err(batch_error_value(message))
}

pub(super) fn batch_error_value(message: impl Into<String>) -> PipelineError {
    PipelineError::LogicalModelPublish(format!("batch publication: {}", message.into()))
}
