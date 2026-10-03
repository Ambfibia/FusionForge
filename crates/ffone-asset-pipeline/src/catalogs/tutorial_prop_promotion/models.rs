use super::*;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialPropPromotionModel {
    pub id: String,
    pub tutorial_root: String,
    pub prop_root: String,
    pub prop_glb: String,
    pub status: TutorialPropPromotionStatus,
    pub proof: Option<TutorialPropPromotionProof>,
    pub blockers: Vec<TutorialPropPromotionBlocker>,
}

pub(super) fn report_model(
    candidate: &PromotionCandidate,
    status: TutorialPropPromotionStatus,
    proof: Option<TutorialPropPromotionProof>,
    blockers: Vec<TutorialPropPromotionBlocker>,
) -> TutorialPropPromotionModel {
    TutorialPropPromotionModel {
        id: candidate.id.to_owned(),
        tutorial_root: candidate.source_root(),
        prop_root: candidate.target_root(),
        prop_glb: candidate.target_glb(),
        status,
        proof,
        blockers,
    }
}
