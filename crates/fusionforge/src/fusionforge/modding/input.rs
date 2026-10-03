
pub(super) fn parse_f64(value: &str, label: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|err| format!("bad OBJ {label} '{value}': {err}"))
}
