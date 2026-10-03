use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EquipmentSourceManifest {
    pub(super) schema: String,
    pub(super) status: String,
    pub(super) source_root: String,
    pub(super) benchmark_route_limit: Option<u64>,
    pub(super) counts: EquipmentSourceCounts,
    #[serde(default)]
    pub(super) bundles: Vec<Value>,
    pub(super) exported: Vec<EquipmentSourceExport>,
    pub(super) blockers: Vec<EquipmentSourceBlocker>,
}

pub(super) fn route_stem_qualifier(exact_route: &str) -> Result<String> {
    let stem = Path::new(exact_route)
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| equipment_error_value("equipment route has no UTF-8 stem"))?;
    let safe_glb = minimal_windows_glb_filename(stem)
        .map_err(|error| equipment_error_value(error.to_string()))?;
    safe_glb
        .strip_suffix(".glb")
        .map(str::to_owned)
        .ok_or_else(|| equipment_error_value("safe equipment route qualifier has no suffix"))
}

pub(super) fn sibling_manifest_path(source_root: &Path) -> Result<PathBuf> {
    let parent = source_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = source_root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| equipment_error_value("equipment source root has no UTF-8 name"))?;
    Ok(parent.join(format!("{name}.manifest.json")))
}

pub(super) fn portable_path_key(path: &Path) -> Result<String> {
    Ok(clean_relative_components(path)?
        .iter()
        .map(|component| portable_key(component))
        .collect::<Vec<_>>()
        .join("/"))
}

pub(super) fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().into_owned()),
            Component::RootDir => None,
            Component::CurDir => Some(".".to_owned()),
            Component::ParentDir => Some("..".to_owned()),
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn slash_path_checked(path: &Path) -> Result<String> {
    let components = clean_relative_components(path)?;
    Ok(components.join("/"))
}
