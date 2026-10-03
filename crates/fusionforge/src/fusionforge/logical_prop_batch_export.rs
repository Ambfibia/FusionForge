//! Atomic export of reviewed, ownership-proven standalone NIF props.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ffone_skinned_model::minimal_windows_glb_filename;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::{
    logical_model_catalog::{normalize_logical_model_route, LogicalModelOwner},
    logical_model_export_plan::{
        plan_logical_model_exports_from_path, LogicalModelExportPlan,
        LogicalModelExportPlanOptions, LogicalModelStandaloneNif, LogicalModelStandaloneProof,
    },
};

pub const LOGICAL_PROP_REVIEW_SCHEMA: &str = "ffclient.logical-prop-review.v1";
pub const LOGICAL_PROP_SOURCE_BATCH_SCHEMA: &str = "ffclient.logical-prop-source-batch.v1";

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicalPropCategory {
    Tree,
    Grass,
    Rock,
}

impl LogicalPropCategory {
    fn directories(self) -> [&'static str; 2] {
        match self {
            Self::Tree => ["vegetation", "trees"],
            Self::Grass => ["vegetation", "grass"],
            Self::Rock => ["nature", "rocks"],
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicalPropReviewMethod {
    ManualVisualReview,
    ExactWorldPlacement,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalPropReviewEvidence {
    pub method: LogicalPropReviewMethod,
    pub reference: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedStandaloneNifProof {
    pub sole_catalog_owner: bool,
    pub exact_route_has_no_collision: bool,
    pub not_referenced_by_any_parsed_kfm: bool,
    pub all_kfm_groups_parsed: bool,
}

impl ReviewedStandaloneNifProof {
    fn complete(&self) -> bool {
        self.sole_catalog_owner
            && self.exact_route_has_no_collision
            && self.not_referenced_by_any_parsed_kfm
            && self.all_kfm_groups_parsed
    }

    fn matches(&self, proof: &LogicalModelStandaloneProof) -> bool {
        self.sole_catalog_owner == proof.sole_catalog_owner
            && self.exact_route_has_no_collision == proof.exact_route_has_no_collision
            && self.not_referenced_by_any_parsed_kfm == proof.not_referenced_by_any_parsed_kfm
            && self.all_kfm_groups_parsed == proof.all_kfm_groups_parsed
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedLogicalProp {
    pub exact_route: String,
    pub normalized_route: String,
    pub category: LogicalPropCategory,
    pub review: LogicalPropReviewEvidence,
    pub expected_standalone_proof: ReviewedStandaloneNifProof,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalPropReview {
    pub schema: String,
    pub status: String,
    pub routes: Vec<ReviewedLogicalProp>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalPropSourceBatchManifest {
    pub schema: &'static str,
    pub status: &'static str,
    pub source_index_path: String,
    pub source_root: String,
    pub manifest_path: String,
    pub reviewed_plan_path: String,
    pub reviewed_plan_sha256: String,
    pub full_plan_ownership_scan_complete: bool,
    pub standalone_nif_proof_complete: bool,
    pub reviewed_count: usize,
    pub exported_count: usize,
    pub exported: Vec<LogicalPropSourceExported>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalPropSourceExported {
    pub exact_route: String,
    pub normalized_route: String,
    pub category: LogicalPropCategory,
    pub logical_name: String,
    pub source_relative_path: String,
    pub source_byte_length: u64,
    pub source_sha256: String,
    pub canonical_owner: LogicalModelOwner,
    pub standalone_proof: ReviewedStandaloneNifProof,
    pub review: LogicalPropReviewEvidence,
}

#[derive(Clone)]
struct PreparedProp {
    reviewed: ReviewedLogicalProp,
    standalone: LogicalModelStandaloneNif,
}

struct PlanGate<'a> {
    scope_mode: &'a str,
    no_requested_kfm: bool,
    ownership_complete: bool,
    standalone_complete: bool,
    standalone_nifs: &'a [LogicalModelStandaloneNif],
}

impl<'a> From<&'a LogicalModelExportPlan> for PlanGate<'a> {
    fn from(plan: &'a LogicalModelExportPlan) -> Self {
        Self {
            scope_mode: plan.scope.mode,
            no_requested_kfm: plan.scope.requested_kfm_route.is_none(),
            ownership_complete: plan.ownership_scan_complete,
            standalone_complete: plan.standalone_nif_proof_complete,
            standalone_nifs: &plan.standalone_nifs,
        }
    }
}

pub fn export_logical_prop_sources_batch(
    input: impl AsRef<Path>,
    review_path: impl AsRef<Path>,
    output_root: impl AsRef<Path>,
) -> Result<LogicalPropSourceBatchManifest, String> {
    let index = resolve_index(input.as_ref())?;
    let project = project_for_index(&index)?;
    let review_path = review_path.as_ref();
    let output_root = output_root.as_ref();
    let manifest_path = manifest_path(output_root)?;
    reject_existing(output_root, &manifest_path)?;

    // A name-token candidate report fails here, before a staging path exists.
    let review_bytes = fs::read(review_path)
        .map_err(|err| format!("could not read {}: {err}", review_path.display()))?;
    let review = parse_review(&review_bytes, review_path)?;

    // Caller-supplied ownership plans are never accepted.
    let plan =
        plan_logical_model_exports_from_path(&index, LogicalModelExportPlanOptions::default())?;
    export_with(
        &index,
        review_path,
        &review_bytes,
        &review,
        PlanGate::from(&plan),
        output_root,
        |candidate| {
            crate::preview_bundle_container_model_exact(
                candidate.standalone.nif.owner.bundle_path.clone(),
                Some(project.to_string_lossy().to_string()),
                candidate.reviewed.exact_route.clone(),
            )
        },
    )
}

fn parse_review(bytes: &[u8], path: &Path) -> Result<LogicalPropReview, String> {
    let review: LogicalPropReview = serde_json::from_slice(bytes)
        .map_err(|err| format!("invalid reviewed prop plan {}: {err}", path.display()))?;
    if review.schema != LOGICAL_PROP_REVIEW_SCHEMA {
        return Err(format!(
            "unsupported reviewed prop schema {:?}",
            review.schema
        ));
    }
    if review.status != "verified" {
        return Err("reviewed prop status must be \"verified\"".to_string());
    }
    if review.routes.is_empty() {
        return Err("reviewed prop plan has no verified routes".to_string());
    }
    Ok(review)
}

#[allow(clippy::too_many_arguments)]
fn export_with(
    index: &Path,
    review_path: &Path,
    review_bytes: &[u8],
    review: &LogicalPropReview,
    plan: PlanGate<'_>,
    output_root: &Path,
    mut load: impl FnMut(&PreparedProp) -> Result<Value, String>,
) -> Result<LogicalPropSourceBatchManifest, String> {
    let manifest_path = manifest_path(output_root)?;
    reject_existing(output_root, &manifest_path)?;
    let prepared = prepare(review, &plan)?;
    reject_existing(output_root, &manifest_path)?;
    let (stage, stage_manifest) = staging_paths(output_root, &manifest_path)?;
    fs::create_dir(&stage).map_err(|err| format!("could not create staging: {err}"))?;
    let exported = match stage_sources(&prepared, &stage, &mut load) {
        Ok(value) => value,
        Err(err) => {
            remove_staging(&stage, &stage_manifest);
            return Err(err);
        }
    };
    let result = LogicalPropSourceBatchManifest {
        schema: LOGICAL_PROP_SOURCE_BATCH_SCHEMA,
        status: "complete",
        source_index_path: slash(index),
        source_root: slash(output_root),
        manifest_path: slash(&manifest_path),
        reviewed_plan_path: slash(review_path),
        reviewed_plan_sha256: sha256(review_bytes),
        full_plan_ownership_scan_complete: plan.ownership_complete,
        standalone_nif_proof_complete: plan.standalone_complete,
        reviewed_count: review.routes.len(),
        exported_count: exported.len(),
        exported,
    };
    if let Err(err) = write_json_new(&stage_manifest, &result) {
        remove_staging(&stage, &stage_manifest);
        return Err(err);
    }
    if let Err(err) = fs::rename(&stage_manifest, &manifest_path) {
        remove_staging(&stage, &stage_manifest);
        return Err(format!(
            "could not commit {}: {err}",
            manifest_path.display()
        ));
    }
    if let Err(err) = fs::rename(&stage, output_root) {
        let _ = fs::remove_file(&manifest_path);
        remove_staging(&stage, &stage_manifest);
        return Err(format!("could not commit {}: {err}", output_root.display()));
    }
    Ok(result)
}

fn prepare(review: &LogicalPropReview, plan: &PlanGate<'_>) -> Result<Vec<PreparedProp>, String> {
    if plan.scope_mode != "full" || !plan.no_requested_kfm {
        return Err("logical-prop export requires a recomputed full plan".to_string());
    }
    if !plan.ownership_complete || !plan.standalone_complete {
        return Err(
            "logical-prop export requires ownershipScanComplete=true and standaloneNifProofComplete=true"
                .to_string(),
        );
    }
    let mut by_route = BTreeMap::new();
    for standalone in plan.standalone_nifs {
        if by_route
            .insert(standalone.nif.normalized_route.as_str(), standalone)
            .is_some()
        {
            return Err("duplicate standalone NIF route in full plan".to_string());
        }
    }
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for reviewed in &review.routes {
        validate_reviewed(reviewed)?;
        if !seen.insert(reviewed.normalized_route.clone()) {
            return Err(format!(
                "duplicate reviewed route {:?}",
                reviewed.normalized_route
            ));
        }
        let standalone = by_route
            .get(reviewed.normalized_route.as_str())
            .copied()
            .ok_or_else(|| {
                format!(
                    "reviewed route {:?} is not a proven standalone NIF",
                    reviewed.exact_route
                )
            })?;
        if standalone.nif.exact_route != reviewed.exact_route {
            return Err("reviewed exact route differs from recomputed route".to_string());
        }
        if !reviewed
            .expected_standalone_proof
            .matches(&standalone.proof)
            || !proof_complete(&standalone.proof)
        {
            return Err("all four standalone-NIF proofs must match exactly".to_string());
        }
        output.push(PreparedProp {
            reviewed: reviewed.clone(),
            standalone: standalone.clone(),
        });
    }
    output.sort_by(|a, b| {
        a.reviewed.category.cmp(&b.reviewed.category).then(
            a.reviewed
                .normalized_route
                .cmp(&b.reviewed.normalized_route),
        )
    });
    Ok(output)
}

fn validate_reviewed(reviewed: &ReviewedLogicalProp) -> Result<(), String> {
    let normalized = normalize_logical_model_route(&reviewed.exact_route);
    if normalized.is_empty()
        || normalized != reviewed.normalized_route
        || !normalized.ends_with(".nif")
    {
        return Err("reviewed route is not an exact normalized NIF".to_string());
    }
    validate_route(&normalized)?;
    if reviewed.review.reference.trim().is_empty() {
        return Err("review evidence reference is empty".to_string());
    }
    if !reviewed.expected_standalone_proof.complete() {
        return Err("review does not assert all four standalone proofs".to_string());
    }
    Ok(())
}

fn proof_complete(proof: &LogicalModelStandaloneProof) -> bool {
    proof.sole_catalog_owner
        && proof.exact_route_has_no_collision
        && proof.not_referenced_by_any_parsed_kfm
        && proof.all_kfm_groups_parsed
}

fn stage_sources(
    prepared: &[PreparedProp],
    stage: &Path,
    load: &mut impl FnMut(&PreparedProp) -> Result<Value, String>,
) -> Result<Vec<LogicalPropSourceExported>, String> {
    let mut paths = BTreeSet::new();
    let mut output = Vec::new();
    for candidate in prepared {
        let source = load(candidate).map_err(|err| {
            format!(
                "exact NIF export failed for {:?}: {err}",
                candidate.reviewed.exact_route
            )
        })?;
        let logical_name = validate_source(&source, candidate)?;
        let relative = source_path(candidate, &logical_name)?;
        let relative_text = slash(&relative);
        if !paths.insert(portable_key(&relative_text)) {
            return Err(format!("portable prop path collision: {relative_text:?}"));
        }
        let mut bytes =
            serde_json::to_vec_pretty(&source).map_err(|err| format!("JSON encode: {err}"))?;
        bytes.push(b'\n');
        let destination = stage.join(&relative);
        fs::create_dir_all(destination.parent().unwrap())
            .map_err(|err| format!("could not create source parent: {err}"))?;
        write_new(&destination, &bytes)?;
        output.push(LogicalPropSourceExported {
            exact_route: candidate.reviewed.exact_route.clone(),
            normalized_route: candidate.reviewed.normalized_route.clone(),
            category: candidate.reviewed.category,
            logical_name,
            source_relative_path: relative_text,
            source_byte_length: bytes.len() as u64,
            source_sha256: sha256(&bytes),
            canonical_owner: candidate.standalone.nif.owner.clone(),
            standalone_proof: candidate.reviewed.expected_standalone_proof.clone(),
            review: candidate.reviewed.review.clone(),
        });
    }
    output.sort_by(|a, b| a.source_relative_path.cmp(&b.source_relative_path));
    Ok(output)
}

fn validate_source(source: &Value, candidate: &PreparedProp) -> Result<String, String> {
    required(source, "schema", "ffone.logical-model-source.v1")?;
    required(source, "selectionMode", "exact-container-route")?;
    required(source, "status", "ready")?;
    required(
        source,
        "exactContainerRoute",
        &candidate.reviewed.exact_route,
    )?;
    let name = source
        .get("logicalName")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "exact NIF source has no logicalName".to_string())?
        .to_string();
    let one_route = |field: &str| {
        source
            .get(field)
            .and_then(Value::as_array)
            .filter(|values| values.len() == 1)
            .and_then(|values| values[0].as_str())
    };
    if one_route("containerPaths") != Some(candidate.reviewed.exact_route.as_str())
        || one_route("matchedPaths")
            .map(normalize_logical_model_route)
            .as_deref()
            != Some(candidate.reviewed.normalized_route.as_str())
    {
        return Err("exact NIF source route identity changed".to_string());
    }
    let roots = source
        .pointer("/modelHierarchy/roots")
        .and_then(Value::as_array)
        .ok_or_else(|| "exact NIF source has no roots".to_string())?;
    if roots.len() != 1 || roots[0].get("name").and_then(Value::as_str) != Some(name.as_str()) {
        return Err("exact NIF source does not preserve one true root".to_string());
    }
    if source
        .pointer("/modelHierarchy/nodes")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
        || source
            .get("meshes")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        return Err("exact NIF source has no complete hierarchy/mesh closure".to_string());
    }
    if source
        .get("warnings")
        .and_then(Value::as_array)
        .is_none_or(|values| !values.is_empty())
    {
        return Err("exact NIF source contains warnings".to_string());
    }
    if source
        .get("kfm")
        .and_then(Value::as_array)
        .is_some_and(|values| !values.is_empty())
    {
        return Err("standalone NIF unexpectedly contains KFM metadata".to_string());
    }
    if source.get("animations").and_then(Value::as_array).is_none() {
        return Err("exact NIF source has no animations array".to_string());
    }
    Ok(name)
}

fn required(source: &Value, field: &str, expected: &str) -> Result<(), String> {
    if source.get(field).and_then(Value::as_str) == Some(expected) {
        Ok(())
    } else {
        Err(format!("exact NIF source {field} is not {expected:?}"))
    }
}

fn source_path(candidate: &PreparedProp, name: &str) -> Result<PathBuf, String> {
    let mut path = PathBuf::from("props");
    path.extend(candidate.reviewed.category.directories());
    for component in candidate
        .reviewed
        .normalized_route
        .strip_suffix(".nif")
        .unwrap()
        .split('/')
    {
        validate_component(component)?;
        path.push(component);
    }
    let filename = minimal_windows_glb_filename(name)
        .map_err(|err| format!("unsafe prop root name {name:?}: {err}"))?;
    path.push(format!(
        "{}.source.json",
        filename.strip_suffix(".glb").unwrap()
    ));
    Ok(path)
}

fn validate_route(route: &str) -> Result<(), String> {
    if route.is_empty()
        || route.contains('\\')
        || route.contains(':')
        || Path::new(route).is_absolute()
        || Path::new(route)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        Err(format!("unsafe prop route {route:?}"))
    } else {
        Ok(())
    }
}

fn validate_component(value: &str) -> Result<(), String> {
    if value.is_empty() || value == "." || value == ".." || value.contains(['/', '\\', ':']) {
        Err(format!("unsafe prop path component {value:?}"))
    } else {
        Ok(())
    }
}

fn resolve_index(input: &Path) -> Result<PathBuf, String> {
    let path = if input.is_dir() {
        input.join("cache").join("bundle-index.json")
    } else {
        input.to_path_buf()
    };
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("missing bundle-index input {}", input.display()))
    }
}

fn project_for_index(index: &Path) -> Result<PathBuf, String> {
    let cache = index
        .parent()
        .ok_or_else(|| "bundle-index has no parent".to_string())?;
    if !cache
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("cache"))
    {
        return Err("bundle-index is not inside a cache directory".to_string());
    }
    cache
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "cache has no project parent".to_string())
}

fn manifest_path(output: &Path) -> Result<PathBuf, String> {
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "output name is not UTF-8".to_string())?;
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Ok(parent.join(format!("{name}.manifest.json")))
}

fn reject_existing(output: &Path, manifest: &Path) -> Result<(), String> {
    if output.exists() || manifest.exists() {
        return Err("logical-prop output or manifest already exists".to_string());
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err("logical-prop output parent does not exist".to_string());
    }
    Ok(())
}

fn staging_paths(output: &Path, manifest: &Path) -> Result<(PathBuf, PathBuf), String> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let output_name = output.file_name().unwrap().to_string_lossy();
    let manifest_name = manifest.file_name().unwrap().to_string_lossy();
    let suffix = unique();
    Ok((
        parent.join(format!(".{output_name}.staging.{suffix}")),
        parent.join(format!(".{manifest_name}.staging.{suffix}")),
    ))
}

fn write_json_new(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?;
    bytes.push(b'\n');
    write_new(path, &bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|err| format!("could not create {}: {err}", path.display()))?;
    file.write_all(bytes)
        .map_err(|err| format!("could not write {}: {err}", path.display()))?;
    file.sync_all()
        .map_err(|err| format!("could not sync {}: {err}", path.display()))
}

fn remove_staging(root: &Path, manifest: &Path) {
    let _ = fs::remove_file(manifest);
    let _ = fs::remove_dir_all(root);
}

fn portable_key(value: &str) -> String {
    value.nfkc().flat_map(char::to_lowercase).nfkc().collect()
}

fn unique() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{nanos}", std::process::id())
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests;
