//! Strict native content packs for FFOneClient.
//!
//! This crate has no Unity reader and intentionally refuses legacy client artifacts. A builder
//! only inventories files that are already in native runtime formats; the runtime only opens files
//! named and fingerprinted by the validated manifest.

#![forbid(unsafe_code)]

mod builder;
mod error;
mod manifest;
mod policy;
mod runtime;
mod validation;

pub use builder::ContentPackBuilder;
pub use error::{ContentError, Result};
pub use manifest::{
    CONTENT_PACK_SCHEMA, CONTENT_PROTOCOL, ContentFileEntry, ContentKind, ContentManifest,
    Provenance, SourceFingerprint,
};
pub use runtime::ContentPack;
pub use validation::{MANIFEST_FILE, validate_pack};
