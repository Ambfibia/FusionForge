use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticTexture {
    pub source_name: String,
    pub path: String,
    pub source_mip_count: u64,
    pub published_policy: String,
    pub png_sha256: String,
    pub mip_paths: Vec<String>,
}
