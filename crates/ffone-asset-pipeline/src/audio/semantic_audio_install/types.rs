use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationCertainty {
    SourceProvenance,
    TrueName,
    ExplicitSharedFallback,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CookReport {
    pub(super) schema: String,
    pub(super) build_uuid: String,
    pub(super) locale: String,
    pub(super) counts: CookCounts,
    pub(super) mappings: Vec<CookMapping>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CookCounts {
    pub(super) audio: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CookMapping {
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) native_key: String,
    pub(super) native_path: String,
    pub(super) source: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug)]
pub(super) struct MappingGroup {
    pub(super) true_name: String,
    pub(super) native_key: String,
    pub(super) native_path: String,
    pub(super) provenance: Vec<BTreeMap<String, serde_json::Value>>,
}

#[derive(Clone, Debug)]
pub(super) struct Classification {
    pub(super) category: SemanticAudioCategory,
    pub(super) owner: String,
    pub(super) proof: SemanticAudioClassificationProof,
}
