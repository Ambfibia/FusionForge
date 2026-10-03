use super::*;

#[derive(Clone, Debug)]
pub struct PlayerAvatarCookOptions {
    pub bundle: PathBuf,
    pub objects_dump: PathBuf,
    pub logical_plan: PathBuf,
    pub output_root: PathBuf,
    pub part_sources: BTreeMap<String, PathBuf>,
}

impl PlayerAvatarCookOptions {
    pub fn new(
        bundle: impl Into<PathBuf>,
        objects_dump: impl Into<PathBuf>,
        logical_plan: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            bundle: bundle.into(),
            objects_dump: objects_dump.into(),
            logical_plan: logical_plan.into(),
            output_root: output_root.into(),
            part_sources: BTreeMap::new(),
        }
    }

    pub fn with_part_source(
        mut self,
        exact_route: impl Into<String>,
        source: impl Into<PathBuf>,
    ) -> Self {
        self.part_sources.insert(exact_route.into(), source.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEvidence {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererRemapEvidence {
    pub renderer_path_id: i64,
    pub renderer_game_object_path_id: i64,
    pub renderer_true_name: String,
    pub actor_wear_index_table_path_id: i64,
    pub renderer_bone_count: usize,
    pub source_joint_count: usize,
    pub inverse_bind_matrix_count: usize,
    pub actor_bone_indices: Vec<usize>,
    pub actor_bone_names: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPartEvidence {
    pub part: String,
    pub participation: String,
    pub ownership: RouteOwnership,
    pub source: FileEvidence,
    pub planned_semantic_glb: String,
    pub source_mesh_count: usize,
    pub source_material_count: usize,
    pub source_texture_count: usize,
    pub source_skeleton_joint_count: usize,
    pub source_warnings: Vec<String>,
    pub legacy_alias_warning_classified: bool,
    pub renderer_remaps: Vec<RendererRemapEvidence>,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestSerEquipment {
    pub slot: String,
    pub item_type: i16,
    pub item_id: i16,
    pub exact_route: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerAvatarCookReport {
    pub schema: String,
    pub profile: String,
    pub status: String,
    pub complete: bool,
    pub bundle: FileEvidence,
    pub objects_dump: FileEvidence,
    pub logical_model_plan: FileEvidence,
    pub selection: TestSerSelection,
    pub native_coordinate_contract: NativeCoordinateContract,
    pub base_skeleton: SkeletonOutputEvidence,
    pub parts: Vec<PlayerPartEvidence>,
    pub blockers: Vec<String>,
}

#[derive(Clone, Copy)]
pub(super) struct PartSpec {
    pub(super) part: &'static str,
    pub(super) participation: &'static str,
    pub(super) route: &'static str,
    pub(super) semantic_glb: &'static str,
}
