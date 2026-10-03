//! Transactional batch export of ownership-proven logical-model sources.
//!
//! This is an offline editor operation. It deliberately delegates every model
//! decode to the existing exact single-root exporter and only adds planning,
//! naming, collision preflight and fresh-directory publication around it.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ffone_skinned_model::model_relative_path;
use serde::Serialize;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::{
    logical_model_catalog::{normalize_logical_model_route, LogicalModelOwner},
    logical_model_export_plan::{
        plan_logical_model_exports_from_path, LogicalModelExportPlan,
        LogicalModelExportPlanOptions, LogicalModelPhysicalFingerprint, LogicalModelRootStatus,
        ResolvedObjectKeyEvidence,
    },
};

pub const LOGICAL_MODEL_SOURCE_BATCH_SCHEMA: &str = "ffclient.logical-model-source-batch.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelSourceBatchManifest {
    pub schema: &'static str,
    pub status: &'static str,
    pub source_index_path: String,
    pub source_root: String,
    pub manifest_path: String,
    pub full_plan_ownership_scan_complete: bool,
    pub counts: LogicalModelSourceBatchCounts,
    pub exported: Vec<LogicalModelSourceBatchExported>,
    pub skipped: Vec<LogicalModelSourceBatchSkipped>,
    pub blocked: Vec<LogicalModelSourceBatchBlocked>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelSourceBatchCounts {
    pub planned_logical_root_count: usize,
    pub ready_self_contained_root_count: usize,
    pub exported_count: usize,
    pub skipped_root_count: usize,
    pub blocked_entry_count: usize,
    pub unresolved_reference_count: usize,
    pub full_plan_blocker_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelSourceBatchExported {
    pub family: String,
    pub semantic_directories: Vec<String>,
    pub true_name: String,
    pub source_relative_path: String,
    pub source_byte_length: u64,
    pub source_sha256: String,
    pub exact_route: String,
    pub normalized_route: String,
    pub source_ownership: LogicalModelSourceOwnershipEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelSourceOwnershipEvidence {
    pub ownership_evidence: String,
    pub self_contained_proof: String,
    pub canonical_owner: LogicalModelOwner,
    pub physical_alias_owners: Vec<LogicalModelOwner>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physical_fingerprint: Option<LogicalModelPhysicalFingerprint>,
    pub root_game_object: ResolvedObjectKeyEvidence,
    pub root_transform: ResolvedObjectKeyEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelSourceBatchSkipped {
    pub exact_route: String,
    pub normalized_route: String,
    pub code: String,
    pub detail: String,
    pub ownership_evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelSourceBatchBlocked {
    pub subject_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact_route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_route: Option<String>,
    pub code: String,
    pub detail: String,
    pub owners: Vec<LogicalModelOwner>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ownership_evidence: Option<String>,
}

#[derive(Debug, Clone)]
struct PreparedCandidate {
    family: String,
    semantic_directories: Vec<String>,
    true_name: String,
    relative_source_path: PathBuf,
    exact_route: String,
    normalized_route: String,
    bundle_path: String,
    ownership: LogicalModelSourceOwnershipEvidence,
}

#[derive(Debug)]
struct PreparedBatch {
    candidates: Vec<PreparedCandidate>,
    skipped: Vec<LogicalModelSourceBatchSkipped>,
    blocked: Vec<LogicalModelSourceBatchBlocked>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RootEligibility {
    Candidate,
    SkippedNotSelfContained,
    Blocked,
}

/// Exports every `ready` self-contained root from a newly computed full plan.
///
/// `input` is either an FusionForge client-project directory or its exact
/// `cache/bundle-index.json`. `output_root` and its sibling manifest must not
/// exist. The source root contains only
/// `<family>/<exact-route-semantic-directories>/<true-name>.source.json` so it
/// can be consumed directly by the native FFOne batch publisher; the compact
/// provenance manifest is written beside it as
/// `<output-name>.manifest.json`.
pub fn export_logical_model_sources_batch(
    input: impl AsRef<Path>,
    output_root: impl AsRef<Path>,
) -> Result<LogicalModelSourceBatchManifest, String> {
    let bundle_index_path = resolve_bundle_index_input(input.as_ref())?;
    let project_dir = project_dir_for_bundle_index(&bundle_index_path)?;
    let output_root = output_root.as_ref();
    let manifest_path = batch_manifest_path(output_root)?;

    reject_existing_destination(output_root, &manifest_path)?;

    // Do not accept a caller-supplied partial plan: physical alias and blocked
    // ownership semantics remain exactly those of the full planner.
    let plan = plan_logical_model_exports_from_path(
        &bundle_index_path,
        LogicalModelExportPlanOptions::default(),
    )?;
    if plan.scope.mode != "full" || plan.scope.requested_kfm_route.is_some() {
        return Err("logical-model batch export requires a full ownership plan".to_string());
    }
    let mut prepared = prepare_batch(&plan)?;

    // Recheck after the long read-only plan, before creating staging output.
    reject_existing_destination(output_root, &manifest_path)?;
    let (staging_root, staging_manifest) = staging_paths(output_root, &manifest_path)?;
    fs::create_dir(&staging_root).map_err(|err| {
        format!(
            "could not create logical-model source staging directory {}: {err}",
            staging_root.display()
        )
    })?;

    let staged = stage_sources(&prepared.candidates, &project_dir, &staging_root);
    let staged = match staged {
        Ok(staged) => staged,
        Err(err) => {
            remove_staging(&staging_root, &staging_manifest);
            return Err(err);
        }
    };
    prepared.blocked.extend(staged.blocked);
    sort_blocked(&mut prepared.blocked);
    let status = if staged.exported.len() == prepared.candidates.len() {
        "complete"
    } else {
        "complete-with-blocked"
    };

    let manifest = LogicalModelSourceBatchManifest {
        schema: LOGICAL_MODEL_SOURCE_BATCH_SCHEMA,
        status,
        source_index_path: bundle_index_path.to_string_lossy().to_string(),
        source_root: output_root.to_string_lossy().to_string(),
        manifest_path: manifest_path.to_string_lossy().to_string(),
        full_plan_ownership_scan_complete: plan.ownership_scan_complete,
        counts: LogicalModelSourceBatchCounts {
            planned_logical_root_count: plan.logical_roots.len(),
            ready_self_contained_root_count: prepared.candidates.len(),
            exported_count: staged.exported.len(),
            skipped_root_count: prepared.skipped.len(),
            blocked_entry_count: prepared.blocked.len(),
            unresolved_reference_count: plan.unresolved.len(),
            full_plan_blocker_count: plan.blockers.len(),
        },
        exported: staged.exported,
        skipped: prepared.skipped,
        blocked: prepared.blocked,
    };
    if let Err(err) = write_new_json(&staging_manifest, &manifest) {
        remove_staging(&staging_root, &staging_manifest);
        return Err(err);
    }

    // Both paths live in the same parent. Ordinary rename failures roll back
    // the sibling manifest, so an unsuccessful call never leaves a partial
    // source root. A crash orphan is still fail-closed because create-new and
    // destination-exists checks refuse to reuse it.
    if let Err(err) = fs::rename(&staging_manifest, &manifest_path) {
        remove_staging(&staging_root, &staging_manifest);
        return Err(format!(
            "could not commit logical-model batch manifest {}: {err}",
            manifest_path.display()
        ));
    }
    if let Err(err) = fs::rename(&staging_root, output_root) {
        let _ = fs::remove_file(&manifest_path);
        remove_staging(&staging_root, &staging_manifest);
        return Err(format!(
            "could not commit logical-model source root {}: {err}",
            output_root.display()
        ));
    }

    Ok(manifest)
}

fn resolve_bundle_index_input(input: &Path) -> Result<PathBuf, String> {
    let bundle_index_path = if input.is_dir() {
        input.join("cache").join("bundle-index.json")
    } else {
        input.to_path_buf()
    };
    if !bundle_index_path.is_file() {
        return Err(format!(
            "logical-model batch input must be a client-project directory or cache/bundle-index.json: {}",
            input.display()
        ));
    }
    Ok(bundle_index_path)
}

fn project_dir_for_bundle_index(bundle_index_path: &Path) -> Result<PathBuf, String> {
    let cache_dir = bundle_index_path.parent().ok_or_else(|| {
        format!(
            "bundle-index path has no cache directory: {}",
            bundle_index_path.display()
        )
    })?;
    if !cache_dir
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("cache"))
    {
        return Err(format!(
            "bundle-index must be inside a project cache directory: {}",
            bundle_index_path.display()
        ));
    }
    cache_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "bundle-index cache directory has no project parent".to_string())
}

fn prepare_batch(plan: &LogicalModelExportPlan) -> Result<PreparedBatch, String> {
    let aliases = plan
        .physical_aliases
        .iter()
        .map(|proof| (proof.normalized_route.as_str(), proof))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = Vec::new();
    let mut skipped = Vec::new();
    let mut blocked = Vec::new();

    for root in &plan.logical_roots {
        match classify_root(
            root.status,
            root.payload.self_contained_game_object.is_some(),
            root.unresolved_nif_routes.is_empty(),
        ) {
            RootEligibility::Candidate => {
                let self_contained = root
                    .payload
                    .self_contained_game_object
                    .as_ref()
                    .expect("candidate classification requires self-contained evidence");
                let family = source_family(&root.kfm.normalized_route)?;
                let semantic_directories =
                    source_semantic_directories(&family, &root.kfm.exact_route)?;
                let relative_source_path = source_relative_path(
                    &family,
                    &semantic_directories,
                    &self_contained.root_name,
                )?;
                let alias = aliases.get(root.kfm.normalized_route.as_str()).copied();
                candidates.push(PreparedCandidate {
                    family,
                    semantic_directories,
                    true_name: self_contained.root_name.clone(),
                    relative_source_path,
                    exact_route: root.kfm.exact_route.clone(),
                    normalized_route: root.kfm.normalized_route.clone(),
                    bundle_path: root.kfm.owner.bundle_path.clone(),
                    ownership: LogicalModelSourceOwnershipEvidence {
                        ownership_evidence: root.payload.ownership_evidence.clone(),
                        self_contained_proof: self_contained.proof.clone(),
                        canonical_owner: root.kfm.owner.clone(),
                        physical_alias_owners: alias
                            .map(|proof| proof.alias_owners.clone())
                            .unwrap_or_default(),
                        physical_fingerprint: alias.map(|proof| proof.fingerprint.clone()),
                        root_game_object: self_contained.root_game_object.clone(),
                        root_transform: self_contained.root_transform.clone(),
                    },
                });
            }
            RootEligibility::SkippedNotSelfContained => {
                skipped.push(LogicalModelSourceBatchSkipped {
                    exact_route: root.kfm.exact_route.clone(),
                    normalized_route: root.kfm.normalized_route.clone(),
                    code: "readyRootNotSelfContained".to_string(),
                    detail: "ready KFM root has no exact self-contained GameObject proof"
                        .to_string(),
                    ownership_evidence: root.payload.ownership_evidence.clone(),
                });
            }
            RootEligibility::Blocked => {
                let detail = if root.unresolved_nif_routes.is_empty() {
                    "full ownership plan marked this logical root blocked".to_string()
                } else {
                    format!(
                        "unresolved owned NIF routes: {}",
                        root.unresolved_nif_routes.join(", ")
                    )
                };
                blocked.push(LogicalModelSourceBatchBlocked {
                    subject_kind: "logicalRoot".to_string(),
                    exact_route: Some(root.kfm.exact_route.clone()),
                    normalized_route: Some(root.kfm.normalized_route.clone()),
                    code: "logicalRootBlockedOrUnresolved".to_string(),
                    detail,
                    owners: vec![root.kfm.owner.clone()],
                    ownership_evidence: Some(root.payload.ownership_evidence.clone()),
                });
            }
        }
    }

    for blocker in &plan.blockers {
        blocked.push(LogicalModelSourceBatchBlocked {
            subject_kind: blocker.subject_kind.clone(),
            exact_route: (blocker.exact_routes.len() == 1).then(|| blocker.exact_routes[0].clone()),
            normalized_route: blocker.normalized_route.clone(),
            code: blocker.code.clone(),
            detail: blocker.detail.clone(),
            owners: blocker.owners.clone(),
            ownership_evidence: None,
        });
    }

    candidates.sort_by(|left, right| {
        slash_path(&left.relative_source_path)
            .cmp(&slash_path(&right.relative_source_path))
            .then_with(|| left.normalized_route.cmp(&right.normalized_route))
            .then_with(|| left.exact_route.cmp(&right.exact_route))
    });
    skipped.sort_by(|left, right| {
        left.normalized_route
            .cmp(&right.normalized_route)
            .then_with(|| left.exact_route.cmp(&right.exact_route))
            .then_with(|| left.code.cmp(&right.code))
    });
    sort_blocked(&mut blocked);

    preflight_output_paths(
        &candidates
            .iter()
            .map(|candidate| {
                (
                    slash_path(&candidate.relative_source_path),
                    candidate.exact_route.clone(),
                )
            })
            .collect::<Vec<_>>(),
    )?;

    Ok(PreparedBatch {
        candidates,
        skipped,
        blocked,
    })
}

fn sort_blocked(blocked: &mut [LogicalModelSourceBatchBlocked]) {
    blocked.sort_by(|left, right| {
        left.subject_kind
            .cmp(&right.subject_kind)
            .then_with(|| left.normalized_route.cmp(&right.normalized_route))
            .then_with(|| left.exact_route.cmp(&right.exact_route))
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.detail.cmp(&right.detail))
    });
}

fn classify_root(
    status: LogicalModelRootStatus,
    has_self_contained_proof: bool,
    unresolved_routes_empty: bool,
) -> RootEligibility {
    if status == LogicalModelRootStatus::Blocked || !unresolved_routes_empty {
        RootEligibility::Blocked
    } else if !has_self_contained_proof {
        RootEligibility::SkippedNotSelfContained
    } else {
        RootEligibility::Candidate
    }
}

fn source_family(normalized_route: &str) -> Result<String, String> {
    let family = normalized_route
        .split('/')
        .next()
        .filter(|family| !family.is_empty())
        .ok_or_else(|| format!("logical-model route has no source family: {normalized_route:?}"))?;
    // Reuse the native publisher's path validation instead of inventing an
    // editor-only family sanitizer. The true name supplied here is harmless
    // and only used to exercise the shared family contract.
    model_relative_path(family, &[], "family-contract-probe")
        .map_err(|err| format!("invalid logical-model source family {family:?}: {err}"))?;
    Ok(family.to_string())
}

fn source_semantic_directories(family: &str, exact_route: &str) -> Result<Vec<String>, String> {
    let components = exact_route.split(['/', '\\']).collect::<Vec<_>>();
    if components.len() < 2 || components.iter().any(|component| component.is_empty()) {
        return Err(format!(
            "logical-model exact route must contain a family and non-empty semantic route components: {exact_route:?}"
        ));
    }
    if components[0].to_lowercase() != family.to_lowercase() {
        return Err(format!(
            "logical-model exact route family {:?} contradicts planned family {family:?}",
            components[0]
        ));
    }
    let filename = components[components.len() - 1];
    if filename.len() <= 4 || !filename[filename.len() - 4..].eq_ignore_ascii_case(".kfm") {
        return Err(format!(
            "logical-model exact route does not end in .kfm: {exact_route:?}"
        ));
    }
    let final_stem = &filename[..filename.len() - 4];
    if final_stem.is_empty() {
        return Err(format!(
            "logical-model exact route has an empty stem: {exact_route:?}"
        ));
    }
    let mut semantic_directories = components[1..components.len() - 1]
        .iter()
        .map(|component| (*component).to_string())
        .collect::<Vec<_>>();
    semantic_directories.push(final_stem.to_string());
    // Route components are semantic owner directories, not generated
    // identities. They must already be safe; silently sanitizing one could
    // merge two distinct route owners.
    let semantic_refs = semantic_directories
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    model_relative_path(
        "family-contract-probe",
        &semantic_refs,
        "name-contract-probe",
    )
    .map_err(|err| format!("invalid exact-route semantic directories: {err}"))?;
    Ok(semantic_directories)
}

fn source_relative_path(
    family: &str,
    semantic_directories: &[String],
    true_name: &str,
) -> Result<PathBuf, String> {
    let semantic_refs = semantic_directories
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let native_path = model_relative_path(family, &semantic_refs, true_name).map_err(|err| {
        format!("true logical-model name {true_name:?} cannot form a native family path: {err}")
    })?;
    let glb_filename = native_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "native true-name path has no UTF-8 filename".to_string())?;
    let stem = glb_filename
        .strip_suffix(".glb")
        .ok_or_else(|| "native true-name path does not end in .glb".to_string())?;
    let mut relative = PathBuf::from(family);
    relative.extend(semantic_directories);
    relative.push(format!("{stem}.source.json"));
    Ok(relative)
}

fn preflight_output_paths(paths: &[(String, String)]) -> Result<(), String> {
    let mut by_case_folded_path = BTreeMap::<String, Vec<(String, String)>>::new();
    for (path, exact_route) in paths {
        by_case_folded_path
            .entry(portable_string_key(path))
            .or_default()
            .push((path.clone(), exact_route.clone()));
    }
    let collisions = by_case_folded_path
        .into_iter()
        .filter(|(_, owners)| owners.len() > 1)
        .collect::<Vec<_>>();
    if collisions.is_empty() {
        return Ok(());
    }

    let detail = collisions
        .into_iter()
        .map(|(folded, mut owners)| {
            owners.sort();
            let sources = owners
                .into_iter()
                .map(|(path, route)| format!("{path:?} from {route:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{folded:?} <- [{sources}]")
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(format!(
        "logical-model batch filename collision after shared minimal Windows sanitization and case-insensitive folding; refusing every write: {detail}"
    ))
}

fn portable_string_key(value: &str) -> String {
    // Keep the editor preflight aligned with the native batch publisher:
    // compatibility-normalize, lowercase Unicode scalars, normalize again.
    value.nfkc().flat_map(char::to_lowercase).nfkc().collect()
}

#[derive(Debug)]
struct StagedSources {
    exported: Vec<LogicalModelSourceBatchExported>,
    blocked: Vec<LogicalModelSourceBatchBlocked>,
}

fn stage_sources(
    candidates: &[PreparedCandidate],
    project_dir: &Path,
    staging_root: &Path,
) -> Result<StagedSources, String> {
    stage_sources_with(candidates, staging_root, |candidate| {
        crate::preview_bundle_container_model_exact(
            candidate.bundle_path.clone(),
            Some(project_dir.to_string_lossy().to_string()),
            candidate.exact_route.clone(),
        )
    })
}

fn stage_sources_with(
    candidates: &[PreparedCandidate],
    staging_root: &Path,
    mut load_source: impl FnMut(&PreparedCandidate) -> Result<JsonValue, String>,
) -> Result<StagedSources, String> {
    let mut exported = Vec::with_capacity(candidates.len());
    let mut blocked = Vec::new();
    for candidate in candidates {
        let source = match load_source(candidate) {
            Ok(source) => source,
            Err(err) => {
                blocked.push(source_stage_blocker(candidate, err));
                continue;
            }
        };
        let mut source_bytes =
            match validate_exact_source_identity(&source, candidate).and_then(|()| {
                serde_json::to_vec_pretty(&source)
                    .map_err(|err| format!("could not encode exact logical-model source: {err}"))
            }) {
                Ok(source_bytes) => source_bytes,
                Err(err) => {
                    blocked.push(source_stage_blocker(candidate, err));
                    continue;
                }
            };
        source_bytes.push(b'\n');

        let destination = staging_root.join(&candidate.relative_source_path);
        let parent = destination.parent().ok_or_else(|| {
            format!(
                "logical-model source path has no parent: {}",
                destination.display()
            )
        })?;
        fs::create_dir_all(parent)
            .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
        write_new_bytes(&destination, &source_bytes)?;

        exported.push(LogicalModelSourceBatchExported {
            family: candidate.family.clone(),
            semantic_directories: candidate.semantic_directories.clone(),
            true_name: candidate.true_name.clone(),
            source_relative_path: slash_path(&candidate.relative_source_path),
            source_byte_length: u64::try_from(source_bytes.len())
                .map_err(|_| "logical-model source byte length overflow".to_string())?,
            source_sha256: format!("{:x}", Sha256::digest(&source_bytes)),
            exact_route: candidate.exact_route.clone(),
            normalized_route: candidate.normalized_route.clone(),
            source_ownership: candidate.ownership.clone(),
        });
    }
    Ok(StagedSources { exported, blocked })
}

fn source_stage_blocker(
    candidate: &PreparedCandidate,
    detail: String,
) -> LogicalModelSourceBatchBlocked {
    let mut owners = vec![candidate.ownership.canonical_owner.clone()];
    for owner in &candidate.ownership.physical_alias_owners {
        if !owners.contains(owner) {
            owners.push(owner.clone());
        }
    }
    LogicalModelSourceBatchBlocked {
        subject_kind: "logicalRoot".to_string(),
        exact_route: Some(candidate.exact_route.clone()),
        normalized_route: Some(candidate.normalized_route.clone()),
        code: "sourceStageExportFailed".to_string(),
        detail,
        owners,
        ownership_evidence: Some(candidate.ownership.ownership_evidence.clone()),
    }
}

fn validate_exact_source_identity(
    source: &JsonValue,
    candidate: &PreparedCandidate,
) -> Result<(), String> {
    require_source_string(source, "schema", "ffone.logical-model-source.v1")?;
    require_source_string(source, "selectionMode", "exact-container-route")?;
    require_source_string(source, "status", "ready")?;
    require_source_string(source, "logicalName", &candidate.true_name)?;
    require_source_string(source, "exactContainerRoute", &candidate.exact_route)?;

    let roots = source
        .pointer("/modelHierarchy/roots")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "exact source has no modelHierarchy.roots array".to_string())?;
    if roots.len() != 1
        || roots[0].get("name").and_then(JsonValue::as_str) != Some(candidate.true_name.as_str())
    {
        return Err(format!(
            "exact source root does not preserve the planned true m_Name {:?}",
            candidate.true_name
        ));
    }

    let matched_paths = source
        .get("matchedPaths")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "exact source has no matchedPaths array".to_string())?;
    if matched_paths.len() != 1
        || matched_paths[0]
            .as_str()
            .map(normalize_logical_model_route)
            .as_deref()
            != Some(candidate.normalized_route.as_str())
    {
        return Err(format!(
            "exact source matchedPaths does not identify only planned route {:?}",
            candidate.exact_route
        ));
    }

    let warnings = source
        .get("warnings")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "exact source has no warnings array".to_string())?;
    if !warnings.is_empty() {
        return Err(format!(
            "exact source {:?} contains {} warning(s); refusing a lossy batch",
            candidate.exact_route,
            warnings.len()
        ));
    }
    Ok(())
}

fn require_source_string(source: &JsonValue, field: &str, expected: &str) -> Result<(), String> {
    let actual = source.get(field).and_then(JsonValue::as_str);
    if actual != Some(expected) {
        return Err(format!(
            "exact source field {field:?} must be {expected:?}, got {actual:?}"
        ));
    }
    Ok(())
}

fn reject_existing_destination(output_root: &Path, manifest_path: &Path) -> Result<(), String> {
    if output_root.exists() {
        return Err(format!(
            "logical-model source output must be a fresh path: {}",
            output_root.display()
        ));
    }
    if manifest_path.exists() {
        return Err(format!(
            "logical-model source manifest must be a fresh path: {}",
            manifest_path.display()
        ));
    }
    let parent = output_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(format!(
            "logical-model source output parent must already exist: {}",
            output_root.display()
        ));
    }
    Ok(())
}

fn batch_manifest_path(output_root: &Path) -> Result<PathBuf, String> {
    let parent = output_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty());
    let parent = parent.unwrap_or_else(|| Path::new("."));
    let name = output_root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "logical-model source output has no UTF-8 directory name: {}",
                output_root.display()
            )
        })?;
    Ok(parent.join(format!("{name}.manifest.json")))
}

fn staging_paths(output_root: &Path, manifest_path: &Path) -> Result<(PathBuf, PathBuf), String> {
    let parent = output_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let output_name = output_root
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "logical-model source output has no directory name".to_string())?;
    let manifest_name = manifest_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "logical-model batch manifest has no filename".to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let suffix = format!("{}.{}", std::process::id(), nonce);
    Ok((
        parent.join(format!(".{output_name}.staging.{suffix}")),
        parent.join(format!(".{manifest_name}.staging.{suffix}")),
    ))
}

fn write_new_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|err| format!("could not encode {}: {err}", path.display()))?;
    bytes.push(b'\n');
    write_new_bytes(path, &bytes)
}

fn write_new_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| format!("could not create {}: {err}", path.display()))?;
    file.write_all(bytes)
        .map_err(|err| format!("could not write {}: {err}", path.display()))?;
    file.sync_all()
        .map_err(|err| format!("could not sync {}: {err}", path.display()))
}

fn remove_staging(staging_root: &Path, staging_manifest: &Path) {
    let _ = fs::remove_file(staging_manifest);
    let _ = fs::remove_dir_all(staging_root);
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests;
