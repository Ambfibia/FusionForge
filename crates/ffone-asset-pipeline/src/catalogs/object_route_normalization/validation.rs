use super::*;

pub(super) fn generated_json_error(source: serde_json::Error) -> PipelineError {
    PipelineError::Json {
        path: "generated object-route JSON".to_owned(),
        source,
    }
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::InvalidManifest(message.into())
}
