use super::*;

pub(super) fn validate_true_name(label: &str, name: &str) -> Result<()> {
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        return player_error(format!("{label} has no usable true m_Name"));
    }
    Ok(())
}

pub(super) fn player_error<T>(detail: impl Into<String>) -> Result<T> {
    Err(player_message(detail))
}
