use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ArchivedRevisionFile {
    pub(super) source_path: String,
    pub(super) archive_path: String,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}
