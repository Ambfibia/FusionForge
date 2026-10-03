use super::*;

pub(super) fn generated_json_error(error: serde_json::Error) -> PipelineError {
    PipelineError::SemanticAssetOrganizer(format!("generated resource-set JSON failed: {error}"))
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::SemanticAssetOrganizer(message.into())
}
