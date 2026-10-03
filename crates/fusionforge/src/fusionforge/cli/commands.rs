use super::*;

pub(super) fn normalize_command(value: &str) -> String {
    Path::new(value)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(value)
        .trim_start_matches('-')
        .replace('_', "-")
        .to_lowercase()
}
