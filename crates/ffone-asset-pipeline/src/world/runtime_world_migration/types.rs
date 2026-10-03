use super::*;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedWorldTechnicalMetadata {
    pub source_path: String,
    pub archive_path: String,
    pub reason: String,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LegacyWorldEnvironmentReference {
    pub(super) path: String,
    pub(super) blake3: String,
}
