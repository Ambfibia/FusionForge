use super::*;

pub const EQUIPMENT_MODEL_SOURCE_BATCH_SCHEMA: &str = "ffclient.equipment-model-source-batch.v1";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentModelSourceBatchManifest {
    pub schema: &'static str,
    pub status: String,
    pub source_index_path: String,
    pub source_index_sha256: String,
    pub semantic_plan_path: String,
    pub semantic_plan_sha256: String,
    pub source_root: String,
    pub manifest_path: String,
    pub taxonomy: String,
    pub benchmark_route_limit: Option<u64>,
    pub timing: EquipmentBatchTiming,
    pub counts: EquipmentModelSourceBatchCounts,
    pub categories: BTreeMap<String, u64>,
    pub bundles: Vec<EquipmentBundleProof>,
    pub exported: Vec<EquipmentModelSourceExported>,
    pub blockers: Vec<EquipmentModelSourceBlocker>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentModelSourceBatchCounts {
    pub equipment_entities: u64,
    pub entities_with_model_routes: u64,
    pub unique_routes: u64,
    pub resolved_routes: u64,
    pub selected_resolved_routes: u64,
    pub missing_routes: u64,
    pub ambiguous_routes: u64,
    pub staged_route_sources: u64,
    pub exported_physical_models: u64,
    pub proven_alias_routes: u64,
    pub source_stage_blockers: u64,
    pub true_name_collision_blockers: u64,
    pub total_blockers: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentModelSourceExported {
    pub category: String,
    pub true_name: String,
    pub safe_true_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_qualifier: Option<String>,
    pub source_relative_path: String,
    pub source_byte_length: u64,
    pub source_sha256: String,
    pub canonical_route: String,
    pub normalized_route: String,
    pub proven_alias_routes: Vec<String>,
    pub table_rows: Vec<EquipmentTableRowProof>,
    pub owner: EquipmentContainerOwner,
    pub physical_target: EquipmentPhysicalTarget,
    pub facts: EquipmentSourceFacts,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentModelSourceBlocker {
    pub code: String,
    pub category: String,
    pub exact_route: String,
    pub normalized_route: String,
    pub table_rows: Vec<EquipmentTableRowProof>,
    pub owners: Vec<EquipmentContainerOwner>,
    pub detail: String,
    pub evidence: JsonValue,
    pub required_evidence: Vec<String>,
    pub disposition: String,
}

/// Exports every exact equipment NIF route from a semantic table plan.
///
/// Both the output directory and its sibling `<name>.manifest.json` must be
/// fresh. Missing/ambiguous routes and lossy source conversions are preserved
/// as typed blockers; no name or asset fallback is attempted.
pub fn export_equipment_model_sources_batch(
    input: impl AsRef<Path>,
    semantic_plan_path: impl AsRef<Path>,
    output_root: impl AsRef<Path>,
) -> Result<EquipmentModelSourceBatchManifest, String> {
    export_equipment_model_sources_batch_limited(input, semantic_plan_path, output_root, None)
}

pub fn export_equipment_model_sources_batch_limited(
    input: impl AsRef<Path>,
    semantic_plan_path: impl AsRef<Path>,
    output_root: impl AsRef<Path>,
    benchmark_route_limit: Option<usize>,
) -> Result<EquipmentModelSourceBatchManifest, String> {
    let total_started = Instant::now();
    let planning_started = Instant::now();
    let bundle_index_path = resolve_bundle_index_input(input.as_ref())?;
    let project_dir = project_dir_for_bundle_index(&bundle_index_path)?;
    let semantic_plan_path = semantic_plan_path.as_ref();
    let output_root = output_root.as_ref();
    let manifest_path = batch_manifest_path(output_root)?;
    reject_existing_destination(output_root, &manifest_path)?;

    let index_bytes = fs::read(&bundle_index_path).map_err(|err| {
        format!(
            "could not read bundle index {}: {err}",
            bundle_index_path.display()
        )
    })?;
    let semantic_bytes = fs::read(semantic_plan_path).map_err(|err| {
        format!(
            "could not read semantic plan {}: {err}",
            semantic_plan_path.display()
        )
    })?;
    let index: BundleIndex = serde_json::from_slice(&index_bytes)
        .map_err(|err| format!("could not parse {}: {err}", bundle_index_path.display()))?;
    let semantic: SemanticPlan = serde_json::from_slice(&semantic_bytes)
        .map_err(|err| format!("could not parse {}: {err}", semantic_plan_path.display()))?;
    if semantic.schema != SEMANTIC_PLAN_SCHEMA {
        return Err(format!(
            "semantic plan schema must be {SEMANTIC_PLAN_SCHEMA:?}, got {:?}",
            semantic.schema
        ));
    }

    let (route_plans, equipment_entities, entities_with_routes, categories) =
        collect_route_plans(&semantic)?;
    let occurrences = collect_route_occurrences(index);
    let (mut candidates, mut blockers) = resolve_routes(&route_plans, &occurrences);
    candidates.sort_by(|left, right| {
        portable_key(&left.owner.bundle_path)
            .cmp(&portable_key(&right.owner.bundle_path))
            .then_with(|| left.route.category.cmp(&right.route.category))
            .then_with(|| {
                left.route
                    .normalized_route
                    .cmp(&right.route.normalized_route)
            })
    });
    sort_blockers(&mut blockers);
    let all_resolved_routes = candidates.len();
    if let Some(limit) = benchmark_route_limit {
        if limit == 0 {
            return Err("equipment benchmark route limit must be positive".to_string());
        }
        candidates.truncate(limit);
    }
    let planning_milliseconds = elapsed_milliseconds(planning_started);

    reject_existing_destination(output_root, &manifest_path)?;
    let (staging_root, staging_manifest) = staging_paths(output_root, &manifest_path)?;
    fs::create_dir(&staging_root).map_err(|err| {
        format!(
            "could not create equipment source staging directory {}: {err}",
            staging_root.display()
        )
    })?;
    let route_stage = staging_root.join(".route-stage");
    fs::create_dir(&route_stage)
        .map_err(|err| format!("could not create {}: {err}", route_stage.display()))?;

    let staged_result = stage_route_sources(&candidates, &project_dir, &route_stage);
    let mut staged = match staged_result {
        Ok(staged) => staged,
        Err(err) => {
            remove_staging(&staging_root, &staging_manifest);
            return Err(err);
        }
    };
    blockers.append(&mut staged.blockers);
    let staged_route_sources = staged.routes.len();
    let bundle_warmups = staged.bundle_warmups;
    let per_route = staged.per_route;

    let publication = publish_staged_routes(&staging_root, staged.routes)?;
    blockers.extend(publication.blockers);
    sort_blockers(&mut blockers);
    fs::remove_dir_all(&route_stage)
        .map_err(|err| format!("could not remove {}: {err}", route_stage.display()))?;

    let selected_resolved_routes = candidates.len();
    let missing_routes = blockers
        .iter()
        .filter(|blocker| blocker.code == "equipmentRouteMissingFromBundleIndex")
        .count();
    let ambiguous_routes = blockers
        .iter()
        .filter(|blocker| blocker.code == "equipmentRouteAmbiguousInBundleIndex")
        .count();
    let source_stage_blockers = blockers
        .iter()
        .filter(|blocker| blocker.code == "equipmentSourceStageExportFailed")
        .count();
    let true_name_collision_blockers = blockers
        .iter()
        .filter(|blocker| blocker.code == "distinctPhysicalEquipmentTrueNameCollision")
        .count();
    let proven_alias_routes = publication
        .exported
        .iter()
        .map(|entry| entry.proven_alias_routes.len())
        .sum::<usize>();
    let bundles = bundle_proofs(&candidates)?;
    let status = if benchmark_route_limit.is_some() {
        if blockers.is_empty() {
            "benchmark-subset-complete".to_string()
        } else {
            "benchmark-subset-complete-with-typed-blockers".to_string()
        }
    } else if blockers.is_empty() {
        "complete".to_string()
    } else {
        "complete-with-typed-blockers".to_string()
    };
    let environment_warmup_milliseconds = bundle_warmups
        .iter()
        .map(|timing| timing.milliseconds)
        .sum();
    let route_export_milliseconds = per_route.iter().map(|timing| timing.milliseconds).sum();
    let prefix_summaries = [50usize, 100usize, per_route.len()]
        .into_iter()
        .filter(|count| *count > 0 && *count <= per_route.len())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|count| timing_summary(&per_route[..count]))
        .collect();
    let manifest = EquipmentModelSourceBatchManifest {
        schema: EQUIPMENT_MODEL_SOURCE_BATCH_SCHEMA,
        status,
        source_index_path: slash_path(&bundle_index_path),
        source_index_sha256: sha256_hex(&index_bytes),
        semantic_plan_path: slash_path(semantic_plan_path),
        semantic_plan_sha256: sha256_hex(&semantic_bytes),
        source_root: slash_path(output_root),
        manifest_path: slash_path(&manifest_path),
        taxonomy:
            "characters/player/equipment/{hat,mask,glasses,back,head,shirt,pants,shoes,weapon,vehicle}/[<xdt-route-qualifier>/]<true-name>"
                .to_string(),
        benchmark_route_limit: benchmark_route_limit.map(u64_count).transpose()?,
        timing: EquipmentBatchTiming {
            planning_milliseconds,
            environment_warmup_milliseconds,
            route_export_milliseconds,
            total_milliseconds: elapsed_milliseconds(total_started),
            bundle_warmups,
            per_route,
            prefix_summaries,
        },
        counts: EquipmentModelSourceBatchCounts {
            equipment_entities: u64_count(equipment_entities)?,
            entities_with_model_routes: u64_count(entities_with_routes)?,
            unique_routes: u64_count(route_plans.len())?,
            resolved_routes: u64_count(all_resolved_routes)?,
            selected_resolved_routes: u64_count(selected_resolved_routes)?,
            missing_routes: u64_count(missing_routes)?,
            ambiguous_routes: u64_count(ambiguous_routes)?,
            staged_route_sources: u64_count(staged_route_sources)?,
            exported_physical_models: u64_count(publication.exported.len())?,
            proven_alias_routes: u64_count(proven_alias_routes)?,
            source_stage_blockers: u64_count(source_stage_blockers)?,
            true_name_collision_blockers: u64_count(true_name_collision_blockers)?,
            total_blockers: u64_count(blockers.len())?,
        },
        categories,
        bundles,
        exported: publication.exported,
        blockers,
    };
    if benchmark_route_limit.is_some() {
        if let Some(summary) = manifest.timing.prefix_summaries.last() {
            eprintln!(
                "equipment benchmark timing: routes={} median={}ms p95={}ms min={}ms max={}ms routeTotal={}ms total={}ms",
                summary.routes,
                summary.median_milliseconds,
                summary.p95_milliseconds,
                summary.min_milliseconds,
                summary.max_milliseconds,
                summary.total_milliseconds,
                manifest.timing.total_milliseconds,
            );
        }
    }
    write_new_json(&staging_manifest, &manifest)?;
    if let Err(err) = rename_with_transient_permission_retry(&staging_root, output_root) {
        remove_staging(&staging_root, &staging_manifest);
        return Err(format!(
            "could not commit equipment source root {}: {err}",
            output_root.display()
        ));
    }
    if let Err(err) = rename_with_transient_permission_retry(&staging_manifest, &manifest_path) {
        let rollback = rename_with_transient_permission_retry(output_root, &staging_root);
        remove_staging(&staging_root, &staging_manifest);
        return Err(match rollback {
            Ok(()) => format!(
                "could not commit equipment source manifest {}: {err}; source root was rolled back",
                manifest_path.display()
            ),
            Err(rollback_err) => format!(
                "could not commit equipment source manifest {}: {err}; CRITICAL rollback of {} also failed: {rollback_err}",
                manifest_path.display(),
                output_root.display()
            ),
        });
    }
    Ok(manifest)
}
