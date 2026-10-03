use super::*;

pub(super) fn invalid_error(reason: impl Into<String>) -> PipelineError {
    PipelineError::UnsupportedAsset {
        path: GAMEPLAY_UI_ROOT.to_owned(),
        reason: reason.into(),
    }
}
