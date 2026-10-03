//! Historical offline import inventory.
//!
//! This shape remains local to the Legacy conversion crate so old extraction
//! receipts stay reproducible. It is not a runtime contract and must never be
//! published back into `assets/game`.

use serde::{Deserialize, Serialize};

pub const PROJECT_ASSET_SCHEMA: &str = "ffone.project-assets.v1";
pub const ASSET_MANIFEST_FILE: &str = "asset-manifest.json";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectAssetManifest {
    pub schema: String,
    pub protocol: u16,
    pub locale: String,
    pub source_pack: SourcePackIdentity,
    pub files: Vec<ProjectAssetFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePackIdentity {
    pub schema: String,
    pub manifest_blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectAssetFile {
    pub source_path: String,
    pub path: String,
    pub kind: ProjectAssetKind,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectAssetKind {
    Model,
    Texture,
    Audio,
    Font,
    Data,
    Shader,
}
