use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceObjectReference {
    pub(super) id: String,
    pub(super) asset_index: usize,
    pub(super) path_id: i64,
    #[serde(rename = "type")]
    pub(super) object_type: String,
}
