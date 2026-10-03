use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StaticWorldWindingRepairAction {
    Converted,
    AlreadyAligned,
    ResumedConverted,
}
