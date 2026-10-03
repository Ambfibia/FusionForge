use super::*;

pub const OBJECT_ROUTE_NORMALIZATION_SCHEMA: &str = "ffone.object-route-normalization.v1";

#[derive(Clone, Debug)]
pub struct ObjectRouteNormalizationOptions {
    pub project_root: PathBuf,
    pub report_path: PathBuf,
    pub apply: bool,
}

impl ObjectRouteNormalizationOptions {
    #[must_use]
    pub fn new(
        project_root: impl Into<PathBuf>,
        report_path: impl Into<PathBuf>,
        apply: bool,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            report_path: report_path.into(),
            apply,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectRouteNormalizationReport {
    pub schema: String,
    pub mode: String,
    pub source_alias: String,
    pub source_build: String,
    pub policy: String,
    pub counts: ObjectRouteNormalizationCounts,
    pub path_lengths: ObjectRoutePathLengths,
    pub source_catalog_blake3: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_catalog_blake3: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<WorldPrefabVerification>,
    pub routes: Vec<ObjectPackageRoute>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectRouteNormalizationCounts {
    pub packages: u64,
    pub members: u64,
    pub textures: u64,
    pub files: u64,
    pub renamed_member_directories: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectRoutePathLengths {
    pub maximum_before: u64,
    pub maximum_after: u64,
    pub over_160_before: u64,
    pub over_160_after: u64,
    pub over_180_before: u64,
    pub over_180_after: u64,
    pub maximum_component_after: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPackageRoute {
    pub resource_set_id: String,
    pub category: String,
    pub source: String,
    pub destination: String,
}

pub(super) fn path_length_report(routes: &BTreeMap<String, String>) -> ObjectRoutePathLengths {
    let sources = routes.keys().map(String::as_str).collect::<Vec<_>>();
    let destinations = routes.values().map(String::as_str).collect::<Vec<_>>();
    ObjectRoutePathLengths {
        maximum_before: sources.iter().map(|path| path.len()).max().unwrap_or(0) as u64,
        maximum_after: destinations
            .iter()
            .map(|path| path.len())
            .max()
            .unwrap_or(0) as u64,
        over_160_before: sources.iter().filter(|path| path.len() > 160).count() as u64,
        over_160_after: destinations.iter().filter(|path| path.len() > 160).count() as u64,
        over_180_before: sources.iter().filter(|path| path.len() > 180).count() as u64,
        over_180_after: destinations.iter().filter(|path| path.len() > 180).count() as u64,
        maximum_component_after: destinations
            .iter()
            .flat_map(|path| path.split('/'))
            .map(str::len)
            .max()
            .unwrap_or(0) as u64,
    }
}

pub(super) fn insert_route(
    routes: &mut BTreeMap<String, String>,
    source: String,
    destination: String,
) -> Result<()> {
    if source == destination {
        return invalid(format!("normalization route is unchanged: {source}"));
    }
    if let Some(old) = routes.insert(source.clone(), destination.clone())
        && old != destination
    {
        return invalid(format!(
            "normalization source {source:?} maps to both {old:?} and {destination:?}"
        ));
    }
    Ok(())
}

pub(super) fn stage_object_path(stage: &Path, route: &str) -> Result<PathBuf> {
    let relative = route
        .strip_prefix("objects/")
        .ok_or_else(|| invalid_error(format!("object route escaped objects/: {route:?}")))?;
    checked_join(stage, relative)
}

pub(super) fn checked_absolute_path(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("output path has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    let parent = fs::canonicalize(parent).map_err(|error| io_at(parent, error))?;
    let name = path
        .file_name()
        .ok_or_else(|| invalid_error("output path has no filename"))?;
    Ok(parent.join(name))
}

pub(super) fn asset_relative(asset_root: &Path, path: &Path) -> Result<String> {
    Ok(slash_path(path.strip_prefix(asset_root).map_err(|_| {
        invalid_error(format!("asset escaped game root: {}", path.display()))
    })?))
}

pub(super) fn relative_path(from: &Path, to: &Path) -> Result<String> {
    let from = normal_components(from)?;
    let to = normal_components(to)?;
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut parts = vec!["..".to_owned(); from.len() - common];
    parts.extend(to[common..].iter().cloned());
    if parts.is_empty() {
        return invalid("relative URI resolved to an empty path");
    }
    Ok(parts.join("/"))
}

pub(super) fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
