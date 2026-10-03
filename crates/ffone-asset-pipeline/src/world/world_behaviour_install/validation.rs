use super::*;

pub(super) fn json_error(source: serde_json::Error) -> PipelineError {
    PipelineError::Json {
        path: "<generated>".to_owned(),
        source,
    }
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::ProjectAssetManifest(message.into())
}
