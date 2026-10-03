use super::*;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentBatchTiming {
    pub planning_milliseconds: u64,
    pub environment_warmup_milliseconds: u64,
    pub route_export_milliseconds: u64,
    pub total_milliseconds: u64,
    pub bundle_warmups: Vec<EquipmentBundleWarmupTiming>,
    pub per_route: Vec<EquipmentRouteTiming>,
    pub prefix_summaries: Vec<EquipmentTimingSummary>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentTimingSummary {
    pub routes: u64,
    pub median_milliseconds: u64,
    pub p95_milliseconds: u64,
    pub min_milliseconds: u64,
    pub max_milliseconds: u64,
    pub total_milliseconds: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentTableRowProof {
    pub entity_id: String,
    pub table_owner: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentPhysicalTarget {
    pub asset_index: u64,
    pub asset_name: String,
    pub path_id: i64,
    pub object_type: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentSourceFacts {
    pub hierarchy_nodes: u64,
    pub meshes: u64,
    pub skinned_meshes: u64,
    pub joints: u64,
    pub animations: u64,
    pub materials: u64,
    pub textures: u64,
    pub coordinate_contract: String,
    pub origin_policy: String,
    pub unit_scale: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SemanticPlan {
    pub(super) schema: String,
    pub(super) entities: Vec<SemanticEntity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SemanticEntity {
    pub(super) id: String,
    pub(super) category: String,
    pub(super) semantic_directory: String,
    pub(super) table_owner: JsonValue,
    #[serde(default)]
    pub(super) model_routes: Vec<String>,
}

pub(super) struct StagingResult {
    pub(super) routes: Vec<StagedRoute>,
    pub(super) blockers: Vec<EquipmentModelSourceBlocker>,
    pub(super) bundle_warmups: Vec<EquipmentBundleWarmupTiming>,
    pub(super) per_route: Vec<EquipmentRouteTiming>,
}

pub(super) struct PublicationResult {
    pub(super) exported: Vec<EquipmentModelSourceExported>,
    pub(super) blockers: Vec<EquipmentModelSourceBlocker>,
}
