use super::*;

pub const RUNTIME_WORLD_MIGRATION_REPORT_SCHEMA: &str = "ffone.runtime-world-migration-report.v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeWorldMigrationMode {
    DryRun,
    Apply,
}

#[derive(Clone, Debug)]
pub struct RuntimeWorldMigrationOptions {
    pub project_root: PathBuf,
    pub source_build: String,
    pub apply: bool,
}

impl RuntimeWorldMigrationOptions {
    #[must_use]
    pub fn new(project_root: impl Into<PathBuf>, source_build: impl Into<String>) -> Self {
        Self {
            project_root: project_root.into(),
            source_build: source_build.into(),
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
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWorldContentReference {
    pub path: String,
    pub blake3: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RewrittenRuntimeWorldMetadata {
    pub path: String,
    pub original_archive_path: String,
    pub original_bytes: u64,
    pub original_blake3: String,
    pub runtime_bytes: u64,
    pub runtime_blake3: String,
    pub removed_evidence_fields: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWorldMigrationCounts {
    pub registry_entries: u64,
    pub excluded_blocked_entries: u64,
    pub legacy_catalog_hash_drifts: u64,
    pub archived_technical_files: u64,
    pub archived_technical_bytes: u64,
    pub rewritten_runtime_files: u64,
    pub rewritten_original_bytes: u64,
    pub rewritten_runtime_bytes: u64,
    pub removed_evidence_fields: u64,
    pub removed_manifest_entries: u64,
    pub updated_manifest_entries: u64,
    pub added_manifest_entries: u64,
    pub manifest_files_before: u64,
    pub manifest_files_after: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWorldMigrationReport {
    pub schema: String,
    pub source_build: String,
    pub mode: RuntimeWorldMigrationMode,
    pub asset_root: String,
    pub archive_root: String,
    pub apply_ready: bool,
    pub registry: RuntimeWorldRegistryArtifact,
    pub counts: RuntimeWorldMigrationCounts,
    pub excluded_blocked_entries: Vec<String>,
    pub legacy_catalog_hash_drifts: Vec<LegacyWorldCatalogHashDrift>,
    pub archived: Vec<ArchivedWorldTechnicalMetadata>,
    pub rewritten: Vec<RewrittenRuntimeWorldMetadata>,
    pub blockers: Vec<String>,
}

#[derive(Debug)]
pub(super) struct PreparedRuntimeRewrite {
    pub(super) report: RewrittenRuntimeWorldMetadata,
    pub(super) bytes: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct RuntimeWorldMigrationPlan {
    pub(super) asset_root: PathBuf,
    pub(super) archive_root: PathBuf,
    pub(super) manifest_path: PathBuf,
    pub(super) manifest: ProjectAssetManifest,
    pub(super) next_manifest: ProjectAssetManifest,
    pub(super) registry_bytes: Vec<u8>,
    pub(super) registry: RuntimeWorldRegistryArtifact,
    pub(super) archived: Vec<ArchivedWorldTechnicalMetadata>,
    pub(super) rewrites: Vec<PreparedRuntimeRewrite>,
    pub(super) excluded_blocked_entries: Vec<String>,
    pub(super) legacy_catalog_hash_drifts: Vec<LegacyWorldCatalogHashDrift>,
    pub(super) blockers: Vec<String>,
}

impl RuntimeWorldMigrationPlan {
    pub(super) fn report(
        &self,
        source_build: &str,
        mode: RuntimeWorldMigrationMode,
    ) -> RuntimeWorldMigrationReport {
        RuntimeWorldMigrationReport {
            schema: RUNTIME_WORLD_MIGRATION_REPORT_SCHEMA.to_owned(),
            source_build: source_build.to_owned(),
            mode,
            asset_root: "assets/game".to_owned(),
            archive_root: format!(
                "../FusionForge/work/ffone/migration-archive/{source_build}/{WORLD_ARCHIVE_DIRECTORY}"
            ),
            apply_ready: self.blockers.is_empty(),
            registry: self.registry.clone(),
            counts: RuntimeWorldMigrationCounts {
                registry_entries: self.registry_entry_count(),
                excluded_blocked_entries: self.excluded_blocked_entries.len() as u64,
                legacy_catalog_hash_drifts: self.legacy_catalog_hash_drifts.len() as u64,
                archived_technical_files: self.archived.len() as u64,
                archived_technical_bytes: self.archived.iter().map(|entry| entry.bytes).sum(),
                rewritten_runtime_files: self.rewrites.len() as u64,
                rewritten_original_bytes: self
                    .rewrites
                    .iter()
                    .map(|entry| entry.report.original_bytes)
                    .sum(),
                rewritten_runtime_bytes: self
                    .rewrites
                    .iter()
                    .map(|entry| entry.report.runtime_bytes)
                    .sum(),
                removed_evidence_fields: self
                    .rewrites
                    .iter()
                    .map(|entry| entry.report.removed_evidence_fields)
                    .sum(),
                removed_manifest_entries: self.archived.len() as u64,
                updated_manifest_entries: self.rewrites.len() as u64,
                added_manifest_entries: 1,
                manifest_files_before: self.manifest.files.len() as u64,
                manifest_files_after: self.next_manifest.files.len() as u64,
            },
            excluded_blocked_entries: self.excluded_blocked_entries.clone(),
            legacy_catalog_hash_drifts: self.legacy_catalog_hash_drifts.clone(),
            archived: self.archived.clone(),
            rewritten: self
                .rewrites
                .iter()
                .map(|entry| entry.report.clone())
                .collect(),
            blockers: self.blockers.clone(),
        }
    }

    pub(super) fn registry_entry_count(&self) -> u64 {
        serde_json::from_slice::<RuntimeWorldRegistry>(&self.registry_bytes)
            .map(|registry| registry.entries.len() as u64)
            .unwrap_or_default()
    }
}

/// Plans or applies the minimal native runtime-world migration.
///
/// Dry-run is the default. Apply is one rollback-capable transaction which
/// archives the technical conversion JSON, preserves every rewritten original,
/// installs sanitized scene/terrain/environment JSON and publishes one
/// manifest-owned `_runtime/world.json`.
pub fn migrate_runtime_world(
    options: &RuntimeWorldMigrationOptions,
) -> Result<RuntimeWorldMigrationReport> {
    validate_source_build(&options.source_build)?;
    let project_root = fs::canonicalize(&options.project_root)
        .map_err(|error| io_at(&options.project_root, error))?;
    let requested_asset_root = project_root.join("assets").join("game");
    let asset_root = fs::canonicalize(&requested_asset_root)
        .map_err(|error| io_at(&requested_asset_root, error))?;
    if !asset_root.is_dir() || !asset_root.starts_with(&project_root) {
        return invalid("assets/game is not a directory inside the project root");
    }
    let archive_root = project_root
        .join("../FusionForge/work/ffone/migration-archive")
        .join(&options.source_build)
        .join(WORLD_ARCHIVE_DIRECTORY);
    if archive_root.exists() {
        let report =
            validate_completed_migration(&asset_root, &archive_root, &options.source_build)?;
        if options.apply {
            return invalid(
                "completed world migration is already applied and verified; rerun without --apply",
            );
        }
        return Ok(report);
    }
    let plan = build_plan(asset_root, archive_root)?;
    let dry_run = plan.report(&options.source_build, RuntimeWorldMigrationMode::DryRun);
    if !options.apply {
        return Ok(dry_run);
    }
    if !plan.blockers.is_empty() {
        return invalid(format!("apply is blocked: {}", plan.blockers.join("; ")));
    }
    apply_plan(&plan, &options.source_build)?;
    Ok(plan.report(&options.source_build, RuntimeWorldMigrationMode::Apply))
}
