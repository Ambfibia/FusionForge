use super::super::*;

#[derive(Debug, Error)]
pub(in super::super) enum EditorError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Path does not exist: {0}")]
    MissingPath(String),
}
