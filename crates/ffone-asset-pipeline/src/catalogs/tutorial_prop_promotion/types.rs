use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct PromotionCandidate {
    pub(super) id: &'static str,
    pub(super) glb_name: &'static str,
    pub(super) runtime_source: &'static str,
}

impl PromotionCandidate {
    pub(super) fn source_root(self) -> String {
        format!("{SOURCE_PACKAGE_ROOT}/{}", self.id)
    }

    pub(super) fn target_root(self) -> String {
        match self.id {
            "etc_domeglass_04" => "objects/structures/etc_domeglass_04_01_default_etc_domglass/models/etc_dome_glass_04".to_owned(),
            "npc_building" => "objects/collections/npc_building/models/npc_building".to_owned(),
            _ => unreachable!("promotion candidates are a closed reviewed set"),
        }
    }

    pub(super) fn candidate_glb(self) -> String {
        format!("models/mob/{}/{}", self.id, self.glb_name)
    }

    pub(super) fn source_glb(self) -> String {
        format!("{}/{}", self.source_root(), self.glb_name)
    }

    pub(super) fn target_glb(self) -> String {
        format!("{}/{}", self.target_root(), self.glb_name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TutorialPropPromotionOptions {
    pub project_root: PathBuf,
    pub candidate_root: PathBuf,
    pub source_root: PathBuf,
    pub gpu_evidence_root: PathBuf,
    pub apply: bool,
}

impl TutorialPropPromotionOptions {
    pub fn new(
        project_root: impl Into<PathBuf>,
        candidate_root: impl Into<PathBuf>,
        source_root: impl Into<PathBuf>,
        gpu_evidence_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            candidate_root: candidate_root.into(),
            source_root: source_root.into(),
            gpu_evidence_root: gpu_evidence_root.into(),
            apply: false,
        }
    }

    #[must_use]
    pub fn with_apply(mut self, apply: bool) -> Self {
        self.apply = apply;
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialPropPromotionBlocker {
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialPropPromotionProof {
    pub candidate_glb: String,
    pub glb_blake3: String,
    pub source_document: String,
    pub source_sha256: String,
    pub publish_report: String,
    pub publish_report_sha256: String,
    pub gpu_evidence_json: String,
    pub gpu_evidence_json_sha256: String,
    pub gpu_screenshot_png: String,
    pub gpu_screenshot_sha256: String,
    pub runtime_reference: TutorialPropRuntimeReferenceProof,
    pub mesh_parts: u64,
    pub materials_applied: u64,
    pub external_texture_bindings: u64,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialPropPromotionCounts {
    pub candidates: u64,
    pub ready: u64,
    pub already_promoted: u64,
    pub blocked: u64,
    pub promoted_files: u64,
    pub promoted_bytes: u64,
    pub runtime_references_updated: u64,
    pub manifest_files_before: u64,
    pub manifest_files_after: u64,
    pub external_manifest_entries_preserved: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialPropPromotionReport {
    pub schema: String,
    pub mode: TutorialPropPromotionMode,
    pub applied: bool,
    pub requires_asset_index_regeneration: bool,
    pub exact_allowlist: Vec<String>,
    pub external_texture_duplicates_untouched: bool,
    pub counts: TutorialPropPromotionCounts,
    pub models: Vec<TutorialPropPromotionModel>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileIdentity {
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

#[derive(Clone, Debug)]
pub(super) struct PreparedPromotion {
    pub(super) candidate: PromotionCandidate,
    pub(super) source_root: String,
    pub(super) target_root: String,
    pub(super) source_tree: BTreeMap<String, FileIdentity>,
    pub(super) target_entries: Vec<ProjectAssetFile>,
    pub(super) runtime_source_path: PathBuf,
    pub(super) runtime_before: Vec<u8>,
    pub(super) runtime_after: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct PromotionPlan {
    pub(super) project_root: PathBuf,
    pub(super) asset_root: PathBuf,
    pub(super) manifest_path: PathBuf,
    pub(super) manifest_before: Vec<u8>,
    pub(super) manifest_after: Vec<u8>,
    pub(super) promotions: Vec<PreparedPromotion>,
    pub(super) report: TutorialPropPromotionReport,
}
