use std::{io, path::PathBuf};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ContentError>;

#[derive(Debug, Error)]
pub enum ContentError {
    #[error("I/O error at {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid JSON manifest at {path:?}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("unsupported content schema {actual:?}; expected {expected:?}")]
    WrongSchema {
        expected: &'static str,
        actual: String,
    },
    #[error("unsupported protocol {actual}; expected {expected}")]
    WrongProtocol { expected: u16, actual: u16 },
    #[error("invalid locale {0:?}")]
    InvalidLocale(String),
    #[error("invalid neutral provenance {context} label {label:?}")]
    InvalidProvenanceLabel {
        context: &'static str,
        label: String,
    },
    #[error("manifest provenance must contain a producer and at least one source fingerprint")]
    MissingProvenance,
    #[error("invalid lowercase BLAKE3 digest for {context:?}: {digest:?}")]
    InvalidDigest { context: String, digest: String },
    #[error("source fingerprints are not strictly sorted at {0:?}")]
    UnsortedSources(String),
    #[error("duplicate source fingerprint name {0:?}")]
    DuplicateSource(String),
    #[error("manifest contains no native files")]
    EmptyPack,
    #[error("unsafe content path {path:?}: {reason}")]
    UnsafePath { path: String, reason: &'static str },
    #[error("legacy content is forbidden at {path:?}: {reason}")]
    ForbiddenLegacy { path: String, reason: &'static str },
    #[error("symbolic links are forbidden in content packs: {0:?}")]
    Symlink(PathBuf),
    #[error("Windows reparse points are forbidden in content packs: {0:?}")]
    ReparsePoint(PathBuf),
    #[error("content path resolves outside the canonical pack root: {0:?}")]
    OutsidePack(String),
    #[error("content paths must be valid UTF-8: {0:?}")]
    NonUtf8Path(PathBuf),
    #[error("file entries are not strictly path-sorted at {0:?}")]
    UnsortedFiles(String),
    #[error("duplicate or case-colliding content path {0:?}")]
    DuplicatePath(String),
    #[error("manifest-listed file is missing: {0:?}")]
    MissingFile(String),
    #[error("manifest path is not a regular file: {0:?}")]
    NotAFile(String),
    #[error("size mismatch for {path:?}: expected {expected}, got {actual}")]
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    #[error("BLAKE3 mismatch for {path:?}: expected {expected}, got {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("staging file has no builder registration: {0:?}")]
    UnregisteredStagingFile(String),
    #[error("builder registration has no staging file: {0:?}")]
    RegisteredFileMissing(String),
    #[error("content path was registered more than once: {0:?}")]
    DuplicateRegistration(String),
    #[error("pack contains a file not listed by its manifest: {0:?}")]
    UnlistedPackFile(String),
    #[error("runtime access denied for non-manifest path {0:?}")]
    NotManifestListed(String),
    #[error(
        "manifest replacement failed at {manifest:?}: {replace}; rollback from {backup:?} also failed: {rollback}"
    )]
    ManifestRollbackFailed {
        manifest: PathBuf,
        backup: PathBuf,
        replace: String,
        rollback: String,
    },
}

pub(crate) fn io_at(path: impl Into<PathBuf>, source: io::Error) -> ContentError {
    ContentError::Io {
        path: path.into(),
        source,
    }
}
