use super::*;

#[derive(Clone, Debug)]
pub(super) struct PlannedFile {
    pub(super) source_absolute: PathBuf,
    pub(super) source_path: String,
    pub(super) target_path: String,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

#[derive(Debug)]
pub(super) struct MigrationPlan {
    pub(super) catalog: StrictAudioCatalog,
    pub(super) manifest: ProjectAssetManifest,
    pub(super) files: Vec<PlannedFile>,
    pub(super) report: AudioTaxonomyMigrationReport,
}

#[derive(Clone, Debug)]
pub(super) struct RecoveryOgg {
    pub(super) absolute_path: PathBuf,
    pub(super) relative_path: String,
    pub(super) true_name: String,
    pub(super) bytes: u64,
    pub(super) blake3: String,
    pub(super) packet_blake3: String,
}

pub(super) struct TransactionTarget {
    pub(super) target: PathBuf,
    pub(super) staged: PathBuf,
    pub(super) backup: PathBuf,
}
