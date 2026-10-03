use super::*;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct PrimaryKey {
    pub(super) container: String,
    pub(super) path_id: u64,
    pub(super) true_name: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct FallbackKey {
    pub(super) container: String,
    pub(super) folded_true_name: String,
}

#[derive(Clone, Debug)]
pub(super) struct RussianSourceFile {
    pub(super) absolute_path: PathBuf,
    pub(super) relative_path: String,
    pub(super) container: Option<String>,
    pub(super) path_id: Option<u64>,
    pub(super) true_name: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct PendingMatch {
    pub(super) source: RussianSourceFile,
    pub(super) asset_index: usize,
    pub(super) match_mode: LocalizedVoiceMatchMode,
}
