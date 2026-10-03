use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldBehaviourOwnership {
    pub schema: String,
    pub installer: String,
    pub source_build: String,
    pub source_set_blake3: String,
    pub tiles: Vec<WorldBehaviourTileProof>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldBehaviourTileProof {
    pub tile_id: String,
    pub scope: String,
    pub source_archive_blake3: String,
    pub export_blake3: String,
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Debug)]
pub(super) struct HierarchyNode {
    pub(super) name: String,
    pub(super) parent: Option<String>,
    pub(super) local_translation: [f64; 3],
    pub(super) local_rotation: [f64; 4],
    pub(super) local_scale: [f64; 3],
    pub(super) world_matrix: [[f64; 4]; 4],
}

#[derive(Debug, Default)]
pub(super) struct Totals {
    pub(super) billboards: u64,
    pub(super) visibility_switches: u64,
    pub(super) effect_emitters: u64,
    pub(super) animations: u64,
    pub(super) animation_clips: u64,
    pub(super) triggers: u64,
    pub(super) waypoints: u64,
    pub(super) trigger_volumes: u64,
    pub(super) rigid_bodies: u64,
    pub(super) blockers: u64,
}
