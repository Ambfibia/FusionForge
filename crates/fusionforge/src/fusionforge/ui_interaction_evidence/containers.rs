use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ManagedRawContainerDocument {
    pub(super) relative_path: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}
