use super::*;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeferredWorldMetadata {
    pub source_path: String,
    pub reason: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ConversionMetadataRevisionReport {
    pub(super) schema: String,
    pub(super) source_build: String,
    pub(super) plan_blake3: String,
    pub(super) archived_files: u64,
    pub(super) archived_bytes: u64,
    pub(super) files: Vec<ArchivedRuntimeMetadata>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConversionMetadataRevisionPlanIdentity<'a> {
    pub(super) schema: &'static str,
    pub(super) source_build: &'a str,
    pub(super) files: &'a [ArchivedRuntimeMetadata],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CleanupTransactionPlanIdentity<'a> {
    pub(super) schema: &'static str,
    pub(super) source_build: &'a str,
    pub(super) manifest_before_blake3: &'a str,
    pub(super) manifest_after_blake3: &'a str,
    pub(super) archived: &'a [ArchivedRuntimeMetadata],
    pub(super) revision_archived: &'a [ArchivedRuntimeMetadata],
    pub(super) removal_only: &'a [ArchivedRuntimeMetadata],
    pub(super) runtime_copies: &'a [RuntimeMetadataRegistryCopy],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CleanupTransactionJournal<'a> {
    pub(super) schema: &'static str,
    pub(super) transaction_id: &'a str,
    pub(super) transaction_plan_blake3: &'a str,
    pub(super) source_build: &'a str,
    pub(super) manifest_before_blake3: &'a str,
    pub(super) manifest_after_blake3: &'a str,
    pub(super) archive_stage: Option<String>,
    pub(super) archive_target: Option<String>,
    pub(super) backup_path: String,
    pub(super) manifest_next_path: String,
    pub(super) manifest_backup_path: String,
    pub(super) archived: &'a [ArchivedRuntimeMetadata],
    pub(super) revision_archived: &'a [ArchivedRuntimeMetadata],
    pub(super) removal_only: &'a [ArchivedRuntimeMetadata],
    pub(super) runtime_copies: &'a [RuntimeMetadataRegistryCopy],
}

#[derive(Debug)]
pub(super) struct CleanupTransactionLock {
    pub(super) path: PathBuf,
    pub(super) released: bool,
}

impl CleanupTransactionLock {
    pub(super) fn release(mut self) -> Result<()> {
        fs::remove_file(&self.path).map_err(|error| io_at(&self.path, error))?;
        self.released = true;
        Ok(())
    }
}

impl Drop for CleanupTransactionLock {
    fn drop(&mut self) {
        if !self.released {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct ArchiveHistory {
    pub(super) identities: BTreeMap<String, BTreeSet<(u64, String)>>,
    pub(super) batches: Vec<Vec<ArchivedRuntimeMetadata>>,
}

impl ArchiveHistory {
    pub(super) fn add(&mut self, files: &[ArchivedRuntimeMetadata]) {
        for entry in files {
            self.identities
                .entry(entry.source_path.clone())
                .or_default()
                .insert((entry.bytes, entry.blake3.clone()));
        }
        self.batches.push(files.to_vec());
    }

    pub(super) fn contains(&self, entry: &ArchivedRuntimeMetadata) -> bool {
        self.batches
            .iter()
            .flatten()
            .any(|archived| archived == entry)
    }

    pub(super) fn contains_identity(&self, path: &str, bytes: u64, blake3: &str) -> bool {
        self.identities
            .get(path)
            .is_some_and(|identities| identities.contains(&(bytes, blake3.to_owned())))
    }

    pub(super) fn paths_below(&self, root: &str) -> BTreeSet<String> {
        let prefix = format!("{root}/");
        self.identities
            .keys()
            .filter(|path| path.starts_with(&prefix))
            .cloned()
            .collect()
    }
}

#[derive(Debug)]
pub(super) struct CleanupPlan {
    pub(super) asset_root: PathBuf,
    pub(super) archive_root: PathBuf,
    pub(super) manifest_path: PathBuf,
    pub(super) manifest: ProjectAssetManifest,
    pub(super) next_manifest: ProjectAssetManifest,
    pub(super) archived: Vec<ArchivedRuntimeMetadata>,
    pub(super) revision_archived: Vec<ArchivedRuntimeMetadata>,
    pub(super) removal_only: Vec<ArchivedRuntimeMetadata>,
    pub(super) revision_plan_blake3: Option<String>,
    pub(super) primary_archive_exists: bool,
    pub(super) runtime_copies: Vec<RuntimeMetadataRegistryCopy>,
    pub(super) deferred_world: Vec<DeferredWorldMetadata>,
    pub(super) orphan_world_payload_roots: Vec<String>,
    pub(super) already_archived_orphan_world_payload_roots: Vec<String>,
    pub(super) already_archived_proven_conversion_payload_files: u64,
    pub(super) already_archived_proven_offline_content_index_files: u64,
    pub(super) blockers: Vec<String>,
}

impl CleanupPlan {
    pub(super) fn report(
        &self,
        source_build: &str,
        mode: CleanRuntimeMetadataMode,
    ) -> CleanRuntimeMetadataReport {
        let archived_bytes = self.archived.iter().map(|entry| entry.bytes).sum();
        let revision_archived_bytes = self.revision_archived.iter().map(|entry| entry.bytes).sum();
        let removal_only_bytes = self.removal_only.iter().map(|entry| entry.bytes).sum();
        let deferred_world_bytes = self.deferred_world.iter().map(|entry| entry.bytes).sum();
        let removed_manifest_entries = self
            .archived
            .iter()
            .filter(|entry| entry.manifested)
            .count();
        let orphan_world_files = self
            .archived
            .iter()
            .filter(|entry| entry.reason == ORPHAN_WORLD_PAYLOAD_REASON)
            .collect::<Vec<_>>();
        let proven_conversion_payload_files = self
            .archived
            .iter()
            .filter(|entry| entry.reason == PROVEN_CONVERSION_PAYLOAD_REASON)
            .collect::<Vec<_>>();
        let proven_offline_content_index_files = self
            .archived
            .iter()
            .filter(|entry| entry.reason == PROVEN_OFFLINE_CONTENT_INDEX_REASON)
            .count() as u64;
        let proven_offline_content_index_bytes = self
            .archived
            .iter()
            .filter(|entry| entry.reason == PROVEN_OFFLINE_CONTENT_INDEX_REASON)
            .map(|entry| entry.bytes)
            .sum();
        CleanRuntimeMetadataReport {
            schema: CLEAN_RUNTIME_METADATA_REPORT_SCHEMA.to_owned(),
            source_build: source_build.to_owned(),
            mode,
            asset_root: "assets/game".to_owned(),
            archive_root: format!(
                "../FusionForge/work/ffone/migration-archive/{source_build}/conversion-metadata"
            ),
            apply_ready: self.blockers.is_empty(),
            world_migration_required: !self.deferred_world.is_empty(),
            requires_asset_index_regeneration: !self.archived.is_empty(),
            counts: CleanRuntimeMetadataCounts {
                archived_files: self.archived.len() as u64,
                archived_bytes,
                revision_archived_files: self.revision_archived.len() as u64,
                revision_archived_bytes,
                removal_only_files: self.removal_only.len() as u64,
                removal_only_bytes,
                removed_manifest_entries: removed_manifest_entries as u64,
                unmanifested_archived_files: self
                    .archived
                    .iter()
                    .filter(|entry| !entry.manifested)
                    .count() as u64,
                deferred_world_files: self.deferred_world.len() as u64,
                deferred_world_bytes,
                runtime_registry_files: self.runtime_copies.len() as u64,
                orphan_world_payloads: self.orphan_world_payload_roots.len() as u64,
                orphan_world_files: orphan_world_files.len() as u64,
                orphan_world_json_files: orphan_world_files
                    .iter()
                    .filter(|entry| entry.source_path.ends_with(".json"))
                    .count() as u64,
                orphan_world_bytes: orphan_world_files.iter().map(|entry| entry.bytes).sum(),
                already_archived_orphan_world_payloads: self
                    .already_archived_orphan_world_payload_roots
                    .len() as u64,
                proven_conversion_payload_files: proven_conversion_payload_files.len() as u64,
                proven_conversion_payload_bytes: proven_conversion_payload_files
                    .iter()
                    .map(|entry| entry.bytes)
                    .sum(),
                already_archived_proven_conversion_payload_files: self
                    .already_archived_proven_conversion_payload_files,
                proven_offline_content_index_files,
                proven_offline_content_index_bytes,
                already_archived_proven_offline_content_index_files: self
                    .already_archived_proven_offline_content_index_files,
                manifest_files_before: self.manifest.files.len() as u64,
                manifest_files_after: self.next_manifest.files.len() as u64,
            },
            archived: self.archived.clone(),
            revision_archived: self.revision_archived.clone(),
            removal_only: self.removal_only.clone(),
            revision_plan_blake3: self.revision_plan_blake3.clone(),
            runtime_registries: self.runtime_copies.clone(),
            deferred_world: self.deferred_world.clone(),
            orphan_world_payload_roots: self.orphan_world_payload_roots.clone(),
            already_archived_orphan_world_payload_roots: self
                .already_archived_orphan_world_payload_roots
                .clone(),
            blockers: self.blockers.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MetadataDisposition {
    Keep,
    Archive(&'static str),
    DeferWorld(&'static str),
    TutorialStatic(&'static str),
}

#[derive(Debug)]
pub(super) struct TutorialStaticGate {
    pub(super) ready: bool,
    pub(super) reason: String,
}
