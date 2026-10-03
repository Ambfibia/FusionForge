use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapTileDocument {
    pub schema: String,
    pub source_build: String,
    pub id: String,
    #[serde(default)]
    pub legacy_aliases: Vec<String>,
    pub grid: [i32; 2],
    pub coordinate_space: String,
    pub terrain: WorldPrefabArtifact,
    pub objects: WorldPrefabArtifact,
    pub scene: WorldPrefabArtifact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behaviour: Option<WorldPrefabArtifact>,
    pub files: Vec<WorldPrefabArtifact>,
    pub source_scene: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapReconstructionProof {
    pub source_set_blake3: String,
    pub resources: u64,
    pub maximum_position_error: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HierarchyDocument {
    pub(super) schema: String,
    pub(super) source_build: String,
    pub(super) tile_id: String,
    pub(super) nodes: Vec<HierarchyNode>,
    pub(super) payloads: Vec<HierarchyPayload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HierarchyNode {
    pub(super) id: String,
    #[serde(default)]
    pub(super) name: String,
    #[serde(default)]
    pub(super) parent_id: Option<String>,
    #[serde(default)]
    pub(super) layer: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct ResourceKey {
    pub(super) kind: WorldPrefabResourceKind,
    pub(super) source_asset: String,
    pub(super) source_path_id: i64,
    pub(super) material_ids: Vec<Option<String>>,
}

#[derive(Clone)]
pub(super) struct ResourceDraft {
    pub(super) id: String,
    pub(super) key: ResourceKey,
    pub(super) aliases: BTreeSet<String>,
    pub(super) representative: HierarchyPayload,
    pub(super) representative_tile: String,
    pub(super) baked_hash: String,
    pub(super) occurrences: u64,
}

#[derive(Clone)]
pub(super) struct PlacementDraft {
    pub(super) scope: String,
    pub(super) tile_id: String,
    pub(super) prefab: String,
    pub(super) source_node: String,
    pub(super) source_geometry: BTreeMap<String, String>,
    pub(super) legacy_layer: Option<i64>,
    pub(super) world_matrix: [[f64; 4]; 4],
}

pub(super) struct BuiltResource {
    pub(super) record: WorldPrefabResource,
}

#[derive(Clone)]
pub(super) struct Taxonomy {
    pub(super) name: String,
    pub(super) category: String,
    pub(super) prefix: String,
    pub(super) family: String,
}

pub(super) struct AccessorLayout {
    pub(super) offset: usize,
    pub(super) count: usize,
    pub(super) stride: usize,
}
