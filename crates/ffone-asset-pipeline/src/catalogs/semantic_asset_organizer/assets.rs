use super::*;

pub const SEMANTIC_ASSET_ORGANIZATION_SCHEMA: &str = "ffone.semantic-asset-organization-plan.v1";

pub(super) const ASSET_MANIFEST_SCHEMA: &str = "ffone.project-assets.v1";

pub(super) const CONTENT_INDEX_SCHEMA: &str = "ffone.asset-index.v1";

#[derive(Clone, Debug)]
pub struct SemanticAssetOrganizerOptions {
    pub asset_manifest: PathBuf,
    pub content_index: PathBuf,
    pub cook_report: PathBuf,
    pub table_set: PathBuf,
    pub logical_model_plan: PathBuf,
    pub output: PathBuf,
}

impl SemanticAssetOrganizerOptions {
    pub fn new(
        asset_manifest: impl Into<PathBuf>,
        content_index: impl Into<PathBuf>,
        cook_report: impl Into<PathBuf>,
        table_set: impl Into<PathBuf>,
        logical_model_plan: impl Into<PathBuf>,
        output: impl Into<PathBuf>,
    ) -> Self {
        Self {
            asset_manifest: asset_manifest.into(),
            content_index: content_index.into(),
            cook_report: cook_report.into(),
            table_set: table_set.into(),
            logical_model_plan: logical_model_plan.into(),
            output: output.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAssetOrganizationReport {
    pub schema: String,
    pub mode: String,
    pub status: String,
    pub production_assets_mutated: bool,
    pub taxonomy: Vec<String>,
    pub inputs: Vec<InputEvidence>,
    pub counts: OrganizerCounts,
    pub entities: Vec<EntityProposal>,
    pub models: Vec<ModelProposal>,
    pub route_scale_usage: Vec<RouteScaleUsage>,
    pub shared_assets: Vec<SharedNativeAsset>,
    pub unresolved_routes: Vec<UnresolvedRoute>,
    pub blockers: Vec<OrganizationBlocker>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeAssetReference {
    pub native_key: String,
    pub content_path: String,
    pub project_path: Option<String>,
    pub mapping_occurrences: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteScaleUsage {
    pub legacy_route: String,
    pub entity_count: u64,
    pub values: Vec<f64>,
    pub multiple_values: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedNativeAsset {
    pub native: NativeAssetReference,
    pub entity_count: u64,
    pub categories: Vec<SemanticCategory>,
    pub legacy_routes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnresolvedRoute {
    pub legacy_route: String,
    pub referenced_entity_ids: Vec<String>,
    pub logical_plan_codes: Vec<String>,
    pub logical_plan_details: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PlanRouteBlockers {
    pub(super) codes: BTreeSet<String>,
    pub(super) details: BTreeSet<String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct NativeIndex {
    pub(super) by_kind_name: BTreeMap<(String, String), Vec<NativeAssetReference>>,
    pub(super) evidence_issues: Vec<String>,
}

pub(super) fn build_native_index(manifest: &Value, content: &Value, cook: &Value) -> NativeIndex {
    let project_by_source = value_array(manifest.get("files"))
        .iter()
        .filter_map(|file| {
            Some((
                normalize_slashes(file.get("source_path")?.as_str()?),
                normalize_slashes(file.get("path")?.as_str()?),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let content_by_key = value_array(content.get("assets"))
        .iter()
        .filter_map(|asset| Some((asset.get("key")?.as_str()?.to_owned(), asset.clone())))
        .collect::<BTreeMap<_, _>>();
    let mut grouped =
        BTreeMap::<(String, String), BTreeMap<(String, String), NativeAssetReference>>::new();
    let mut evidence_issues = BTreeSet::new();
    for (index, mapping) in value_array(cook.get("mappings")).iter().enumerate() {
        let Some(kind) = mapping.get("kind").and_then(Value::as_str) else {
            evidence_issues.insert(format!("cook mapping {index} has no kind"));
            continue;
        };
        // This character organizer owns only runtime texture and audio dependencies.
        // Aggregated localization/table mappings intentionally use per-source keys that do
        // not identify the single native aggregate in the content index, and are outside
        // this taxonomy rather than broken character evidence.
        if !matches!(kind, "texture" | "audio") {
            continue;
        }
        let Some(name) = mapping.get("name").and_then(Value::as_str) else {
            evidence_issues.insert(format!("cook mapping {index} has no exact name"));
            continue;
        };
        let Some(native_key) = mapping.get("nativeKey").and_then(Value::as_str) else {
            evidence_issues.insert(format!("cook mapping {index} has no nativeKey"));
            continue;
        };
        let Some(native_path) = mapping.get("nativePath").and_then(Value::as_str) else {
            evidence_issues.insert(format!("cook mapping {index} has no nativePath"));
            continue;
        };
        let native_path = normalize_slashes(native_path);
        let Some(indexed) = content_by_key.get(native_key) else {
            evidence_issues.insert(format!(
                "cook mapping {index} nativeKey {native_key} is absent from content index"
            ));
            continue;
        };
        let indexed_kind = indexed
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let indexed_path = indexed
            .get("path")
            .and_then(Value::as_str)
            .map(normalize_slashes)
            .unwrap_or_default();
        // The content index has one canonical display name per native key, while the cook
        // report intentionally records every source m_Name alias that produced that key.
        // Key + kind + path prove native-file identity; name remains only a fail-closed
        // candidate lookup and never proves an AssetLoader route target on its own.
        if indexed_kind != kind || indexed_path != native_path {
            evidence_issues.insert(format!(
                "cook mapping {index} contradicts content index for nativeKey {native_key}"
            ));
            continue;
        }
        let project_path = project_by_source.get(&native_path).cloned();
        if project_path.is_none() {
            evidence_issues.insert(format!(
                "content path {native_path:?} has no exact asset-manifest source_path"
            ));
        }
        let candidate = grouped
            .entry((kind.to_owned(), name.to_owned()))
            .or_default()
            .entry((native_key.to_owned(), native_path.clone()))
            .or_insert_with(|| NativeAssetReference {
                native_key: native_key.to_owned(),
                content_path: native_path,
                project_path,
                mapping_occurrences: 0,
            });
        candidate.mapping_occurrences += 1;
    }
    NativeIndex {
        by_kind_name: grouped
            .into_iter()
            .map(|(key, values)| (key, values.into_values().collect()))
            .collect(),
        evidence_issues: evidence_issues.into_iter().collect(),
    }
}

#[derive(Clone, Debug)]
pub(super) struct FieldRoute {
    pub(super) legacy_route: String,
    pub(super) source_name: String,
}

pub(super) fn route_from_field(row: &Value, field: &str, prefix: &str, extension: &str) -> Option<FieldRoute> {
    make_route(prefix, string_field(row, field)?, extension)
}

pub(super) fn make_route(prefix: &str, value: &str, extension: &str) -> Option<FieldRoute> {
    let source_name = asset_stem(value)?;
    let prefix = normalize_route(prefix).trim_matches('/').to_owned();
    let mut route = normalize_route(value);
    if route.is_empty() {
        return None;
    }
    let expected = extension.trim_start_matches('.').to_ascii_lowercase();
    let current = route
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match current.as_deref() {
        Some(current) if current == expected => {}
        Some(current) if matches!(current, "nif" | "kfm" | "dds" | "wav") => {
            route.truncate(route.len().saturating_sub(current.len()));
            route.push_str(&expected);
        }
        Some(_) => {}
        None => {
            route.push('.');
            route.push_str(&expected);
        }
    }
    if route != prefix && !route.starts_with(&format!("{prefix}/")) {
        route = format!("{prefix}/{}", route.trim_start_matches('/'));
    }
    Some(FieldRoute {
        legacy_route: route,
        source_name,
    })
}

pub(super) fn asset_stem(value: &str) -> Option<String> {
    let clean = clean_value(value)?;
    let normalized = clean.replace('\\', "/");
    let name = normalized.rsplit('/').next()?;
    let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
    let stem = if matches!(
        extension.to_ascii_lowercase().as_str(),
        "nif" | "kfm" | "dds" | "wav"
    ) {
        stem
    } else {
        name
    };
    (!stem.is_empty()).then(|| stem.to_owned())
}

pub(super) fn route_directory(route: &str) -> String {
    let mut components = normalize_route(route)
        .split('/')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if !components.is_empty() {
        components.remove(0);
    }
    if let Some(last) = components.last_mut() {
        if let Some((stem, extension)) = last.rsplit_once('.') {
            if matches!(extension, "kfm" | "nif") {
                *last = stem.to_owned();
            }
        }
    }
    components.join("/")
}

pub(super) fn positive_index(value: &Value, key: &str) -> Option<usize> {
    usize::try_from(int_field(value, key)?)
        .ok()
        .filter(|value| *value > 0)
}

pub(super) fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(env::current_dir()
            .map_err(|error| io_at(".", error))?
            .join(path))
    }
}

pub(super) fn same_absolute_path(left: &Path, right: &Path) -> Result<bool> {
    Ok(absolute_path(left)? == absolute_path(right)?)
}
