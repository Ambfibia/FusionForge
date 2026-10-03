use serde::{Deserialize, Serialize};

pub const CONTENT_PACK_SCHEMA: &str = "ffone.content-pack.v1";
pub const CONTENT_PROTOCOL: u16 = 104;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentManifest {
    pub schema: String,
    pub protocol: u16,
    pub locale: String,
    pub provenance: Provenance,
    pub files: Vec<ContentFileEntry>,
}

impl ContentManifest {
    pub fn new(
        locale: impl Into<String>,
        provenance: Provenance,
        mut files: Vec<ContentFileEntry>,
    ) -> Self {
        files.sort_by(|left, right| left.path.cmp(&right.path));
        Self {
            schema: CONTENT_PACK_SCHEMA.to_owned(),
            protocol: CONTENT_PROTOCOL,
            locale: locale.into(),
            provenance,
            files,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub producer: String,
    pub sources: Vec<SourceFingerprint>,
}

impl Provenance {
    pub fn new(producer: impl Into<String>, mut sources: Vec<SourceFingerprint>) -> Self {
        sources.sort_by(|left, right| left.label.cmp(&right.label));
        Self {
            producer: producer.into(),
            sources,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFingerprint {
    pub label: String,
    pub blake3: String,
}

impl SourceFingerprint {
    pub fn new(label: impl Into<String>, blake3: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            blake3: blake3.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentFileEntry {
    pub path: String,
    pub kind: ContentKind,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Table,
    World,
    Mesh,
    Material,
    Texture,
    Animation,
    Audio,
    Font,
    Localization,
    Shader,
}
