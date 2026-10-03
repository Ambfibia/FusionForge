use super::*;

pub(super) struct LoadedInput {
    pub(super) env: UnityEnvironment,
    pub(super) _session_dir: Option<PathBuf>,
    pub(super) source_asset_names: BTreeSet<String>,
}

#[derive(Default)]
pub(super) struct ExtractOptions {
    pub(super) outdir: PathBuf,
    pub(super) types: HashSet<String>,
    pub(super) filters: Vec<String>,
    pub(super) dry_run: bool,
    pub(super) files: Vec<PathBuf>,
}
