use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlanModel {
    pub(super) id: String,
    pub(super) legacy_route: String,
    pub(super) output_glb: String,
    pub(super) eligible: bool,
    pub(super) blocker_ids: Vec<String>,
}

pub(super) fn semantic_model_path(category: &str, name: &str) -> String {
    format!("characters/player/{category}/{name}/{name}.glb")
}
