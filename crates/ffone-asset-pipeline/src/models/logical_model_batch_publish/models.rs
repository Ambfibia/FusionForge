use super::*;

pub const LOGICAL_MODEL_BATCH_REPORT_SCHEMA: &str = "ffone.logical-model-batch-publish-report.v4";

pub const LOGICAL_MODEL_BATCH_REPORT_FILE: &str = "logical-model-batch-report.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalModelBatchPublishOptions {
    pub source_root: PathBuf,
    pub output_root: PathBuf,
}

impl LogicalModelBatchPublishOptions {
    pub fn new(source_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            source_root: source_root.into(),
            output_root: output_root.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelBatchPublishReport {
    pub schema: String,
    pub status: String,
    pub structural_audit_passed: bool,
    pub coordinate_status: String,
    pub coordinate_artifact_space_proven: u64,
    pub coordinate_runtime_spawn_policy_pending: u64,
    pub coordinate_mismatches: u64,
    pub skinning_basis_parity_max_error: Option<f64>,
    pub current_pose_bind_identity_deviation_max: Option<f64>,
    pub gpu_gate_pending: bool,
    pub candidate_publishable: bool,
    pub counts: LogicalModelBatchCounts,
    pub models: Vec<LogicalModelBatchMapping>,
    pub blockers: Vec<LogicalModelBatchBlocker>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelBatchCounts {
    pub planned_sources: u64,
    pub sources: u64,
    pub blocked_sources: u64,
    pub families: u64,
    pub semantic_directories: u64,
    pub files: u64,
    pub glbs: u64,
    pub pngs: u64,
    pub publish_reports: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelBatchMapping {
    pub source: String,
    pub source_sha256: String,
    pub family: String,
    pub semantic_directories: Vec<String>,
    pub logical_name: String,
    pub output_glb: String,
    pub coordinate_status: String,
    pub runtime_spawn_policy: String,
    pub skinning_basis_parity_status: String,
    pub skinning_basis_parity_max_error: Option<f64>,
    pub current_pose_bind_identity_deviation_max: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelBatchBlocker {
    pub code: String,
    pub source: String,
    pub source_sha256: String,
    pub family: String,
    pub semantic_directories: Vec<String>,
    pub logical_name: String,
    pub detail: String,
    pub evidence: Value,
    pub required_evidence: Vec<String>,
    pub disposition: String,
}

/// Publishes every source only after a complete global path preflight.
///
/// The input layout is strictly
/// `<family>/<exact-route-stem>[/...]/<minimally-safe-true-m_Name>.source.json`.
/// The first directory is the family; every remaining parent directory is
/// mirrored below `models/<family>/` as semantic ownership context.
pub fn publish_logical_model_batch(
    options: &LogicalModelBatchPublishOptions,
) -> Result<LogicalModelBatchPublishReport> {
    if fs::symlink_metadata(&options.output_root).is_ok() {
        return Err(PipelineError::OutputExists(options.output_root.clone()));
    }

    // This is the only phase before staging creation. It reads every source,
    // proves its exact root identity and reserves every portable output path.
    let plans = preflight_batch(&options.source_root)?;
    let mut staging = BatchStagingDirectory::create(&options.output_root)?;

    let publishable_count = plans.iter().filter(|plan| plan.blocker.is_none()).count();
    let mut coordinate_evidence = Vec::with_capacity(publishable_count);
    for plan in plans.iter().filter(|plan| plan.blocker.is_none()) {
        let bytes = fs::read(&plan.source).map_err(|error| io_at(&plan.source, error))?;
        if sha256_hex(&bytes) != plan.source_sha256 {
            return batch_error(format!(
                "source changed after global preflight: {:?}",
                plan.source
            ));
        }
        let publish_options =
            LogicalModelPublishOptions::new(&plan.source, &plan.family, staging.path())
                .with_semantic_directories(plan.semantic_directories.iter().cloned());
        let prepared = prepare_logical_model(&publish_options, &bytes).map_err(|error| {
            batch_error_value(format!(
                "failed to prepare source {:?}: {error}",
                plan.source_relative
            ))
        })?;
        if prepared.report.contract.legacy_name != plan.logical_name
            || Path::new(&prepared.report.contract.output_glb) != plan.output_glb
        {
            return batch_error(format!(
                "prepared model identity/path differs from global preflight for {:?}",
                plan.source
            ));
        }
        let prepared_files = prepared
            .relative_files()
            .map(Path::to_path_buf)
            .collect::<BTreeSet<_>>();
        let planned_files = plan.output_files.iter().cloned().collect::<BTreeSet<_>>();
        if prepared_files != planned_files {
            return batch_error(format!(
                "prepared texture/report outputs differ from global preflight for {:?}",
                plan.source
            ));
        }
        coordinate_evidence.push(BatchCoordinateEvidence {
            coordinate_status: prepared.report.coordinate_audit.status.clone(),
            runtime_spawn_policy: prepared
                .report
                .coordinate_audit
                .runtime_spawn_policy
                .clone(),
            skinning_basis_parity_status: prepared
                .report
                .coordinate_audit
                .skinning_basis_parity_status
                .clone(),
            skinning_basis_parity_max_error: prepared
                .report
                .coordinate_audit
                .skinning_basis_parity_max_error,
            current_pose_bind_identity_deviation_max: prepared
                .report
                .coordinate_audit
                .current_pose_bind_identity_deviation_max,
        });
        write_prepared_logical_model(staging.path(), &prepared)?;
    }

    let report = build_report(&plans, &coordinate_evidence)?;
    write_batch_report(staging.path(), &report)?;
    let audit = audit_logical_model_tree(staging.path())?;
    if !audit.passed {
        return batch_error(format!(
            "staged batch failed structural audit with {} violations",
            audit.violations.len()
        ));
    }
    if audit.counts.files != report.counts.files
        || audit.counts.glbs != report.counts.glbs
        || audit.counts.pngs != report.counts.pngs
        || audit.counts.publish_reports != report.counts.publish_reports
        || audit.counts.logical_roots != report.counts.sources
    {
        return batch_error(format!(
            "structural audit counts differ from batch report: audit files/glbs/pngs/reports/roots={}/{}/{}/{}/{}, batch={}/{}/{}/{}/{}",
            audit.counts.files,
            audit.counts.glbs,
            audit.counts.pngs,
            audit.counts.publish_reports,
            audit.counts.logical_roots,
            report.counts.files,
            report.counts.glbs,
            report.counts.pngs,
            report.counts.publish_reports,
            report.counts.sources
        ));
    }

    staging.publish(&options.output_root)?;
    Ok(report)
}
