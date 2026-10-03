use super::super::*;

pub(in super::super) fn patch_table_data_asset_file(
    extracted_dir: &Path,
    asset_name: &str,
    replacements: &BTreeMap<i64, PathBuf>,
) -> Result<usize, String> {
    let extracted_dir = extracted_dir.to_path_buf();
    let asset_name = asset_name.to_string();
    let replacements = replacements.clone();
    run_table_data_task(move || {
        patch_table_data_asset_file_impl(&extracted_dir, &asset_name, &replacements)
    })
}

pub(in super::super) fn patch_table_data_asset_file_impl(
    extracted_dir: &Path,
    asset_name: &str,
    replacements: &BTreeMap<i64, PathBuf>,
) -> Result<usize, String> {
    let asset_path = extracted_asset_path(extracted_dir, asset_name)?;
    let asset = fusionforge::Asset::from_path(&asset_path)?;
    let mut unity_replacements = BTreeMap::new();
    for (path_id, patch_path) in replacements {
        unity_replacements.insert(*path_id, table_data_patch_value(patch_path, 0)?);
    }
    if unity_replacements.is_empty() {
        return Ok(0);
    }
    let bytes = asset.rebuild_with_object_values(0, &unity_replacements)?;
    fs::write(&asset_path, bytes).map_err(|err| format!("{}: {err}", asset_path.display()))?;
    Ok(unity_replacements.len())
}

pub(in super::super) fn path_from_normalized_relative(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
            )
        })
    {
        return Err(format!("Refusing unsafe relative path: {relative}"));
    }
    Ok(root.join(relative.replace('/', "\\")))
}

pub(in super::super) fn npc_import_manifest_context(project: &Path, manifest: &JsonValue) -> NpcImportManifestContext {
    let source = manifest.get("source");
    let table_data_patch = manifest.get("tableDataPatch");
    NpcImportManifestContext {
        source_table_data_bundle: source
            .and_then(|value| value.get("tableDataBundle"))
            .and_then(JsonValue::as_str)
            .map(|value| resolve_project_path(project, value))
            .or_else(|| project_source_table_data_bundle(project)),
        source_asset: source
            .and_then(|value| value.get("asset"))
            .and_then(JsonValue::as_str)
            .map(ToOwned::to_owned),
        source_path_id: source
            .and_then(|value| value.get("pathId"))
            .and_then(JsonValue::as_i64),
        source_npc_id: source
            .and_then(|value| value.get("npcId"))
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok()),
        table_data_patch_path: table_data_patch
            .and_then(|value| value.get("file"))
            .and_then(JsonValue::as_str)
            .map(|value| resolve_project_path(project, value)),
        source_paths: manifest
            .get("sourceBundles")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter_map(JsonValue::as_str)
            .map(|value| resolve_project_path(project, value))
            .collect(),
    }
}

pub(in super::super) fn migrate_npc_import_manifest_source_table_bundle(manifest: &mut JsonValue) {
    if let Some(source) = manifest
        .get_mut("source")
        .and_then(JsonValue::as_object_mut)
    {
        if let Some(bundle) = source
            .get("tableDataBundle")
            .and_then(JsonValue::as_str)
            .map(|value| value.replace('\\', "/"))
        {
            source.insert("tableDataBundle".to_string(), json!(bundle));
        }
    }
}

pub(in super::super) fn hydrate_npc_import_manifest_source_table_bundle(project: &Path, manifest: &mut JsonValue) {
    let bundle_from_notes = manifest
        .get("blueprint")
        .and_then(|value| value.get("notes"))
        .and_then(JsonValue::as_str)
        .and_then(|notes| {
            notes.lines().find_map(|line| {
                line.trim()
                    .strip_prefix("Source TableData:")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
            })
        })
        .filter(|path| path.is_file());
    let Some(source) = manifest
        .get_mut("source")
        .and_then(JsonValue::as_object_mut)
    else {
        return;
    };
    if let Some(bundle) = bundle_from_notes {
        source.insert(
            "tableDataBundle".to_string(),
            json!(bundle.to_string_lossy().replace('\\', "/")),
        );
        return;
    }
    let has_bundle = source
        .get("tableDataBundle")
        .and_then(JsonValue::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    if has_bundle {
        return;
    }
    let Some(bundle) = project_source_table_data_bundle(project) else {
        return;
    };
    source.insert(
        "tableDataBundle".to_string(),
        json!(bundle.to_string_lossy().replace('\\', "/")),
    );
}

pub(in super::super) fn external_resource_import_metadata_path(spec: &ExternalResourceImportSpec) -> PathBuf {
    spec.project_bundle.with_extension(format!(
        "{}.import.json",
        spec.project_bundle
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("bundle")
    ))
}

pub(in super::super) fn npc_source_assets_from_path(
    project: &Path,
    source_path: &Path,
) -> Result<Vec<fusionforge::Asset>, String> {
    let mut assets = Vec::<fusionforge::Asset>::new();
    if source_path.is_file() && !is_unity_bundle_path(source_path) {
        return Ok(assets);
    }
    let source_dir = if source_path.is_dir() {
        source_path.to_path_buf()
    } else {
        extract_bundle_cached(project, source_path)?
    };
    for file in extracted_files_in_dir(&source_dir) {
        let path = PathBuf::from(&file.path);
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if name.ends_with(".json") {
            continue;
        }
        let Ok(asset) = fusionforge::Asset::from_path(&path) else {
            continue;
        };
        assets.push(asset);
    }
    Ok(assets)
}

pub(in super::super) fn manifest_entries(
    manifest_path: &Path,
    expected_format: &str,
) -> Result<Vec<serde_json::Value>, String> {
    if !manifest_path.exists() {
        return Ok(Vec::new());
    }
    let manifest = serde_json::from_str::<serde_json::Value>(
        &fs::read_to_string(manifest_path).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    if manifest.get("format").and_then(serde_json::Value::as_str) != Some(expected_format) {
        return Err(format!(
            "Unsupported manifest format in {}",
            manifest_path.display()
        ));
    }
    Ok(manifest
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default())
}

pub(in super::super) fn unique_manifest_values(
    entries: &[serde_json::Value],
    key: &str,
    container: Option<&str>,
) -> Vec<String> {
    let mut values = BTreeSet::new();
    for entry in entries {
        if let Some(container) = container {
            if entry.get("container").and_then(serde_json::Value::as_str) != Some(container) {
                continue;
            }
        }
        if let Some(value) = entry.get(key).and_then(serde_json::Value::as_str) {
            values.insert(value.to_string());
        }
    }
    values.into_iter().collect()
}

pub(in super::super) fn build_status_path(extracted_dir: &Path, kind: &str, key: &str) -> PathBuf {
    let parent = extracted_dir.parent().unwrap_or_else(|| Path::new("."));
    let dir_name = extracted_dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("bundle");
    parent.join(format!(
        "{dir_name}_{kind}_status_{}.json",
        stable_hash(key)
    ))
}
