use super::*;

pub(super) fn output_collision<T>(exact: &str, owner: &str, previous: &RegisteredPath) -> Result<T> {
    Err(PipelineError::OutputCollision {
        output: exact.to_owned(),
        first: format!("{} ({})", previous.owner, previous.exact),
        second: owner.to_owned(),
    })
}
