use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentGpuModelRun {
    pub category: String,
    pub true_name: String,
    pub relative_glb: String,
    pub selected_exact_animation_name: Option<String>,
    pub selection_reasons: Vec<String>,
    pub facts: EquipmentGpuFactsSummary,
    pub disposition: String,
    pub process_exit_code: Option<i32>,
    pub elapsed_milliseconds: u64,
    pub evidence_json: String,
    pub screenshot_png: String,
}

pub(super) fn model_blocker(
    stage: &str,
    code: &str,
    candidate: &Candidate,
    detail: &str,
    evidence: Value,
    disposition: &str,
) -> EquipmentGpuBlocker {
    EquipmentGpuBlocker {
        stage: stage.to_owned(),
        code: code.to_owned(),
        category: Some(candidate.category.clone()),
        true_name: Some(candidate.true_name.clone()),
        relative_glb: Some(candidate.relative_glb.clone()),
        detail: detail.to_owned(),
        evidence,
        disposition: disposition.to_owned(),
    }
}
