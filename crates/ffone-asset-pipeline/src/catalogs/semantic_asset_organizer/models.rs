use super::*;

pub(super) const LOGICAL_MODEL_PLAN_SCHEMA: &str = "ffclient.logical-model-export-plan.v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelProposal {
    pub id: String,
    pub legacy_route: String,
    pub true_root_m_name: String,
    pub category: SemanticCategory,
    pub semantic_directory: String,
    pub output_glb: String,
    pub proof: String,
    pub source_root: SerializedObjectIdentity,
    pub feature_closure: ModelFeatureClosure,
    pub materials: ModelMaterialPlan,
    pub entity_ids: Vec<String>,
    pub eligible: bool,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelFeatureClosure {
    pub status: ModelFeatureClosureStatus,
    pub source_mesh_count: u64,
    pub source_skinned_mesh_renderer_count: u64,
    pub source_bone_count: u64,
    pub source_animation_component_count: u64,
    pub source_animation_clip_count: u64,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFeatureClosureStatus {
    PreserveExactLogicalRootClosure,
}

pub(super) fn face_head_model_aliases(row: &Value, field: &str) -> Vec<FieldRoute> {
    let Some(value) = string_field(row, field) else {
        return Vec::new();
    };
    let Some(base) = alias_base(value, &[]) else {
        return Vec::new();
    };
    (1..=5)
        .filter_map(|shape| make_route("wear", &format!("{base}_type0{shape}"), "nif"))
        .collect()
}
