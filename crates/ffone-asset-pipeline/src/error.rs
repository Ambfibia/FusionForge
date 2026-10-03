use std::{io, path::PathBuf};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, PipelineError>;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error(transparent)]
    Content(#[from] ffone_content::ContentError),
    #[error("I/O error at {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid native JSON at {path:?}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid ffone.mesh.v1 at {path:?}: {reason}")]
    InvalidMesh { path: String, reason: String },
    #[error("invalid ffone.logical-model.v1 publish contract: {0}")]
    InvalidModelContract(String),
    #[error("model audit failed: {0}")]
    ModelAudit(String),
    #[error("semantic asset organization plan failed: {0}")]
    SemanticAssetOrganizer(String),
    #[error("logical-model publish failed: {0}")]
    LogicalModelPublish(String),
    #[error("logical-character install failed: {0}")]
    LogicalCharacterInstall(String),
    #[error("player-avatar cook failed: {0}")]
    PlayerAvatarCook(String),
    #[error("character-creation native data install failed: {0}")]
    CharacterCreationData(String),
    #[error("project-asset manifest refresh failed: {0}")]
    ProjectAssetManifest(String),
    #[error("invalid native manifest: {0}")]
    InvalidManifest(String),
    #[error("unsupported native asset at {path:?}: {reason}")]
    UnsupportedAsset { path: String, reason: String },
    #[error("legacy locator or artifact is forbidden at {path:?}: {reason}")]
    ForbiddenLegacy { path: String, reason: &'static str },
    #[error("output directory already exists; imports never mutate it in place: {0:?}")]
    OutputExists(PathBuf),
    #[error("output path has no usable final component: {0:?}")]
    InvalidOutputPath(PathBuf),
    #[error("two source assets map to the same output path {output:?}: {first:?} and {second:?}")]
    OutputCollision {
        output: String,
        first: String,
        second: String,
    },
    #[error("native asset is too large for GLB 2.0 at {path:?}: {what}")]
    GlbOverflow { path: String, what: &'static str },
    #[error("could not allocate a unique staging directory beside {0:?}")]
    StagingCollision(PathBuf),
}

pub(crate) fn io_at(path: impl Into<PathBuf>, source: io::Error) -> PipelineError {
    PipelineError::Io {
        path: path.into(),
        source,
    }
}
