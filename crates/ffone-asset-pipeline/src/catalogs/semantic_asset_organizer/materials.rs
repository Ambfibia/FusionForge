use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelMaterialPlan {
    pub status: ModelMaterialStatus,
    pub source_material_count: u64,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelMaterialStatus {
    EmittedWithLogicalModelSidecarPending,
}
