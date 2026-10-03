use super::*;

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
pub(super) struct NamedOgg {
    pub(super) absolute_path: PathBuf,
    pub(super) relative_path: String,
    pub(super) container: String,
    pub(super) path_id: u64,
    pub(super) true_name: String,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AmbiguousKeyProof {
    pub(super) logical_key: String,
    pub(super) true_name: String,
    pub(super) english_source: String,
    pub(super) english_blake3: String,
    pub(super) russian_candidates: Vec<QuarantineFileProof>,
    pub(super) reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QuarantineFileProof {
    pub(super) source: String,
    pub(super) quarantine_path: String,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IgnoredFileProof {
    pub(super) source: String,
    pub(super) quarantine_path: String,
    pub(super) reason: String,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

pub(super) struct TransactionTarget {
    pub(super) target: PathBuf,
    pub(super) staged: Option<PathBuf>,
    pub(super) backup: PathBuf,
}
