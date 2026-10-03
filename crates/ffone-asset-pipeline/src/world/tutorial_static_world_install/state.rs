use super::*;

#[derive(Clone, Debug)]
pub(super) struct RuntimeMigrationProof {
    pub(super) archive_root: PathBuf,
    pub(super) report: RuntimeWorldMigrationReport,
    pub(super) registry_by_scene: BTreeMap<String, RuntimeWorldRegistryEntry>,
}
