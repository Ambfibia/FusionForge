use super::*;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TutorialCharacterPromotionMode {
    DryRun,
    Apply,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TutorialCharacterPromotionStatus {
    Promote,
    AlreadyPromoted,
    Blocked,
}

pub(super) enum PreparedState {
    Promote(PreparedPromotion),
    AlreadyPromoted(TutorialCharacterPromotionProof),
}
