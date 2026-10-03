//! Native, reproducible audit of authored logical-model root transforms.
//!
//! This offline import-time check reads the exact serialized Unity `Transform`
//! selected by the ownership plan. It never invokes Unity and never recenters,
//! rescales, or otherwise normalizes authored content.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::Value as JsonValue;

use super::{
    coordinates::{unity_to_native_quaternion, unity_to_native_scale, unity_to_native_vec3},
    extract_bundle, Asset, UnityEnvironment, UnityValue,
};

pub const LEGACY_ROOT_TRANSFORM_AUDIT_SCHEMA: &str = "ffone.legacy-root-transform-audit.v1";
const LOGICAL_MODEL_PLAN_SCHEMA: &str = "ffclient.logical-model-export-plan.v1";
const APPROX_EPSILON: f64 = 1.0e-6;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRootTransformAudit {
    pub schema: &'static str,
    pub source_plan: String,
    pub coordinate_contract: RootCoordinateContract,
    pub counts: RootTransformAuditCounts,
    pub errors: Vec<RootTransformAuditError>,
    pub roots: Vec<RootTransformAuditEntry>,
}

impl LegacyRootTransformAudit {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
            && self.counts.errors == self.errors.len()
            && self.counts.audited_roots == self.roots.len()
            && self.counts.planned_ready_roots == self.counts.audited_roots
            && self.counts.nonuniform_scale_roots == 0
            && self.counts.nonpositive_scale_roots == 0
            && self.counts.nonfinite_roots == 0
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootCoordinateContract {
    pub basis: &'static str,
    pub translation: &'static str,
    pub rotation: &'static str,
    pub scale: &'static str,
    pub unit_scale: &'static str,
    pub origin_policy: &'static str,
}

impl Default for RootCoordinateContract {
    fn default() -> Self {
        Self {
            basis: "H=diag(-1,1,1)",
            translation: "[-unity.x,unity.y,unity.z]",
            rotation: "[unity.x,-unity.y,-unity.z,unity.w]",
            scale: "unchanged",
            unit_scale: "1-unity-unit-equals-1-bevy-unit",
            origin_policy: "source-root-trs-unchanged-no-auto-centering",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootTransformAuditCounts {
    pub planned_ready_roots: usize,
    pub audited_roots: usize,
    pub errors: usize,
    pub unit_scale_roots: usize,
    pub nonunit_scale_roots: usize,
    pub nonuniform_scale_roots: usize,
    pub nonpositive_scale_roots: usize,
    pub nonzero_origin_roots: usize,
    pub nonidentity_rotation_roots: usize,
    pub nonfinite_roots: usize,
    pub unique_physical_root_transforms: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootTransformAuditError {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_route: Option<String>,
    pub code: String,
    pub detail: String,
}

impl RootTransformAuditError {
    fn new(route: Option<&str>, code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            legacy_route: route.map(str::to_string),
            code: code.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootTransformAuditEntry {
    pub legacy_route: String,
    pub true_root_m_name: String,
    pub bundle: String,
    pub asset_name: String,
    pub transform_path_id: i64,
    pub unity: RootTrs,
    pub native: RootTrs,
    pub classification: RootTransformClassification,
    #[serde(skip)]
    physical_bundle_path: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootTrs {
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
    pub scale: [f64; 3],
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootTransformClassification {
    pub finite: bool,
    pub uniform_scale: bool,
    pub unit_scale: bool,
    pub positive_scale: bool,
    pub nonzero_authored_origin: bool,
    pub identity_authored_rotation: bool,
}

#[derive(Debug, Clone)]
struct ReadyRootSpec {
    legacy_route: String,
    true_root_name: String,
    owner_bundle_path: String,
    owner_asset_name: String,
    root_asset_name: String,
    root_path_id: i64,
    dependency_archives: Vec<DependencyArchiveSpec>,
}

#[derive(Debug, Clone)]
struct DependencyArchiveSpec {
    archive_name: String,
    candidate_bundle_paths: Vec<String>,
}

struct LocatedTransform {
    physical_bundle_path: String,
    body: UnityValue,
}

struct LoadedPhysicalBundle {
    env: UnityEnvironment,
}

struct PhysicalBundleCache {
    temp_root: PathBuf,
    by_path: BTreeMap<String, LoadedPhysicalBundle>,
    next_extract_index: usize,
}

impl PhysicalBundleCache {
    fn create() -> Result<Self, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock error: {err}"))?
            .as_nanos();
        for attempt in 0..64_u32 {
            let path = std::env::temp_dir().join(format!(
                "ffone-root-transform-audit-{}-{nanos}-{attempt}",
                process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        temp_root: path,
                        by_path: BTreeMap::new(),
                        next_extract_index: 0,
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(format!(
                        "could not create audit session {}: {err}",
                        path.display()
                    ));
                }
            }
        }
        Err("could not allocate a unique root-transform audit session".to_string())
    }

    fn load(&mut self, bundle_path: &str) -> Result<&LoadedPhysicalBundle, String> {
        let key = physical_path_key(bundle_path);
        if !self.by_path.contains_key(&key) {
            let path = Path::new(bundle_path);
            if !path.is_file() {
                return Err(format!(
                    "candidate physical bundle does not exist: {bundle_path}"
                ));
            }
            let env = match Asset::from_path(path) {
                Ok(asset) => UnityEnvironment::from_assets(vec![asset]),
                Err(_) => {
                    let extract_dir = self
                        .temp_root
                        .join(format!("bundle-{:04}", self.next_extract_index));
                    self.next_extract_index += 1;
                    extract_bundle(path, &extract_dir).map_err(|err| {
                        format!("could not extract physical bundle {bundle_path}: {err}")
                    })?;
                    UnityEnvironment::from_dir(&extract_dir)
                }
            };
            if env.assets.is_empty() {
                return Err(format!(
                    "physical bundle contained no readable serialized assets: {bundle_path}"
                ));
            }
            self.by_path
                .insert(key.clone(), LoadedPhysicalBundle { env });
        }
        self.by_path
            .get(&key)
            .ok_or_else(|| "physical-bundle cache insertion failed".to_string())
    }

    fn locate_transform(&mut self, root: &ReadyRootSpec) -> Result<LocatedTransform, String> {
        let candidates = root_candidate_bundle_paths(root)?;
        let mut matches = Vec::new();
        let mut load_errors = Vec::new();
        for bundle_path in candidates {
            let loaded = match self.load(&bundle_path) {
                Ok(loaded) => loaded,
                Err(err) => {
                    load_errors.push(err);
                    continue;
                }
            };
            for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
                if asset.name != root.root_asset_name {
                    continue;
                }
                let Some(info) = asset.objects.get(&root.root_path_id) else {
                    continue;
                };
                let object_type = asset.object_type_name(info);
                if object_type != "Transform" {
                    return Err(format!(
                        "{}#{} in {} is {object_type}, expected Transform",
                        root.root_asset_name, root.root_path_id, bundle_path
                    ));
                }
                let body = asset.read_object(asset_index, info).map_err(|err| {
                    format!(
                        "could not read {}#{} from {}: {err}",
                        root.root_asset_name, root.root_path_id, bundle_path
                    )
                })?;
                matches.push(LocatedTransform {
                    physical_bundle_path: bundle_path.clone(),
                    body,
                });
            }
        }
        if !load_errors.is_empty() {
            return Err(format!(
                "physical bundle proof was incomplete: {}",
                load_errors.join("; ")
            ));
        }
        match matches.len() {
            0 => Err(format!(
                "{}#{} was not found in the exact owner/dependency bundle candidates",
                root.root_asset_name, root.root_path_id
            )),
            1 => Ok(matches.remove(0)),
            count => Err(format!(
                "{}#{} is ambiguous: found {count} physical Transform objects",
                root.root_asset_name, root.root_path_id
            )),
        }
    }
}

impl Drop for PhysicalBundleCache {
    fn drop(&mut self) {
        if is_owned_audit_temp_dir(&self.temp_root) {
            let _ = fs::remove_dir_all(&self.temp_root);
        }
    }
}

fn is_owned_audit_temp_dir(path: &Path) -> bool {
    let has_owned_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.starts_with("ffone-root-transform-audit-"));
    if !has_owned_name || !path.is_dir() {
        return false;
    }
    let Some(parent) = path.parent() else {
        return false;
    };
    let Ok(parent) = fs::canonicalize(parent) else {
        return false;
    };
    let Ok(temp_root) = fs::canonicalize(std::env::temp_dir()) else {
        return false;
    };
    parent == temp_root
}

/// Audit every `ready` logical root and atomically replace `report_path`.
/// Root-level failures stay in the report so callers can inspect complete
/// evidence and then return a non-zero status via [`LegacyRootTransformAudit::passed`].
pub fn audit_logical_model_root_transforms(
    plan_path: impl AsRef<Path>,
    report_path: impl AsRef<Path>,
) -> Result<LegacyRootTransformAudit, String> {
    let plan_path = plan_path.as_ref();
    let report_path = report_path.as_ref();
    let bytes = fs::read(plan_path).map_err(|err| {
        format!(
            "could not read logical-model plan {}: {err}",
            plan_path.display()
        )
    })?;
    let plan: JsonValue = serde_json::from_slice(&bytes).map_err(|err| {
        format!(
            "could not parse logical-model plan {}: {err}",
            plan_path.display()
        )
    })?;
    let schema = plan.get("schema").and_then(JsonValue::as_str);
    if schema != Some(LOGICAL_MODEL_PLAN_SCHEMA) {
        return Err(format!(
            "logical-model plan schema must be {LOGICAL_MODEL_PLAN_SCHEMA:?}, found {schema:?}"
        ));
    }
    let logical_roots = plan
        .get("logicalRoots")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "logical-model plan has no logicalRoots array".to_string())?;
    let planned_ready_roots = logical_roots
        .iter()
        .filter(|root| root.get("status").and_then(JsonValue::as_str) == Some("ready"))
        .count();

    let source_plan = readable_absolute_path(plan_path);
    let mut report = LegacyRootTransformAudit {
        schema: LEGACY_ROOT_TRANSFORM_AUDIT_SCHEMA,
        source_plan,
        coordinate_contract: RootCoordinateContract::default(),
        counts: RootTransformAuditCounts {
            planned_ready_roots,
            ..RootTransformAuditCounts::default()
        },
        errors: Vec::new(),
        roots: Vec::new(),
    };

    if let Some(declared) = plan
        .pointer("/counts/readyLogicalRootCount")
        .and_then(JsonValue::as_u64)
    {
        if usize::try_from(declared).ok() != Some(planned_ready_roots) {
            report.errors.push(RootTransformAuditError::new(
                None,
                "readyRootCountMismatch",
                format!(
                    "plan declares {declared} ready roots but logicalRoots contains {planned_ready_roots}"
                ),
            ));
        }
    }

    let mut ready_roots = Vec::with_capacity(planned_ready_roots);
    let mut seen_routes = BTreeSet::new();
    for root in logical_roots
        .iter()
        .filter(|root| root.get("status").and_then(JsonValue::as_str) == Some("ready"))
    {
        match parse_ready_root(root) {
            Ok(spec) if seen_routes.insert(spec.legacy_route.clone()) => ready_roots.push(spec),
            Ok(spec) => report.errors.push(RootTransformAuditError::new(
                Some(&spec.legacy_route),
                "duplicateReadyRoute",
                "ready logical-model route occurs more than once",
            )),
            Err(error) => report.errors.push(error),
        }
    }

    let mut cache = PhysicalBundleCache::create()?;
    for root in ready_roots {
        let located = match cache.locate_transform(&root) {
            Ok(located) => located,
            Err(detail) => {
                report.errors.push(RootTransformAuditError::new(
                    Some(&root.legacy_route),
                    if detail.contains("ambiguous") {
                        "ambiguousRootTransform"
                    } else {
                        "missingRootTransform"
                    },
                    detail,
                ));
                continue;
            }
        };
        let unity = match read_root_trs(&located.body) {
            Ok(trs) => trs,
            Err(detail) => {
                report.errors.push(RootTransformAuditError::new(
                    Some(&root.legacy_route),
                    "invalidRootTransform",
                    detail,
                ));
                continue;
            }
        };
        let native = unity_to_native_trs(unity);
        let classification = classify_root_trs(unity);
        if !classification.finite {
            report.errors.push(RootTransformAuditError::new(
                Some(&root.legacy_route),
                "nonfiniteRootTransform",
                "root TRS contains NaN or infinity",
            ));
        }
        if !classification.uniform_scale {
            report.errors.push(RootTransformAuditError::new(
                Some(&root.legacy_route),
                "nonuniformRootScale",
                format!("authored root scale {:?} is not uniform", unity.scale),
            ));
        }
        if !classification.positive_scale {
            report.errors.push(RootTransformAuditError::new(
                Some(&root.legacy_route),
                "nonpositiveRootScale",
                format!(
                    "authored root scale {:?} is not strictly positive",
                    unity.scale
                ),
            ));
        }
        report.roots.push(RootTransformAuditEntry {
            legacy_route: root.legacy_route,
            true_root_m_name: root.true_root_name,
            bundle: portable_file_name(&located.physical_bundle_path),
            asset_name: root.root_asset_name,
            transform_path_id: root.root_path_id,
            unity,
            native,
            classification,
            physical_bundle_path: located.physical_bundle_path,
        });
    }
    drop(cache);

    report.roots.sort_by(|left, right| {
        left.legacy_route
            .cmp(&right.legacy_route)
            .then(left.asset_name.cmp(&right.asset_name))
            .then(left.transform_path_id.cmp(&right.transform_path_id))
    });
    report.errors.sort_by(|left, right| {
        left.legacy_route
            .cmp(&right.legacy_route)
            .then(left.code.cmp(&right.code))
            .then(left.detail.cmp(&right.detail))
    });
    recompute_counts(&mut report);
    write_json_atomically(report_path, &report)?;
    Ok(report)
}

fn parse_ready_root(value: &JsonValue) -> Result<ReadyRootSpec, RootTransformAuditError> {
    let route = value
        .pointer("/kfm/exactRoute")
        .and_then(JsonValue::as_str)
        .unwrap_or("<missing-ready-route>");
    let malformed =
        |detail: String| RootTransformAuditError::new(Some(route), "malformedReadyRoot", detail);
    let required_string = |pointer: &str| {
        value
            .pointer(pointer)
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| malformed(format!("missing non-empty {pointer}")))
    };
    let legacy_route = required_string("/kfm/exactRoute")?;
    let owner_bundle_path = required_string("/kfm/owner/bundlePath")?;
    let owner_asset_name = required_string("/kfm/owner/assetName")?;
    let true_root_name = required_string("/payload/selfContainedGameObject/rootName")?;
    let root_asset_name =
        required_string("/payload/selfContainedGameObject/rootTransform/assetName")?;
    let root_path_id = value
        .pointer("/payload/selfContainedGameObject/rootTransform/pathId")
        .and_then(JsonValue::as_i64)
        .ok_or_else(|| malformed("missing integer rootTransform.pathId".to_string()))?;
    let object_type = required_string("/payload/selfContainedGameObject/rootTransform/objectType")?;
    if object_type != "Transform" {
        return Err(malformed(format!(
            "rootTransform.objectType is {object_type:?}, expected \"Transform\""
        )));
    }
    let dependency_archives = value
        .pointer("/payload/preloadOwnership/dependencyArchives")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .map(|dependency| DependencyArchiveSpec {
            archive_name: dependency
                .get("archiveName")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .to_string(),
            candidate_bundle_paths: dependency
                .get("candidateBundlePaths")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten()
                .filter_map(JsonValue::as_str)
                .filter(|path| !path.is_empty())
                .map(str::to_string)
                .collect(),
        })
        .collect();
    Ok(ReadyRootSpec {
        legacy_route,
        true_root_name,
        owner_bundle_path,
        owner_asset_name,
        root_asset_name,
        root_path_id,
        dependency_archives,
    })
}

fn root_candidate_bundle_paths(root: &ReadyRootSpec) -> Result<Vec<String>, String> {
    if root.root_asset_name == root.owner_asset_name {
        return Ok(vec![root.owner_bundle_path.clone()]);
    }
    let target_archive = archive_identity(&root.root_asset_name);
    let mut paths = BTreeSet::new();
    for dependency in &root.dependency_archives {
        if archive_identity(&dependency.archive_name) == target_archive {
            paths.extend(dependency.candidate_bundle_paths.iter().cloned());
        }
    }
    if paths.is_empty() {
        // Old plans may spell an archive differently. Searching the complete
        // ownership-proven dependency set remains deterministic and fails
        // closed on zero or multiple physical matches.
        for dependency in &root.dependency_archives {
            paths.extend(dependency.candidate_bundle_paths.iter().cloned());
        }
    }
    if paths.is_empty() {
        return Err(format!(
            "root Transform asset {:?} differs from owner asset {:?}, but the plan has no dependency candidate bundle",
            root.root_asset_name, root.owner_asset_name
        ));
    }
    Ok(paths.into_iter().collect())
}

fn archive_identity(value: &str) -> String {
    let normalized = value
        .trim()
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(value)
        .to_ascii_lowercase();
    normalized
        .strip_prefix("customassetbundle-")
        .unwrap_or(&normalized)
        .trim_end_matches(".assets")
        .to_string()
}

fn physical_path_key(value: &str) -> String {
    value
        .trim_start_matches(r"\\?\")
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn readable_absolute_path(path: &Path) -> String {
    let value = fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
    }
}

fn portable_file_name(value: &str) -> String {
    value
        .rsplit(['\\', '/'])
        .find(|part| !part.is_empty())
        .unwrap_or(value)
        .to_string()
}

fn read_root_trs(body: &UnityValue) -> Result<RootTrs, String> {
    Ok(RootTrs {
        translation: read_vec3(body.get("m_LocalPosition"), "m_LocalPosition")?,
        rotation: read_quat(body.get("m_LocalRotation"), "m_LocalRotation")?,
        scale: read_vec3(body.get("m_LocalScale"), "m_LocalScale")?,
    })
}

fn read_vec3(value: Option<&UnityValue>, field: &str) -> Result<[f64; 3], String> {
    let value = value.ok_or_else(|| format!("Transform has no {field}"))?;
    Ok([
        required_component(value, field, "x")?,
        required_component(value, field, "y")?,
        required_component(value, field, "z")?,
    ])
}

fn read_quat(value: Option<&UnityValue>, field: &str) -> Result<[f64; 4], String> {
    let value = value.ok_or_else(|| format!("Transform has no {field}"))?;
    Ok([
        required_component(value, field, "x")?,
        required_component(value, field, "y")?,
        required_component(value, field, "z")?,
        required_component(value, field, "w")?,
    ])
}

fn required_component(value: &UnityValue, field: &str, component: &str) -> Result<f64, String> {
    value
        .get(component)
        .and_then(UnityValue::as_f64)
        .ok_or_else(|| format!("Transform {field}.{component} is missing or non-numeric"))
}

fn unity_to_native_trs(unity: RootTrs) -> RootTrs {
    let translation = unity_to_native_vec3((
        unity.translation[0],
        unity.translation[1],
        unity.translation[2],
    ));
    let rotation = unity_to_native_quaternion((
        unity.rotation[0],
        unity.rotation[1],
        unity.rotation[2],
        unity.rotation[3],
    ));
    let scale = unity_to_native_scale((unity.scale[0], unity.scale[1], unity.scale[2]));
    RootTrs {
        translation: [translation.0, translation.1, translation.2],
        rotation: [rotation.0, rotation.1, rotation.2, rotation.3],
        scale: [scale.0, scale.1, scale.2],
    }
}

fn classify_root_trs(trs: RootTrs) -> RootTransformClassification {
    let finite = trs
        .translation
        .into_iter()
        .chain(trs.rotation)
        .chain(trs.scale)
        .all(f64::is_finite);
    let uniform_scale =
        approx_eq(trs.scale[0], trs.scale[1]) && approx_eq(trs.scale[0], trs.scale[2]);
    let unit_scale = trs.scale.into_iter().all(|value| approx_eq(value, 1.0));
    let positive_scale = trs.scale.into_iter().all(|value| value > 0.0);
    let nonzero_authored_origin = trs
        .translation
        .into_iter()
        .any(|value| value.abs() > APPROX_EPSILON);
    let identity_authored_rotation = approx_eq(trs.rotation[0], 0.0)
        && approx_eq(trs.rotation[1], 0.0)
        && approx_eq(trs.rotation[2], 0.0)
        && (approx_eq(trs.rotation[3], 1.0) || approx_eq(trs.rotation[3], -1.0));
    RootTransformClassification {
        finite,
        uniform_scale,
        unit_scale,
        positive_scale,
        nonzero_authored_origin,
        identity_authored_rotation,
    }
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= APPROX_EPSILON
}

fn recompute_counts(report: &mut LegacyRootTransformAudit) {
    let mut unique = BTreeSet::new();
    let mut counts = RootTransformAuditCounts {
        planned_ready_roots: report.counts.planned_ready_roots,
        audited_roots: report.roots.len(),
        errors: report.errors.len(),
        ..RootTransformAuditCounts::default()
    };
    for root in &report.roots {
        let class = root.classification;
        counts.unit_scale_roots += usize::from(class.unit_scale);
        counts.nonunit_scale_roots += usize::from(!class.unit_scale);
        counts.nonuniform_scale_roots += usize::from(!class.uniform_scale);
        counts.nonpositive_scale_roots += usize::from(!class.positive_scale);
        counts.nonzero_origin_roots += usize::from(class.nonzero_authored_origin);
        counts.nonidentity_rotation_roots += usize::from(!class.identity_authored_rotation);
        counts.nonfinite_roots += usize::from(!class.finite);
        unique.insert((
            physical_path_key(&root.physical_bundle_path),
            root.asset_name.clone(),
            root.transform_path_id,
        ));
    }
    counts.unique_physical_root_transforms = unique.len();
    report.counts = counts;
}

fn write_json_atomically(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "could not create report directory {}: {err}",
            parent.display()
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("root-transform-audit.json");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock error: {err}"))?
        .as_nanos();
    let temporary = parent.join(format!(".{file_name}.tmp.{}.{nanos}", process::id()));
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|err| format!("could not encode {}: {err}", path.display()))?;
    bytes.push(b'\n');
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|err| format!("could not create {}: {err}", temporary.display()))?;
        file.write_all(&bytes)
            .map_err(|err| format!("could not write {}: {err}", temporary.display()))?;
        file.sync_all()
            .map_err(|err| format!("could not sync {}: {err}", temporary.display()))?;
        atomic_replace(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(source, destination).map_err(|err| {
        format!(
            "could not atomically replace {} with {}: {err}",
            destination.display(),
            source.display()
        )
    })
}

#[cfg(windows)]
fn atomic_replace(source: &Path, destination: &Path) -> Result<(), String> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
    }
    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }
    let source = wide(source.as_os_str());
    let destination = wide(destination.as_os_str());
    // SAFETY: both pointers are live NUL-terminated UTF-16 buffers, and the
    // temporary report resides in the destination directory/same volume.
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(format!(
            "could not atomically replace report: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
