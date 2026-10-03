use super::*;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialCharacterPromotionModel {
    pub id: String,
    pub category: RuntimeCharacterCategory,
    pub tutorial_root: String,
    pub character_root: String,
    pub character_glb: String,
    pub status: TutorialCharacterPromotionStatus,
    pub proof: Option<TutorialCharacterPromotionProof>,
    pub blockers: Vec<TutorialCharacterPromotionBlocker>,
}

pub(super) fn proof_glb_true_name(candidate_root: &Path, candidate: &PromotionCandidate) -> Result<String> {
    let path = candidate_root.join(native_path(&format!(
        "models/mob/{}/{}",
        candidate.id, candidate.glb_name
    )));
    let bytes = read_regular(&path)?;
    gpu_model_facts_from_glb(&bytes)
        .map(|facts| facts.true_name)
        .map_err(|error| invalid_error(format!("GLB facts failed: {error}")))
}
