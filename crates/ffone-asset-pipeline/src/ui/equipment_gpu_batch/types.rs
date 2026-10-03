use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentGpuBatchOptions {
    pub candidate_root: PathBuf,
    pub evidence_root: PathBuf,
    pub preview_executable: PathBuf,
    pub mode: EquipmentGpuBatchMode,
    pub report_path: PathBuf,
    pub max_frames: u64,
    pub timeout_seconds: f64,
    pub shard: Option<EquipmentGpuShard>,
}

impl EquipmentGpuBatchOptions {
    pub fn new(
        candidate_root: impl Into<PathBuf>,
        evidence_root: impl Into<PathBuf>,
        preview_executable: impl Into<PathBuf>,
        mode: EquipmentGpuBatchMode,
        report_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            candidate_root: candidate_root.into(),
            evidence_root: evidence_root.into(),
            preview_executable: preview_executable.into(),
            mode,
            report_path: report_path.into(),
            max_frames: 900,
            timeout_seconds: 45.0,
            shard: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentGpuShard {
    pub index: u32,
    pub count: u32,
}

impl EquipmentGpuShard {
    pub fn parse(value: &str) -> std::result::Result<Self, String> {
        let (index, count) = value
            .split_once('/')
            .ok_or_else(|| "--shard must use INDEX/COUNT".to_owned())?;
        let index = index
            .parse::<u32>()
            .map_err(|_| "--shard INDEX must be an unsigned integer".to_owned())?;
        let count = count
            .parse::<u32>()
            .map_err(|_| "--shard COUNT must be an unsigned integer".to_owned())?;
        if count < 2 || index >= count {
            return Err("--shard requires COUNT >= 2 and INDEX < COUNT".to_owned());
        }
        Ok(Self { index, count })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentGpuBatchReport {
    pub schema: String,
    pub status: String,
    pub mode: EquipmentGpuBatchMode,
    pub shard: Option<EquipmentGpuShard>,
    pub scope: String,
    pub standalone_gate_applicable_to_rigid: bool,
    pub standalone_gate_applicable_to_self_contained_skinned: bool,
    pub player_attachment_parity_asserted: bool,
    pub visual_parity_pending: bool,
    pub publishable: bool,
    pub production_assets_mutated: bool,
    pub candidate_root: String,
    pub evidence_root: String,
    pub candidate_batch_report: GpuBatchFileEvidence,
    pub preview_executable: GpuBatchFileEvidence,
    pub deterministic_selection_policy: String,
    pub animation_selection_policy: String,
    pub timing: EquipmentGpuBatchTiming,
    pub counts: EquipmentGpuBatchCounts,
    pub coverage: EquipmentGpuCoverage,
    pub models: Vec<EquipmentGpuModelRun>,
    pub blockers: Vec<EquipmentGpuBlocker>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GpuBatchFileEvidence {
    pub path: String,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentGpuBatchTiming {
    pub preflight_milliseconds: u64,
    pub preview_execution_milliseconds: u64,
    pub independent_audit_milliseconds: u64,
    pub total_milliseconds: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentGpuBatchCounts {
    pub candidate_models: u64,
    pub selected_models: u64,
    pub executed_models: u64,
    pub resumed_valid_models: u64,
    pub standalone_gpu_passed_models: u64,
    pub execution_blocked_models: u64,
    pub rigid_selected: u64,
    pub skinned_selected: u64,
    pub animated_selected: u64,
    pub static_selected: u64,
    pub slots_required: u64,
    pub slots_with_candidates: u64,
    pub slots_selected: u64,
    pub scope_blockers: u64,
    pub execution_blockers: u64,
    pub total_blockers: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentGpuCoverage {
    pub required_slots: Vec<String>,
    pub slots: Vec<EquipmentGpuSlotCoverage>,
    pub required_variant_labels: Vec<String>,
    pub covered_variant_labels: Vec<String>,
    pub independent_structural_passed: bool,
    pub independent_selected_evidence_passed: bool,
    pub independent_full_evidence_passed: bool,
    pub independent_audit_matched_models: u64,
    pub independent_audit_candidate_models: u64,
    pub independent_audit_violations: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentGpuSlotCoverage {
    pub slot: String,
    pub candidate_models: u64,
    pub selected_models: u64,
    pub passed_models: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentGpuFactsSummary {
    pub mesh_parts: u64,
    pub skinned_mesh_parts: u64,
    pub skin_joint_references: u64,
    pub inverse_bind_matrices: u64,
    pub standard_animation_names: Vec<String>,
    pub materials_applied: u64,
    pub legacy_pass_companions: u64,
    pub outline_pass_companions: u64,
    pub assigned_texture_bindings: u64,
    pub exact_mip_chains: u64,
    pub exact_mip_levels: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentGpuBlocker {
    pub stage: String,
    pub code: String,
    pub category: Option<String>,
    pub true_name: Option<String>,
    pub relative_glb: Option<String>,
    pub detail: String,
    pub evidence: Value,
    pub disposition: String,
}

#[derive(Clone)]
pub(super) struct Candidate {
    pub(super) category: String,
    pub(super) true_name: String,
    pub(super) relative_glb: String,
    pub(super) facts: LogicalModelGpuFacts,
    pub(super) variants: BTreeSet<String>,
}

pub(super) struct ClassifiedPreviewFailure {
    pub(super) code: String,
    pub(super) detail: String,
    pub(super) runtime_error: Option<String>,
    pub(super) typed_evidence: Value,
}
