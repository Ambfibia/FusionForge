use super::*;

pub(super) fn require_exact_state(
    shader_name: &str,
    scope: &str,
    actual: &RawState,
    expected: &RawState,
) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "shader {shader_name:?} {scope} render-state evidence contradicts the audited program: actual {actual:?}, expected {expected:?}"
        ))
    }
}

pub(super) fn require_token_count(
    tokens: &[&str],
    count: usize,
    context: &str,
    statement: &str,
) -> Result<(), String> {
    if tokens.len() == count {
        Ok(())
    } else {
        Err(format!(
            "{context} has unsupported render-state syntax {statement:?}"
        ))
    }
}
