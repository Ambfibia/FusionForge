use super::*;

pub const CLEAN_RUNTIME_METADATA_REPORT_SCHEMA: &str = "ffone.clean-runtime-metadata-report.v1";

pub const CLEAN_RUNTIME_METADATA_REPORT_FILE: &str = "report.json";

pub(super) const CLEAN_RUNTIME_METADATA_REVISION_REPORT_SCHEMA: &str =
    "ffone.conversion-metadata-revision-report.v1";

pub(super) const CLEAN_RUNTIME_METADATA_REVISION_PLAN_SCHEMA: &str =
    "ffone.conversion-metadata-revision-plan.v1";

pub(super) const CLEAN_RUNTIME_METADATA_TRANSACTION_SCHEMA: &str =
    "ffone.clean-runtime-metadata-transaction.v1";

pub(super) const CLEAN_RUNTIME_METADATA_TRANSACTION_DIRECTORY_PREFIX: &str =
    ".clean-runtime-metadata-transaction-";

pub(super) const CLEAN_RUNTIME_METADATA_LOCK_FILE: &str = ".clean-runtime-metadata-lock";

pub(super) const CHARACTER_RUNTIME_REGISTRIES: &[&str] = &[
    "_runtime/characters.json",
    "characters/_runtime/characters.json",
    "characters/registry.json",
];

pub(super) const ACTIVE_RUNTIME_CATALOGS: &[&str] = &[
    "data/character_creation/appearance.json",
    "data/character_creation/avatar_items.json",
    "data/character_creation/runtime_textures.json",
    "localization/catalog.json",
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanRuntimeMetadataMode {
    DryRun,
    Apply,
}

#[derive(Clone, Debug)]
pub struct CleanRuntimeMetadataOptions {
    pub project_root: PathBuf,
    pub source_build: String,
    pub apply: bool,
}

impl CleanRuntimeMetadataOptions {
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
#[serde(rename_all = "camelCase")]
pub struct ArchivedRuntimeMetadata {
    pub source_path: String,
    pub archive_path: String,
    pub reason: String,
    pub bytes: u64,
    pub blake3: String,
    pub manifested: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest_source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest_kind: Option<ProjectAssetKind>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanRuntimeMetadataCounts {
    pub archived_files: u64,
    pub archived_bytes: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub revision_archived_files: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub revision_archived_bytes: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub removal_only_files: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub removal_only_bytes: u64,
    pub removed_manifest_entries: u64,
    pub unmanifested_archived_files: u64,
    pub deferred_world_files: u64,
    pub deferred_world_bytes: u64,
    pub runtime_registry_files: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub orphan_world_payloads: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub orphan_world_files: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub orphan_world_json_files: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub orphan_world_bytes: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub already_archived_orphan_world_payloads: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub proven_conversion_payload_files: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub proven_conversion_payload_bytes: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub already_archived_proven_conversion_payload_files: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub proven_offline_content_index_files: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub proven_offline_content_index_bytes: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub already_archived_proven_offline_content_index_files: u64,
    pub manifest_files_before: u64,
    pub manifest_files_after: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanRuntimeMetadataReport {
    pub schema: String,
    pub source_build: String,
    pub mode: CleanRuntimeMetadataMode,
    pub asset_root: String,
    pub archive_root: String,
    pub apply_ready: bool,
    pub world_migration_required: bool,
    pub requires_asset_index_regeneration: bool,
    pub counts: CleanRuntimeMetadataCounts,
    pub archived: Vec<ArchivedRuntimeMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revision_archived: Vec<ArchivedRuntimeMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removal_only: Vec<ArchivedRuntimeMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_plan_blake3: Option<String>,
    pub runtime_registries: Vec<RuntimeMetadataRegistryCopy>,
    pub deferred_world: Vec<DeferredWorldMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub orphan_world_payload_roots: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub already_archived_orphan_world_payload_roots: Vec<String>,
    pub blockers: Vec<String>,
}

pub(super) fn runtime_world_references_root(registry: &RuntimeWorldRegistry, id: &str, root: &str) -> bool {
    let prefix = format!("{root}/");
    registry.entries.iter().any(|entry| {
        entry.id == id
            || entry.scene.path.starts_with(&prefix)
            || entry.terrain.path.starts_with(&prefix)
            || entry
                .environment
                .as_ref()
                .is_some_and(|environment| environment.path.starts_with(&prefix))
    })
}

/// Plans or applies the conversion-metadata archive.
///
/// Dry-run is the default and performs no writes. `--apply` archives only the
/// proven conversion-only set. Tutorial static conversion JSON is eligible only
/// when the manifested runtime-world registry pins every corresponding merged
/// scene. Other world evidence remains deferred.
pub fn clean_runtime_metadata(
    options: &CleanRuntimeMetadataOptions,
) -> Result<CleanRuntimeMetadataReport> {
    validate_source_build(&options.source_build)?;
    let project_root = fs::canonicalize(&options.project_root)
        .map_err(|error| io_at(&options.project_root, error))?;
    if !project_root.is_dir() {
        return invalid("project root is not a directory");
    }
    let asset_root_requested = project_root.join("assets").join("game");
    let asset_root = fs::canonicalize(&asset_root_requested)
        .map_err(|error| io_at(&asset_root_requested, error))?;
    if !asset_root.is_dir() || !asset_root.starts_with(&project_root) {
        return invalid("assets/game is not a directory inside the project root");
    }
    let archive_root = project_root
        .join("../FusionForge/work/ffone/migration-archive")
        .join(&options.source_build)
        .join("conversion-metadata");
    let archive_history = if archive_root.exists() {
        validate_completed_archive(&archive_root, &options.source_build)?
    } else {
        ArchiveHistory::default()
    };
    let plan = build_plan(
        asset_root,
        archive_root,
        &options.source_build,
        &archive_history,
    )?;
    let dry_run = plan.report(&options.source_build, CleanRuntimeMetadataMode::DryRun);
    if !options.apply {
        return Ok(dry_run);
    }
    if !plan.blockers.is_empty() {
        return invalid(format!("apply is blocked: {}", plan.blockers.join("; ")));
    }
    if plan.archived.is_empty() && plan.runtime_copies.is_empty() {
        return Ok(plan.report(&options.source_build, CleanRuntimeMetadataMode::Apply));
    }
    apply_plan(&plan, &options.source_build)?;
    Ok(plan.report(&options.source_build, CleanRuntimeMetadataMode::Apply))
}

pub(super) fn verify_committed_cleanup_state(
    plan: &CleanupPlan,
    source_build: &str,
    archive_target: Option<&Path>,
) -> Result<()> {
    let manifest_bytes =
        fs::read(&plan.manifest_path).map_err(|error| io_at(&plan.manifest_path, error))?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: plan.manifest_path.display().to_string(),
            source,
        })?;
    if manifest != plan.next_manifest {
        return invalid("committed asset manifest does not match the cleanup plan");
    }
    for entry in &plan.archived {
        let source = plan.asset_root.join(native_path(&entry.source_path)?);
        if source.exists() {
            return invalid(format!(
                "committed archive source still exists in the live tree: {}",
                source.display()
            ));
        }
    }
    for copy in &plan.runtime_copies {
        validate_file_identity(
            &plan.asset_root.join(native_path(&copy.runtime_path)?),
            copy.bytes,
            &copy.blake3,
            "committed runtime registry",
        )?;
    }
    if let Some(target) = archive_target {
        let staged_files = if plan.primary_archive_exists {
            &plan.revision_archived
        } else {
            &plan.archived
        };
        for entry in staged_files {
            validate_file_identity(
                &target.join(native_path(&entry.archive_path)?),
                entry.bytes,
                &entry.blake3,
                "published immutable archive",
            )?;
        }
        for metadata in [
            CLEAN_RUNTIME_METADATA_INDEX_FILE,
            CLEAN_RUNTIME_METADATA_REPORT_FILE,
        ] {
            let path = target.join(metadata);
            if !path.is_file() {
                return invalid(format!(
                    "published immutable archive is missing {}",
                    path.display()
                ));
            }
        }
    }
    validate_completed_archive(&plan.archive_root, source_build)?;
    Ok(())
}
