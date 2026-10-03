use super::*;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TriageStatus {
    pub(super) allow_unsatisfied: bool,
}
