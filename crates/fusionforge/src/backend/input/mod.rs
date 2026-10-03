use super::super::*;

pub(in super::super) fn collect_reference_assemblies(
    project: &Path,
    assembly_parent: Option<&Path>,
) -> BTreeMap<String, PathBuf> {
    let mut refs = BTreeMap::new();
    for root in default_script_reference_roots(project, assembly_parent) {
        if !root.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("dll") {
                continue;
            }
            let Ok(Some(name)) = fusionforge::managed::managed_assembly_name(&path) else {
                continue;
            };
            refs.entry(name).or_insert(path);
        }
    }
    refs
}

pub(in super::super) fn find_matching_paren(data: &str, open: usize) -> Option<usize> {
    let bytes = data.as_bytes();
    if bytes.get(open).copied() != Some(b'(') {
        return None;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    for (relative, ch) in data[open..].char_indices() {
        if relative == 0 {
            depth = 1;
            continue;
        }
        if in_string || in_char {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open + relative);
                }
            }
            _ => {}
        }
    }
    None
}

pub(in super::super) fn find_decompiled_csproj(file: &Path) -> Option<PathBuf> {
    let mut current = if file.is_dir() {
        file.to_path_buf()
    } else {
        file.parent()?.to_path_buf()
    };
    loop {
        let mut matches = fs::read_dir(&current)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("csproj"))
            .collect::<Vec<_>>();
        matches.sort();
        if let Some(path) = matches.into_iter().next() {
            return Some(path);
        }
        if !current.pop() {
            return None;
        }
    }
}

pub(in super::super) fn read_cache_metadata(cache_dir: &Path) -> Option<CacheMetadata> {
    let data = fs::read_to_string(cache_dir.join("cache-meta.json")).ok()?;
    serde_json::from_str(&data).ok()
}

pub(in super::super) fn load_tile_document(path: String) -> EditorResult<serde_json::Value> {
    let data = fs::read_to_string(&path).map_err(|err| EditorError::Io(err).to_string())?;
    let mut document: serde_json::Value =
        serde_json::from_str(&data).map_err(|err| EditorError::Json(err).to_string())?;
    migrate_world_document_json(&mut document);
    Ok(document)
}

pub(in super::super) fn collect_table_sections(
    value: &fusionforge::UnityValue,
    prefix: &str,
    output: &mut Vec<TableDataSectionSummary>,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    for (key, item) in object {
        if is_unity_metadata_field(key) {
            continue;
        }
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match item {
            fusionforge::UnityValue::Array(rows) => {
                let fields = table_section_fields(rows);
                output.push(TableDataSectionSummary {
                    path,
                    row_count: rows.len(),
                    field_count: fields.len(),
                    fields,
                });
            }
            fusionforge::UnityValue::Object(_) => collect_table_sections(item, &path, output),
            _ => {}
        }
    }
}

pub(in super::super) fn collect_npc_staged_source_aliases(
    project: &Path,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
) -> BTreeMap<String, Vec<PathBuf>> {
    let stage_root = npc_stage_root(project, blueprint);
    let mut aliases = BTreeMap::<String, Vec<PathBuf>>::new();
    let mut candidates = context.source_paths.clone();
    for candidate in [
        blueprint.model_bundle.as_deref(),
        blueprint.texture_bundle.as_deref(),
        blueprint.icon_bundle.as_deref(),
        blueprint.animation_set.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        candidates.push(resolve_project_path(project, candidate));
    }
    for candidate in candidates {
        if !candidate.starts_with(&stage_root) {
            continue;
        }
        let Some(label) = npc_staged_source_label(&candidate) else {
            continue;
        };
        aliases.entry(label).or_default().push(candidate);
    }
    aliases
}

pub(in super::super) fn collect_value_pointers(
    value: &fusionforge::UnityValue,
    pointers: &mut Vec<fusionforge::Pointer>,
) {
    match value {
        fusionforge::UnityValue::Pointer(pointer) if !pointer.is_null() => {
            pointers.push(pointer.clone());
        }
        fusionforge::UnityValue::Array(items) => {
            for item in items {
                collect_value_pointers(item, pointers);
            }
        }
        fusionforge::UnityValue::Object(values) => {
            for value in values.values() {
                collect_value_pointers(value, pointers);
            }
        }
        fusionforge::UnityValue::Pair(left, right) => {
            collect_value_pointers(left, pointers);
            collect_value_pointers(right, pointers);
        }
        _ => {}
    }
}

pub(in super::super) fn load_table_data_section(
    bundle_path: String,
    project_dir: Option<String>,
    asset: String,
    path_id: i64,
    section_path: String,
    offset: Option<usize>,
    limit: Option<usize>,
) -> EditorResult<serde_json::Value> {
    run_table_data_task(move || {
        load_table_data_section_impl(
            bundle_path,
            project_dir,
            asset,
            path_id,
            section_path,
            offset,
            limit,
        )
    })
}

pub(in super::super) fn load_table_data_section_impl(
    bundle_path: String,
    project_dir: Option<String>,
    asset: String,
    path_id: i64,
    section_path: String,
    offset: Option<usize>,
    limit: Option<usize>,
) -> EditorResult<serde_json::Value> {
    let bundle = PathBuf::from(&bundle_path);
    let project = default_table_data_project_dir(project_dir);
    let extract_dir = extract_bundle_cached(&project, &bundle)?;
    let asset_path = extracted_asset_path(&extract_dir, &asset)?;
    let asset_file = fusionforge::Asset::from_path(&asset_path)?;
    let info = asset_file
        .objects
        .get(&path_id)
        .ok_or_else(|| format!("{}#{} was not found", asset_file.name, path_id))?;
    let body = asset_file.read_object(0, info)?;
    let section = table_value_at_path(&body, &section_path)
        .ok_or_else(|| format!("section was not found: {section_path}"))?;
    let rows = section
        .as_array()
        .ok_or_else(|| format!("{section_path} is not an array"))?;
    let offset = offset.unwrap_or_default().min(rows.len());
    let limit = limit.unwrap_or(50).clamp(1, 250);
    let end = offset.saturating_add(limit).min(rows.len());
    let visible = rows[offset..end]
        .iter()
        .map(unity_value_to_json)
        .collect::<Vec<_>>();
    Ok(json!({
        "bundlePath": bundle_path,
        "cacheDir": extract_dir,
        "asset": asset_file.name,
        "pathId": path_id,
        "sectionPath": section_path,
        "offset": offset,
        "limit": limit,
        "totalRows": rows.len(),
        "fields": table_section_fields(rows),
        "rows": visible,
    }))
}

pub(in super::super) fn collect_file_keys(
    root: &Path,
    current: &Path,
    files: &mut BTreeSet<String>,
) -> Result<(), String> {
    if !current.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(current).map_err(|err| format!("{}: {err}", current.display()))? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_file_keys(root, &path, files)?;
        } else if path.is_file() {
            files.insert(normalized_relative_file(root, &path)?);
        }
    }
    Ok(())
}

pub(in super::super) fn collect_source_snapshot_files(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<String, BuildSourceFileSnapshot>,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|err| format!("{}: {err}", current.display()))? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_source_snapshot_files(root, &path, files)?;
        } else if path.is_file() {
            let metadata = path
                .metadata()
                .map_err(|err| format!("{}: {err}", path.display()))?;
            files.insert(
                normalized_relative_file(root, &path)?,
                BuildSourceFileSnapshot {
                    size: metadata.len(),
                    modified_ms: metadata_modified_ms(&metadata)?,
                },
            );
        }
    }
    Ok(())
}

pub(in super::super) fn read_build_source_snapshot(out_dir: &Path) -> Option<BuildSourceSnapshot> {
    let path = out_dir.join(BUILD_SOURCE_INDEX_FILE);
    let data = fs::read_to_string(path).ok()?;
    let snapshot = serde_json::from_str::<BuildSourceSnapshot>(&data).ok()?;
    (snapshot.format == "fftools.build-source-snapshot.v1").then_some(snapshot)
}

pub(in super::super) fn collect_existing_files_recursive(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if root.is_file() {
        files.push(root.to_path_buf());
        return Ok(());
    }
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root).map_err(|err| format!("{}: {err}", root.display()))? {
        let path = entry.map_err(|err| err.to_string())?.path();
        if path.is_dir() {
            collect_existing_files_recursive(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}
