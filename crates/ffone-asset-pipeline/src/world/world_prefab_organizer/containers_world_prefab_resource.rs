use super::*;

pub const WORLD_PREFAB_SCHEMA: &str = "ffone.map-object.v1";

pub const MAP_COMPOSITE_OBJECT_SCHEMA: &str = "ffone.map-composite-object.v1";

pub const WORLD_PREFAB_PLACEMENTS_SCHEMA: &str = "ffone.map-tile-objects.v1";

pub const WORLD_PREFAB_ORGANIZER_REPORT_SCHEMA: &str = "ffone.map-organizer-report.v1";

pub const WORLD_PREFAB_VERIFICATION_SCHEMA: &str = "ffone.map-verification.v1";

pub const WORLD_PREFAB_TOOL: &str = "ffone-asset-pipeline/organize-map";

#[derive(Clone, Debug)]
pub struct WorldPrefabOrganizerOptions {
    pub project_root: PathBuf,
    pub report_path: PathBuf,
    pub tutorial_metadata_root: Option<PathBuf>,
    pub apply: bool,
    pub replace: bool,
}

impl WorldPrefabOrganizerOptions {
    pub fn new(
        project_root: impl Into<PathBuf>,
        report_path: impl Into<PathBuf>,
        apply: bool,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            report_path: report_path.into(),
            tutorial_metadata_root: None,
            apply,
            replace: false,
        }
    }

    pub fn with_tutorial_metadata_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.tutorial_metadata_root = Some(root.into());
        self
    }

    pub fn replacing_existing(mut self) -> Self {
        self.replace = true;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorldPrefabOrganizerMode {
    Plan,
    Apply,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabOrganizerCounts {
    pub tiles: u64,
    pub source_nodes: u64,
    pub source_payloads: u64,
    pub source_visual_instances: u64,
    pub source_collider_instances: u64,
    pub excluded_character_payloads: u64,
    pub excluded_character_nodes: u64,
    #[serde(default)]
    pub excluded_collision_helper_visuals: u64,
    #[serde(default)]
    pub excluded_additive_black_visuals: u64,
    pub reusable_resources: u64,
    pub visual_resources: u64,
    pub collider_resources: u64,
    #[serde(rename = "objects")]
    pub prefabs: u64,
    pub placements: u64,
    pub textures: u64,
    pub output_files: u64,
    pub output_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabOrganizerReport {
    pub schema: String,
    pub tool: String,
    pub source_build: String,
    pub mode: WorldPrefabOrganizerMode,
    pub project_root: String,
    pub asset_root: String,
    pub report_path: String,
    pub world_metadata_root: String,
    pub tutorial_metadata_root: String,
    pub catalog_path: String,
    pub compatibility_policy: String,
    pub classification_policy: String,
    pub counts: WorldPrefabOrganizerCounts,
    pub categories: BTreeMap<String, u64>,
    pub prefixes: BTreeMap<String, u64>,
    pub category_samples: BTreeMap<String, Vec<String>>,
    pub prefix_samples: BTreeMap<String, Vec<String>>,
    pub source_set_blake3: String,
    pub result_set_blake3: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorldPrefabResourceKind {
    Visual,
    #[serde(rename = "collision")]
    Collider,
}

impl WorldPrefabResourceKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Visual => "visual",
            Self::Collider => "collision",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabArtifact {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabBounds {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabSourceIdentity {
    pub asset: String,
    pub path_id: i64,
    pub object_type: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabResource {
    pub id: String,
    pub kind: WorldPrefabResourceKind,
    pub name: String,
    pub category: String,
    pub prefix: String,
    pub family: String,
    pub aliases: Vec<String>,
    pub model: WorldPrefabArtifact,
    pub bounds: WorldPrefabBounds,
    #[serde(default)]
    pub scene: usize,
    #[serde(default)]
    pub mesh: usize,
    #[serde(default)]
    pub primitive: usize,
    #[serde(default)]
    pub vertex_count: u64,
    #[serde(default)]
    pub index_count: u64,
    pub source_mesh: WorldPrefabSourceIdentity,
    pub material_ids: Vec<Option<String>>,
    pub representative_tile: String,
    pub representative_payload: String,
    pub representative_baked_model: String,
    pub representative_baked_blake3: String,
    #[serde(default = "identity_string_matrix")]
    pub representative_world_matrix: [[String; 4]; 4],
    pub pre_winding_model_blake3: Option<String>,
    pub occurrence_count: u64,
    pub derivation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabPart {
    #[serde(rename = "geometry")]
    pub resource: String,
    pub role: WorldPrefabResourceKind,
    pub local_matrix: [[String; 4]; 4],
    pub is_trigger: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabDefinition {
    pub schema: String,
    pub id: String,
    pub name: String,
    pub category: String,
    pub prefix: String,
    pub family: String,
    pub aliases: Vec<String>,
    pub coordinate_space: String,
    pub parts: Vec<WorldPrefabPart>,
    pub source_nodes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_set: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapCompositeObjectDefinition {
    pub schema: String,
    pub id: String,
    pub name: String,
    pub category: String,
    pub prefix: String,
    pub family: String,
    pub model: WorldPrefabArtifact,
    pub visual_scenes: Vec<usize>,
    pub collision_meshes: Vec<usize>,
    pub collision_is_integral: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_set: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabPlacementSetReference {
    pub tile_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_id: Option<String>,
    pub manifest: WorldPrefabArtifact,
    #[serde(rename = "objects")]
    pub artifact: WorldPrefabArtifact,
    pub instance_count: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabPlacement {
    #[serde(rename = "object")]
    pub prefab: String,
    pub source_node: String,
    pub source_geometry: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_layer: Option<i64>,
    pub world_matrix: [[String; 4]; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabPlacementDocument {
    pub schema: String,
    pub source_build: String,
    pub tile_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_id: Option<String>,
    pub coordinate_space: String,
    #[serde(rename = "objects")]
    pub instances: Vec<WorldPrefabPlacement>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabVerification {
    pub schema: String,
    pub catalog_blake3: String,
    pub resources: u64,
    #[serde(rename = "objects")]
    pub prefabs: u64,
    #[serde(rename = "tiles")]
    pub placement_sets: u64,
    pub placements: u64,
    pub textures: u64,
    pub files: u64,
    pub bytes: u64,
    pub maximum_position_reconstruction_error: f64,
    pub result_set_blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct PrefabPartKey {
    pub(super) resource: String,
    pub(super) role: WorldPrefabResourceKind,
    pub(super) is_trigger: bool,
}

#[derive(Clone)]
pub(super) struct PrefabDraft {
    pub(super) id: String,
    pub(super) parts: Vec<PrefabPartKey>,
    pub(super) aliases: BTreeSet<String>,
    pub(super) source_nodes: BTreeSet<String>,
    pub(super) occurrences: u64,
}

#[derive(Clone)]
pub(super) struct ObjectPlan {
    pub(super) taxonomy: Taxonomy,
    pub(super) relative_root: String,
}
