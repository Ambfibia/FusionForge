use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyGuiStyleState {
    pub background: LegacyGuiObjectPointer,
    pub text_color: LegacyGuiColor,
}
