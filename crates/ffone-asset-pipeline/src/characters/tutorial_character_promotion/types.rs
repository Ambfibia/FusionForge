use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct PromotionCandidate {
    pub(super) id: &'static str,
    pub(super) glb_name: &'static str,
    pub(super) category: RuntimeCharacterCategory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TutorialCharacterPromotionOptions {
    pub asset_root: PathBuf,
    pub candidate_root: PathBuf,
    pub source_root: PathBuf,
    pub gpu_evidence_root: PathBuf,
    pub apply: bool,
}

impl TutorialCharacterPromotionOptions {
    pub fn new(
        asset_root: impl Into<PathBuf>,
        candidate_root: impl Into<PathBuf>,
        source_root: impl Into<PathBuf>,
        gpu_evidence_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            asset_root: asset_root.into(),
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
pub struct TutorialCharacterPromotionBlocker {
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialCharacterPromotionProof {
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
    pub standard_animations: Vec<String>,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialCharacterPromotionCounts {
    pub candidates: u64,
    pub ready: u64,
    pub already_promoted: u64,
    pub blocked: u64,
    pub promoted_files: u64,
    pub promoted_bytes: u64,
    pub registry_models_before: u64,
    pub registry_models_after: u64,
    pub manifest_files_before: u64,
    pub manifest_files_after: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialCharacterPromotionReport {
    pub schema: String,
    pub mode: TutorialCharacterPromotionMode,
    pub applied: bool,
    pub requires_asset_index_regeneration: bool,
    pub excluded_tutorial_props: Vec<String>,
    pub missing_models_not_synthesized: Vec<String>,
    pub counts: TutorialCharacterPromotionCounts,
    pub models: Vec<TutorialCharacterPromotionModel>,
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
    pub(super) source_files: Vec<PathBuf>,
    pub(super) target_entries: Vec<ProjectAssetFile>,
    pub(super) registry_model: RuntimeCharacterModel,
    pub(super) proof: TutorialCharacterPromotionProof,
}

#[derive(Debug)]
pub(super) struct PromotionPlan {
    pub(super) asset_root: PathBuf,
    pub(super) manifest_path: PathBuf,
    pub(super) registry_path: PathBuf,
    pub(super) manifest_before: Vec<u8>,
    pub(super) registry_before: Vec<u8>,
    pub(super) manifest_after: Vec<u8>,
    pub(super) registry_after: Vec<u8>,
    pub(super) promotions: Vec<PreparedPromotion>,
    pub(super) report: TutorialCharacterPromotionReport,
}
