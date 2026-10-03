use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ManagedSelectionDocument {
    pub(super) level: usize,
    pub(super) exact_entry: String,
    pub(super) route: String,
}
