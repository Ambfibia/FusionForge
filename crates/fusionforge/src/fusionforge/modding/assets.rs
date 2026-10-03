
pub(super) fn parse_index(value: &str, label: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<isize>()
        .map_err(|err| format!("bad OBJ {label} index '{value}': {err}"))?;
    if parsed <= 0 {
        return Err(format!("OBJ {label} index must be positive, got {value}"));
    }
    Ok(parsed as usize - 1)
}
