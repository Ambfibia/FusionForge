use super::*;

#[derive(Clone, Debug)]
pub struct StaticWorldWindingRepairOptions {
    pub project_root: PathBuf,
    pub report_path: PathBuf,
    pub apply: bool,
}

impl StaticWorldWindingRepairOptions {
    pub fn new(
        project_root: impl Into<PathBuf>,
        report_path: impl Into<PathBuf>,
        apply: bool,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            report_path: report_path.into(),
            apply,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticWorldWindingRepairReport {
    pub schema: String,
    pub tool: String,
    pub source_build: String,
    pub mode: StaticWorldWindingRepairMode,
    pub operation: String,
    pub source_contract: String,
    pub project_root: String,
    pub asset_root: String,
    pub report_path: String,
    pub backup_root: Option<String>,
    pub scopes: Vec<StaticWorldWindingRepairScopeReport>,
    pub counts: StaticWorldWindingRepairCounts,
    pub source_set_blake3: String,
    pub result_set_blake3: String,
    pub runtime_world_before_blake3: String,
    pub runtime_world_after_blake3: String,
    pub files: Vec<StaticWorldWindingRepairFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticWorldWindingRepairScopeReport {
    pub scope: String,
    pub ownership_source: String,
    pub ownership_target: String,
    pub visual_files: u64,
    pub converted_files: u64,
    pub unchanged_files: u64,
    pub scene_files: u64,
    pub source_set_blake3: String,
    pub result_set_blake3: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticWorldWindingRepairCounts {
    pub visual_files: u64,
    pub converted_files: u64,
    pub unchanged_files: u64,
    pub resumed_files: u64,
    pub normal_bearing_files: u64,
    pub normal_less_files: u64,
    pub index_accessors: u64,
    pub triangles: u64,
    pub aligned_triangles_before: u64,
    pub opposed_triangles_before: u64,
    pub aligned_triangles_after: u64,
    pub opposed_triangles_after: u64,
    pub degenerate_triangles: u64,
    pub tangent_accessors: u64,
    pub scene_files: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticWorldWindingRepairFile {
    pub scope: String,
    pub path: String,
    pub bytes: u64,
    pub source_blake3: String,
    pub result_blake3: String,
    pub action: StaticWorldWindingRepairAction,
    pub normal_bearing: bool,
    pub index_accessors: u64,
    pub triangles: u64,
    pub aligned_before: u64,
    pub opposed_before: u64,
    pub aligned_after: u64,
    pub opposed_after: u64,
    pub degenerate: u64,
    pub tangent_accessors: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticWorldWindingVerification {
    pub schema: String,
    pub source_report: String,
    pub visual_files: u64,
    pub scene_files: u64,
    pub triangles: u64,
    pub normal_bearing_files: u64,
    pub normal_less_files: u64,
    pub result_set_blake3: String,
    pub runtime_world_blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticWorldWindingArchiveReport {
    pub schema: String,
    pub source_build: String,
    pub revision_plan_blake3: String,
    pub ownership_source: String,
    pub ownership_archive: String,
    pub bytes: u64,
    pub blake3: String,
    pub already_archived: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RevisionReport {
    pub(super) schema: String,
    pub(super) source_build: String,
    pub(super) plan_blake3: String,
    pub(super) archived_files: u64,
    pub(super) archived_bytes: u64,
    pub(super) files: Vec<ArchivedRuntimeMetadata>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RevisionPlanIdentity<'a> {
    pub(super) schema: &'static str,
    pub(super) source_build: &'a str,
    pub(super) files: &'a [ArchivedRuntimeMetadata],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct GeometryStats {
    pub(super) normal_bearing: bool,
    pub(super) index_accessors: u64,
    pub(super) triangles: u64,
    pub(super) aligned: u64,
    pub(super) opposed: u64,
    pub(super) degenerate: u64,
    pub(super) tangent_accessors: u64,
}

impl GeometryStats {
    pub(super) fn needs_reversal(self) -> bool {
        !self.normal_bearing || self.opposed >= self.aligned
    }
}

pub(super) struct ScopeWork {
    pub(super) label: &'static str,
    pub(super) ownership_source: PathBuf,
    pub(super) ownership_target: PathBuf,
    pub(super) ownership: TutorialStaticWorldOwnership,
    pub(super) file_start: usize,
    pub(super) file_end: usize,
    pub(super) scene_hashes: BTreeMap<String, String>,
    pub(super) source_set_blake3: String,
    pub(super) result_set_blake3: String,
    pub(super) converted_files: u64,
    pub(super) unchanged_files: u64,
}

pub(super) struct AccessorLayout {
    pub(super) offset: usize,
    pub(super) count: usize,
    pub(super) stride: usize,
    pub(super) component_type: u64,
    pub(super) element_components: usize,
}
