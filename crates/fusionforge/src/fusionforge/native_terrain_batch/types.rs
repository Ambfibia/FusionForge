use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchScope {
    pub mode: &'static str,
    pub requested_tiles: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSource {
    pub input_path: String,
    pub input_kind: &'static str,
    pub effective_build_root: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCounts {
    pub discovered_dong_resource_count: usize,
    pub discovered_tutorial_terrain_count: usize,
    pub selected_tile_count: usize,
    pub selected_world_tile_count: usize,
    pub selected_tutorial_terrain_count: usize,
    pub preflight_candidate_count: usize,
    pub exported_count: usize,
    pub blocked_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchExported {
    pub scope: &'static str,
    pub tile_id: String,
    pub true_name: String,
    pub source_container_route: String,
    pub asset_route_tile_id: String,
    pub raw_serialized_m_name: String,
    pub provenance_findings: Vec<ProvenanceFinding>,
    pub terrain_data_path_id: i64,
    pub resource_bundle: SourceFileEvidence,
    pub map_scene_bundle: SourceFileEvidence,
    pub terrain_root: String,
    pub terrain_document: String,
    pub placement_status: &'static str,
    pub scene_instance_document: Option<String>,
    pub scene_instance: Option<SceneInstanceSummary>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchBlocked {
    pub scope: Option<&'static str>,
    pub tile_id: Option<String>,
    pub source_path: Option<String>,
    pub stage: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFileEvidence {
    pub path: String,
    pub blake3: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneInstanceSummary {
    pub terrain_collider_path_id: i64,
    pub game_object_path_id: i64,
    pub transform_path_id: i64,
    pub parent_transform_path_id: Option<i64>,
    pub map_tile_root_transform_path_id: i64,
    pub root_chain_blake3: String,
    pub native_local_translation: [f64; 3],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceFinding {
    pub code: &'static str,
    pub message: String,
}

#[derive(Clone, Debug)]
pub(super) struct TileSource {
    pub(super) scope: TerrainScope,
    pub(super) tile_id: String,
    pub(super) resource: SourceFileEvidence,
    pub(super) resource_path: PathBuf,
    pub(super) map_scene: SourceFileEvidence,
    pub(super) map_scene_path: PathBuf,
    pub(super) destination: String,
}

#[derive(Debug)]
pub(super) struct Preflight {
    pub(super) discovered_count: usize,
    pub(super) discovered_tutorial_count: usize,
    pub(super) selected_count: usize,
    pub(super) selected_world_count: usize,
    pub(super) selected_tutorial_count: usize,
    pub(super) candidates: Vec<TileSource>,
    pub(super) blocked: Vec<BatchBlocked>,
}

pub(super) struct LoadedTile {
    pub(super) env: UnityEnvironment,
    pub(super) resource_asset_names: BTreeSet<String>,
    pub(super) map_asset_names: BTreeSet<String>,
    pub(super) _session: SessionDirectory,
}

pub(super) struct LoadedMapScene {
    pub(super) env: UnityEnvironment,
    pub(super) map_asset_names: BTreeSet<String>,
    pub(super) _session: SessionDirectory,
}

#[derive(Debug)]
pub(super) struct SceneLink {
    pub(super) document: JsonValue,
    pub(super) summary: SceneInstanceSummary,
    pub(super) sidecars: Vec<SceneSidecar>,
    pub(super) gameplay_attributes: Option<JsonValue>,
    pub(super) semantic_blockers: Vec<(&'static str, String)>,
}

#[derive(Debug)]
pub(super) struct SceneSidecar {
    pub(super) relative_path: String,
    pub(super) bytes: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct EnvironmentScan {
    pub(super) scope: &'static str,
    pub(super) tile_id: String,
    pub(super) document: JsonValue,
    pub(super) sidecars: Vec<SceneSidecar>,
    pub(super) blocked: Vec<BatchBlocked>,
    pub(super) ambience_exact: bool,
    pub(super) terrain_detail_exact: bool,
    pub(super) placement_classification: &'static str,
}

pub(super) struct EnvironmentScanOutcome {
    pub(super) scan: Option<EnvironmentScan>,
    pub(super) fatal: Option<String>,
}

pub(super) struct TileOutcome {
    pub(super) exported: Option<BatchExported>,
    pub(super) blocked: Vec<BatchBlocked>,
    pub(super) fatal: Option<String>,
}
