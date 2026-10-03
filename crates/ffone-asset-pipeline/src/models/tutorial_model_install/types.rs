use super::*;

#[derive(Clone, Debug)]
pub(super) struct PreparedFile {
    pub(super) candidate_path: String,
    pub(super) installed_path: String,
    pub(super) kind: ProjectAssetKind,
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(super) struct EvidenceRoot {
    pub(super) canonical: PathBuf,
    pub(super) files: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub(super) struct EvidenceProof {
    pub(super) root: PathBuf,
    pub(super) json_relative: String,
    pub(super) json_bytes: Vec<u8>,
    pub(super) png_relative: String,
    pub(super) png_bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(super) struct ExpectedExternalFile {
    pub(super) path: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}
