use super::*;

pub const EQUIPMENT_LOGICAL_MODEL_BATCH_SCHEMA: &str =
    "ffone.equipment-logical-model-batch-publish.v1";

pub const EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE: &str =
    "equipment-logical-model-batch-report.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EquipmentLogicalModelBatchPublishOptions {
    pub source_root: PathBuf,
    pub output_root: PathBuf,
}

impl EquipmentLogicalModelBatchPublishOptions {
    pub fn new(source_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self {
            source_root: source_root.into(),
            output_root: output_root.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentLogicalModelBatchReport {
    pub schema: String,
    pub status: String,
    pub taxonomy: String,
    pub source_root: String,
    pub source_manifest: FileEvidence,
    pub output_root: String,
    pub production_assets_mutated: bool,
    pub structural_audit_passed: bool,
    pub coordinate_status: String,
    pub coordinate_mismatches: u64,
    pub skinning_basis_parity_max_error: Option<f64>,
    pub current_pose_bind_identity_deviation_max: Option<f64>,
    pub gpu_gate_pending: bool,
    pub candidate_publishable: bool,
    pub source_bundles: Vec<Value>,
    pub timing: EquipmentLogicalModelBatchTiming,
    pub counts: EquipmentLogicalModelBatchCounts,
    pub models: Vec<EquipmentLogicalModelMapping>,
    pub blockers: Vec<EquipmentLogicalModelBlocker>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentLogicalModelBatchTiming {
    pub preflight_milliseconds: u64,
    pub preparation_and_write_milliseconds: u64,
    pub structural_audit_milliseconds: u64,
    pub total_milliseconds: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentLogicalModelBatchCounts {
    pub equipment_entities: u64,
    pub entities_with_model_routes: u64,
    pub unique_routes: u64,
    pub resolved_routes: u64,
    pub source_manifest_exports: u64,
    pub discovered_source_files: u64,
    pub preflight_eligible_sources: u64,
    pub portable_collision_blocked_sources: u64,
    pub native_preparation_blocked_sources: u64,
    pub published_models: u64,
    pub upstream_blockers: u64,
    pub total_blockers: u64,
    pub categories: u64,
    pub files: u64,
    pub glbs: u64,
    pub pngs: u64,
    pub publish_reports: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentLogicalModelMapping {
    pub category: String,
    pub exact_route: String,
    pub true_name: String,
    pub source: String,
    pub source_byte_length: u64,
    pub source_sha256: String,
    pub output_glb: String,
    pub glb_blake3: String,
    pub output_publish_report: String,
    pub physical_target: Value,
    pub source_facts: Value,
    pub published_features: Value,
    pub coordinate_status: String,
    pub runtime_spawn_policy: String,
    pub skinning_basis_parity_status: String,
    pub skinning_basis_parity_max_error: Option<f64>,
    pub current_pose_bind_identity_deviation_max: Option<f64>,
    pub material_status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EquipmentLogicalModelBlocker {
    pub stage: String,
    pub code: String,
    pub upstream_code: Option<String>,
    pub category: String,
    pub exact_route: String,
    pub true_name: Option<String>,
    pub source: Option<String>,
    pub source_sha256: Option<String>,
    pub detail: String,
    pub evidence: Value,
    pub required_evidence: Vec<String>,
    pub disposition: String,
}

pub(super) struct PublishedModel {
    pub(super) mapping: EquipmentLogicalModelMapping,
    pub(super) output_files: Vec<PathBuf>,
}

pub fn publish_equipment_logical_model_batch(
    options: &EquipmentLogicalModelBatchPublishOptions,
) -> Result<EquipmentLogicalModelBatchReport> {
    let total_started = Instant::now();
    reject_existing_output(&options.output_root)?;

    let preflight_started = Instant::now();
    let preflight = preflight_batch(&options.source_root)?;
    let preflight_milliseconds = elapsed_milliseconds(preflight_started)?;
    reject_existing_output(&options.output_root)?;
    let PreflightResult {
        source_manifest_path,
        source_manifest_bytes,
        source_manifest,
        plans,
        collision_blockers,
        upstream_blockers,
    } = preflight;

    let mut staging = EquipmentBatchStagingDirectory::create(&options.output_root)?;
    let publication_started = Instant::now();
    let mut native_blockers = Vec::new();
    let mut published = Vec::new();
    for (index, plan) in plans.iter().enumerate() {
        if collision_blockers.contains_key(&index) {
            continue;
        }
        let bytes = fs::read(&plan.source).map_err(|error| io_at(&plan.source, error))?;
        if u64_count(bytes.len(), "source byte length")? != plan.export.source_byte_length
            || sha256_hex(&bytes) != plan.export.source_sha256
        {
            return equipment_error(format!(
                "source changed after global preflight: {:?}",
                plan.source
            ));
        }

        let mut semantic_directories = vec!["player", "equipment", plan.export.category.as_str()];
        if let Some(route_qualifier) = plan.export.route_qualifier.as_deref() {
            semantic_directories.push(route_qualifier);
        }
        semantic_directories.push(plan.export.true_name.as_str());
        let options = LogicalModelPublishOptions::new(&plan.source, "characters", staging.path())
            .with_semantic_directories(semantic_directories)
            .with_semantic_root_layout();
        let prepared = match prepare_logical_model(&options, &bytes) {
            Ok(prepared) => prepared,
            Err(error) => {
                native_blockers.push(native_preparation_blocker(plan, error.to_string()));
                continue;
            }
        };
        if prepared.report.contract.legacy_name != plan.export.true_name
            || Path::new(&prepared.report.contract.output_glb) != plan.output_glb
        {
            native_blockers.push(native_contract_blocker(
                plan,
                "prepared identity or semantic output path differs from global preflight",
                json!({
                    "preparedTrueName": prepared.report.contract.legacy_name,
                    "preparedOutputGlb": prepared.report.contract.output_glb,
                    "plannedOutputGlb": slash_path(&plan.output_glb),
                }),
            ));
            continue;
        }
        let prepared_files = prepared
            .relative_files()
            .map(Path::to_path_buf)
            .collect::<BTreeSet<_>>();
        let planned_files = plan.output_files.iter().cloned().collect::<BTreeSet<_>>();
        if prepared_files != planned_files {
            native_blockers.push(native_contract_blocker(
                plan,
                "prepared texture/report outputs differ from global collision preflight",
                json!({
                    "preparedFiles": prepared_files.iter().map(|path| slash_path(path)).collect::<Vec<_>>(),
                    "plannedFiles": planned_files.iter().map(|path| slash_path(path)).collect::<Vec<_>>(),
                }),
            ));
            continue;
        }

        let mapping = EquipmentLogicalModelMapping {
            category: plan.export.category.clone(),
            exact_route: plan.export.canonical_route.clone(),
            true_name: plan.export.true_name.clone(),
            source: plan.export.source_relative_path.clone(),
            source_byte_length: plan.export.source_byte_length,
            source_sha256: plan.export.source_sha256.clone(),
            output_glb: slash_path(&plan.output_glb),
            glb_blake3: prepared.report.contract.glb_blake3.clone(),
            output_publish_report: prepared.report.report_path.clone(),
            physical_target: plan.export.physical_target.clone(),
            source_facts: plan.export.facts.clone(),
            published_features: serde_json::to_value(&prepared.report.contract.published).map_err(
                |error| {
                    equipment_error_value(format!(
                        "could not serialize published feature counts: {error}"
                    ))
                },
            )?,
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
            material_status: prepared.report.material_publish.status.clone(),
        };
        write_prepared_logical_model(staging.path(), &prepared)?;
        published.push(PublishedModel {
            mapping,
            output_files: plan.output_files.clone(),
        });
    }
    let preparation_and_write_milliseconds = elapsed_milliseconds(publication_started)?;

    let mut blockers = upstream_blockers;
    blockers.extend(collision_blockers.into_values());
    blockers.extend(native_blockers);
    sort_blockers(&mut blockers);
    published.sort_by(|left, right| left.mapping.output_glb.cmp(&right.mapping.output_glb));

    let audit_started = Instant::now();
    let predicted_counts = published_file_counts(&published)?;
    let coordinate = coordinate_summary(&published);
    let mut report = EquipmentLogicalModelBatchReport {
        schema: EQUIPMENT_LOGICAL_MODEL_BATCH_SCHEMA.to_owned(),
        status: if blockers.is_empty() {
            "structural-audit-passed-runtime-coordinate-and-gpu-gates-pending".to_owned()
        } else {
            "complete-with-typed-blockers-structural-audit-passed-runtime-coordinate-and-gpu-gates-pending"
                .to_owned()
        },
        taxonomy: EQUIPMENT_TAXONOMY.to_owned(),
        source_root: slash_path(&options.source_root),
        source_manifest: FileEvidence {
            path: slash_path(&source_manifest_path),
            byte_length: u64_count(source_manifest_bytes.len(), "source manifest byte length")?,
            sha256: sha256_hex(&source_manifest_bytes),
        },
        output_root: slash_path(&options.output_root),
        production_assets_mutated: false,
        structural_audit_passed: true,
        coordinate_status: coordinate.0,
        coordinate_mismatches: coordinate.1,
        skinning_basis_parity_max_error: coordinate.2,
        current_pose_bind_identity_deviation_max: coordinate.3,
        gpu_gate_pending: true,
        candidate_publishable: false,
        source_bundles: source_manifest.bundles.clone(),
        timing: EquipmentLogicalModelBatchTiming {
            preflight_milliseconds,
            preparation_and_write_milliseconds,
            structural_audit_milliseconds: 0,
            total_milliseconds: 0,
        },
        counts: EquipmentLogicalModelBatchCounts {
            equipment_entities: source_manifest.counts.equipment_entities,
            entities_with_model_routes: source_manifest.counts.entities_with_model_routes,
            unique_routes: source_manifest.counts.unique_routes,
            resolved_routes: source_manifest.counts.resolved_routes,
            source_manifest_exports: u64_count(
                source_manifest.exported.len(),
                "manifest export count",
            )?,
            discovered_source_files: u64_count(plans.len(), "discovered source count")?,
            preflight_eligible_sources: u64_count(
                plans.len().saturating_sub(
                    blockers
                        .iter()
                        .filter(|blocker| blocker.stage == "global-output-preflight")
                        .count(),
                ),
                "preflight eligible source count",
            )?,
            portable_collision_blocked_sources: u64_count(
                blockers
                    .iter()
                    .filter(|blocker| blocker.stage == "global-output-preflight")
                    .count(),
                "portable collision blocker count",
            )?,
            native_preparation_blocked_sources: u64_count(
                blockers
                    .iter()
                    .filter(|blocker| blocker.stage == "native-glb-preparation")
                    .count(),
                "native preparation blocker count",
            )?,
            published_models: u64_count(published.len(), "published model count")?,
            upstream_blockers: source_manifest.counts.total_blockers,
            total_blockers: u64_count(blockers.len(), "total blocker count")?,
            categories: u64_count(
                published
                    .iter()
                    .map(|model| model.mapping.category.as_str())
                    .collect::<BTreeSet<_>>()
                    .len(),
                "published category count",
            )?,
            files: predicted_counts.0,
            glbs: predicted_counts.1,
            pngs: predicted_counts.2,
            publish_reports: predicted_counts.3,
        },
        models: published
            .iter()
            .map(|model| model.mapping.clone())
            .collect(),
        blockers,
    };
    write_report(staging.path(), &report)?;
    let audit = audit_logical_model_tree(staging.path())?;
    if !audit.passed {
        return equipment_error(format!(
            "staged equipment GLB tree failed structural audit with {} violations: {:?}; blockers: {:?}",
            audit.violations.len(),
            audit.violations,
            report.blockers
        ));
    }
    if audit.counts.files != report.counts.files
        || audit.counts.glbs != report.counts.glbs
        || audit.counts.pngs != report.counts.pngs
        || audit.counts.publish_reports != report.counts.publish_reports
        || audit.counts.logical_roots != report.counts.published_models
    {
        return equipment_error(format!(
            "structural audit counts differ from report: audit files/glbs/pngs/reports/roots={}/{}/{}/{}/{}, report={}/{}/{}/{}/{}",
            audit.counts.files,
            audit.counts.glbs,
            audit.counts.pngs,
            audit.counts.publish_reports,
            audit.counts.logical_roots,
            report.counts.files,
            report.counts.glbs,
            report.counts.pngs,
            report.counts.publish_reports,
            report.counts.published_models,
        ));
    }
    report.timing.structural_audit_milliseconds = elapsed_milliseconds(audit_started)?;
    report.timing.total_milliseconds = elapsed_milliseconds(total_started)?;
    rewrite_report(staging.path(), &report)?;

    // The timing-only rewrite cannot affect model structure, but re-read the
    // exact report and require byte-for-byte serde agreement before commit.
    let report_path = staging
        .path()
        .join(EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE);
    let committed_report: EquipmentLogicalModelBatchReport = serde_json::from_slice(
        &fs::read(&report_path).map_err(|error| io_at(&report_path, error))?,
    )
    .map_err(|source| PipelineError::Json {
        path: report_path.display().to_string(),
        source,
    })?;
    if committed_report != report {
        return equipment_error("staged equipment report roundtrip mismatch");
    }

    staging.publish(&options.output_root)?;
    Ok(report)
}
