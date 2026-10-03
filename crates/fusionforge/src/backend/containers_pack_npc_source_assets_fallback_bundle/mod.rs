use super::super::*;

pub(in super::super) fn npc_container_named_object_pointer(
    env: &fusionforge::UnityEnvironment,
    container_path: &str,
) -> Option<(fusionforge::Pointer, i32)> {
    let expected_types = npc_container_expected_object_types(container_path);
    let expected_name = asset_stem_from_container_path(container_path)?.to_ascii_lowercase();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            let object_type = asset.object_type_name(info);
            if !expected_types.is_empty() && !expected_types.contains(&object_type.as_str()) {
                continue;
            }
            let Ok(value) = asset.read_object(asset_index, info) else {
                continue;
            };
            if fusionforge::object_name(&value).to_ascii_lowercase() != expected_name {
                continue;
            }
            return Some((
                fusionforge::Pointer {
                    source_asset: asset_index,
                    file_id: 0,
                    path_id: info.path_id,
                },
                2,
            ));
        }
    }
    None
}

pub(in super::super) fn npc_source_container_paths_by_key(
    env: &fusionforge::UnityEnvironment,
) -> BTreeMap<(usize, i64), BTreeSet<String>> {
    let mut paths = BTreeMap::<(usize, i64), BTreeSet<String>>::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let Ok(body) = asset.read_object(asset_index, info) else {
                continue;
            };
            for entry in fusionforge::value_array(body.get("m_Container")) {
                let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                    continue;
                };
                let Some(pointer) = metadata
                    .get("asset")
                    .and_then(fusionforge::UnityValue::as_pointer)
                else {
                    continue;
                };
                for key in pointer_candidate_keys_for_rewrite(env, pointer) {
                    paths
                        .entry(key)
                        .or_default()
                        .insert(normalized_asset_path(path));
                }
            }
        }
    }
    paths
}

pub(in super::super) fn npc_external_ref_for_source_object(
    env: &fusionforge::UnityEnvironment,
    key: (usize, i64),
    source_container_paths: &BTreeMap<(usize, i64), BTreeSet<String>>,
    required_container_paths: &BTreeSet<String>,
    target_index: &NpcTargetExternalIndex,
) -> Option<NpcTargetExternalRef> {
    if npc_key_has_required_container_path(&key, source_container_paths, required_container_paths) {
        return None;
    }
    let asset = env.assets.get(key.0)?;
    let info = asset.objects.get(&key.1)?;
    let body = asset.read_object(key.0, info).ok()?;
    let object_type = asset.object_type_name(info);
    match object_type.as_str() {
        "Shader" => {
            if let Some(bytes) = object_payload_bytes(&body) {
                if let Some(reference) = target_index
                    .shader_by_fingerprint
                    .get(&npc_payload_fingerprint(&bytes))
                {
                    return Some(reference.clone());
                }
            }
            npc_shader_name_from_body(&body)
                .and_then(|name| target_index.shader_by_name.get(&name).cloned())
        }
        "Texture2D" => {
            if let Some(paths) = source_container_paths.get(&key) {
                for path in paths {
                    if let Some(reference) = target_index.by_container_path.get(path) {
                        return Some(reference.clone());
                    }
                }
            }
            None
        }
        _ => None,
    }
}

pub(in super::super) fn inspect_table_data_bundle(
    bundle_path: String,
    project_dir: Option<String>,
) -> EditorResult<TableDataInspection> {
    run_table_data_task(move || inspect_table_data_bundle_impl(bundle_path, project_dir))
}

pub(in super::super) fn inspect_table_data_bundle_impl(
    bundle_path: String,
    project_dir: Option<String>,
) -> EditorResult<TableDataInspection> {
    use fusionforge::{object_name, Asset};

    let bundle = PathBuf::from(&bundle_path);
    if !bundle.is_file() {
        return Err(EditorError::MissingPath(bundle_path).to_string());
    }
    let project = default_table_data_project_dir(project_dir);
    let extract_dir = extract_bundle_cached(&project, &bundle)?;
    let mut assets = Vec::new();
    for extracted in extracted_files_in_dir(&extract_dir) {
        let asset_path = PathBuf::from(&extracted.path);
        let asset = match Asset::from_path(&asset_path) {
            Ok(asset) => asset,
            Err(_) => continue,
        };
        let mut objects = Vec::new();
        for (path_id, info) in &asset.objects {
            let object_type = asset.object_type_name(info);
            if object_type != "MonoBehaviour" {
                continue;
            }
            let mut errors = Vec::new();
            let body = match asset.read_object(0, info) {
                Ok(value) => value,
                Err(err) => {
                    errors.push(err);
                    fusionforge::UnityValue::Object(BTreeMap::new())
                }
            };
            let mut sections = Vec::new();
            collect_table_sections(&body, "", &mut sections);
            if sections.is_empty() && errors.is_empty() {
                continue;
            }
            let row_count = sections.iter().map(|section| section.row_count).sum();
            let mut string_count = 0usize;
            let mut number_count = 0usize;
            count_table_scalars(&body, &mut string_count, &mut number_count);
            objects.push(TableDataObjectSummary {
                asset: asset.name.clone(),
                path_id: *path_id,
                name: object_name(&body),
                object_type,
                section_count: sections.len(),
                row_count,
                string_count,
                number_count,
                sections,
                errors,
            });
        }
        if !objects.is_empty() {
            assets.push(TableDataAssetSummary {
                asset: asset.name,
                objects,
            });
        }
    }

    Ok(TableDataInspection {
        bundle_path,
        cache_dir: extract_dir.to_string_lossy().to_string(),
        assets,
    })
}

pub(in super::super) fn export_table_data_object(
    bundle_path: String,
    project_dir: Option<String>,
    asset: String,
    path_id: i64,
    output_path: String,
) -> EditorResult<String> {
    run_table_data_task(move || {
        export_table_data_object_impl(bundle_path, project_dir, asset, path_id, output_path)
    })
}

pub(in super::super) fn export_table_data_object_impl(
    bundle_path: String,
    project_dir: Option<String>,
    asset: String,
    path_id: i64,
    output_path: String,
) -> EditorResult<String> {
    use fusionforge::object_name;

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
    let document = json!({
        "format": "fftools.tabledata-object.v1",
        "sourceBundle": bundle.to_string_lossy(),
        "container": container_name_from_bundle(&bundle),
        "asset": asset_file.name,
        "pathId": path_id,
        "name": object_name(&body),
        "objectType": asset_file.object_type_name(info),
        "value": unity_value_to_json(&body),
    });
    let output = PathBuf::from(&output_path);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    let data = serde_json::to_string_pretty(&document)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(&output, format!("{data}\n")).map_err(|err| EditorError::Io(err).to_string())?;
    Ok(output.to_string_lossy().to_string())
}

pub(in super::super) fn dump_xdt_json_from_table_data_bundle(
    project: &Path,
    table_data_bundle: &Path,
) -> Result<Option<JsonValue>, String> {
    if !table_data_bundle.is_file() {
        return Ok(None);
    }
    let extract_dir = extract_bundle_cached(project, table_data_bundle)?;
    for extracted in extracted_files_in_dir(&extract_dir) {
        let asset_path = PathBuf::from(&extracted.path);
        let asset = match fusionforge::Asset::from_path(&asset_path) {
            Ok(asset) => asset,
            Err(_) => continue,
        };
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "MonoBehaviour" {
                continue;
            }
            let body = asset.read_object(0, info)?;
            if body.get("m_pNpcTable").is_some() || body.get("m_pAnimationTable").is_some() {
                return Ok(Some(unity_value_to_json(&body)));
            }
        }
    }
    Ok(None)
}

pub(in super::super) fn update_server_tdata_from_table_bundle(
    project: &Path,
    table_data_bundle: &Path,
    server_tdata_path: &Path,
) -> Result<Option<PathBuf>, String> {
    let target = if server_tdata_path.is_dir()
        || server_tdata_path
            .extension()
            .and_then(|value| value.to_str())
            .is_none()
    {
        server_tdata_path.join("xdt.json")
    } else {
        server_tdata_path.to_path_buf()
    };
    let Some(document) = dump_xdt_json_from_table_data_bundle(project, table_data_bundle)? else {
        return Ok(None);
    };
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let data = serde_json::to_string_pretty(&document)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(&target, format!("{data}\n"))
        .map_err(|err| format!("{}: {err}", target.display()))?;
    Ok(Some(target))
}

pub(in super::super) fn apply_table_data_patch_to_bundle(
    input_bundle: String,
    output_bundle: String,
    asset: String,
    path_id: i64,
    patch_path: String,
) -> EditorResult<String> {
    let input = PathBuf::from(&input_bundle);
    if !input.is_file() {
        return Err(EditorError::MissingPath(input_bundle).to_string());
    }
    let output = PathBuf::from(&output_bundle);
    let patch = PathBuf::from(&patch_path);
    if !patch.is_file() {
        return Err(EditorError::MissingPath(patch_path).to_string());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    let temp = native_build_temp_dir("tabledata")?;
    extract_bundle_native_to_dir(&input, temp.path())?;
    let replacements = BTreeMap::from([(path_id, patch)]);
    let patched = patch_table_data_asset_file(temp.path(), &asset, &replacements)?;
    if patched == 0 {
        return Err("No TableData objects were patched.".to_string());
    }
    pack_bundle_native_from_dir(temp.path(), &output)?;
    Ok(format!(
        "Patched TableData bundle: {}",
        output.to_string_lossy()
    ))
}

pub(in super::super) fn reset_build_container_from_source(
    source: &Path,
    out_dir: &Path,
    container: &str,
) -> Result<(), String> {
    let relative = container.trim().replace('\\', "/");
    if relative.is_empty() {
        return Ok(());
    }
    let source_path = path_from_normalized_relative(source, &relative)?;
    if source_path.is_file() {
        copy_source_relative_file(source, out_dir, &relative)?;
    }
    Ok(())
}

pub(in super::super) fn direct_npc_character_bundle_source(
    project: &Path,
    manifest: &JsonValue,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
) -> Option<PathBuf> {
    if npc_blueprint_uses_authoring_model_bundle(blueprint) || blueprint.generated_icon.is_some() {
        return None;
    }
    let bundle_name = manifest
        .get("bundleName")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| npc_blueprint_bundle_name(blueprint));
    let lower_bundle_name = bundle_name.to_ascii_lowercase();
    if !(lower_bundle_name.starts_with("character_")
        && lower_bundle_name.ends_with(".resourcefile"))
    {
        return None;
    }

    let npc_root = npc_stage_root(project, blueprint);
    let mut seen = BTreeSet::<String>::new();
    context
        .source_paths
        .iter()
        .cloned()
        .chain(npc_blueprint_source_paths(project, blueprint))
        .filter(|path| {
            let key = path.to_string_lossy().to_ascii_lowercase();
            seen.insert(key)
        })
        .find(|path| {
            path.is_file()
                && is_unity_bundle_path(path)
                && !path.starts_with(&npc_root)
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case(&bundle_name))
        })
}

pub(in super::super) fn read_table_data_object_body(
    project: &Path,
    bundle: &Path,
    asset_name: &str,
    path_id: i64,
) -> Result<fusionforge::UnityValue, String> {
    let extract_dir = extract_bundle_cached(project, bundle)?;
    let asset_path = extracted_asset_path(&extract_dir, asset_name)?;
    let asset = fusionforge::Asset::from_path(&asset_path)?;
    if let Some(info) = asset.objects.get(&path_id) {
        return asset.read_object(0, info);
    }
    if let Some(info) = asset.objects.get(&7) {
        let body = asset.read_object(0, info)?;
        if body.get("m_pNpcTable").is_some() || body.get("m_pAnimationTable").is_some() {
            return Ok(body);
        }
    }
    for info in asset.objects.values() {
        if asset.object_type_name(info) != "MonoBehaviour" {
            continue;
        }
        let body = asset.read_object(0, info)?;
        if body.get("m_pNpcTable").is_some() || body.get("m_pAnimationTable").is_some() {
            return Ok(body);
        }
    }
    Err(format!("{}#{} was not found", asset.name, path_id))
}

pub(in super::super) fn npc_standalone_bundle_inputs(
    project: &Path,
    manifest_path: &Path,
    manifest: &JsonValue,
    blueprint: &NpcBlueprint,
) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    paths.push(manifest_path.to_path_buf());
    if let Some(authoring_model) = authoring_model_path_from_manifest(project, blueprint, manifest)
    {
        paths.push(authoring_model);
    }
    let context = npc_import_manifest_context(project, manifest);
    for path in context
        .source_paths
        .iter()
        .cloned()
        .chain(npc_blueprint_source_paths(project, blueprint))
    {
        if path.exists() {
            paths.push(path);
        }
    }
    let source_dir = npc_stage_root(project, blueprint).join("source");
    if source_dir.exists() {
        paths.push(source_dir);
    }

    let mut files = Vec::new();
    let mut seen = BTreeSet::new();
    for path in paths {
        let mut expanded = Vec::new();
        collect_existing_files_recursive(&path, &mut expanded)?;
        for file in expanded {
            let key = file.to_string_lossy().to_ascii_lowercase();
            if seen.insert(key) {
                files.push(file);
            }
        }
    }
    Ok(files)
}

pub(in super::super) fn npc_standalone_bundle_has_resolved_mob_root(target: &Path) -> Result<bool, String> {
    let temp = native_build_temp_dir("npc_freshness_probe")?;
    extract_bundle_native_to_dir(target, temp.path())?;
    let assets = extracted_files_in_dir(temp.path())
        .into_iter()
        .filter_map(|file| fusionforge::Asset::from_path(Path::new(&file.path)).ok())
        .collect::<Vec<_>>();
    if assets.is_empty() {
        return Ok(false);
    }
    let env = fusionforge::UnityEnvironment::from_assets(assets);
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let Ok(body) = asset.read_object(asset_index, info) else {
                continue;
            };
            for entry in fusionforge::value_array(body.get("m_Container")) {
                let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                    continue;
                };
                if !normalized_asset_path(path).starts_with("mob/") {
                    continue;
                }
                let Some(pointer) = metadata
                    .get("asset")
                    .and_then(fusionforge::UnityValue::as_pointer)
                else {
                    continue;
                };
                if npc_container_pointer_quality(&env, asset_index, path, pointer) > 0 {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

pub(in super::super) fn npc_standalone_bundle_is_fresh(
    project: &Path,
    manifest_path: &Path,
    manifest: &JsonValue,
    blueprint: &NpcBlueprint,
    target: &Path,
) -> Result<bool, String> {
    if !target.is_file() || manifest.get("rawAssets").is_none() {
        return Ok(false);
    }
    let target_modified = metadata_modified_ms(
        &target
            .metadata()
            .map_err(|err| format!("{}: {err}", target.display()))?,
    )?;
    let inputs = npc_standalone_bundle_inputs(project, manifest_path, manifest, blueprint)?;
    let Some(input_modified) = newest_modified_ms(&inputs)? else {
        return Ok(false);
    };
    // The build writes the readable NPC manifest immediately after the bundle;
    // tolerate that small timestamp skew while still detecting later edits.
    if target_modified.saturating_add(5_000) < input_modified {
        return Ok(false);
    }
    // Timestamp freshness is insufficient: older packer versions could emit a
    // tiny AssetBundle wrapper whose mob/ entry pointed at an object that was
    // never copied. Treat that output as stale so it is rebuilt automatically.
    Ok(npc_standalone_bundle_has_resolved_mob_root(target).unwrap_or(false))
}

pub(in super::super) fn standalone_npc_bundle_names(out_dir: &Path) -> Result<Vec<String>, String> {
    if !out_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = fs::read_dir(out_dir)
        .map_err(|err| format!("{}: {err}", out_dir.display()))?
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            (lower.starts_with("character_") || lower.starts_with("npc_pack_"))
                && lower.ends_with(".resourcefile")
        })
        .collect::<Vec<_>>();
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    Ok(names)
}

pub(in super::super) fn extract_bundle_native_to_dir(bundle: &Path, dir: &Path) -> Result<(), String> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let (_header, bundle_data) =
        ffbuildtool::bundle::AssetBundle::from_file(&bundle.to_string_lossy())?;
    bundle_data.extract_files(&dir.to_string_lossy())
}

pub(in super::super) fn npc_container_paths_from_source_bundles(
    project: &Path,
    source_paths: &[PathBuf],
) -> Result<BTreeSet<String>, String> {
    let mut container_paths = BTreeSet::new();
    for source_path in source_paths {
        if source_path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(is_npc_shared_dependency_bundle)
        {
            continue;
        }
        if !(source_path.is_file() || source_path.is_dir()) {
            continue;
        }
        if source_path.is_file() && !is_unity_bundle_path(source_path) {
            continue;
        }
        let assets = npc_source_assets_from_path(project, source_path)?;
        let env = fusionforge::UnityEnvironment::from_assets(assets);
        for (asset_index, asset) in env.assets.iter().enumerate() {
            for info in asset.objects.values() {
                if asset.object_type_name(info) != "AssetBundle" {
                    continue;
                }
                let body = asset.read_object(asset_index, info)?;
                for entry in fusionforge::value_array(body.get("m_Container")) {
                    let Some((path, _metadata)) = fusionforge::pair_name_value(entry) else {
                        continue;
                    };
                    container_paths.insert(normalized_asset_path(path));
                }
            }
        }
    }
    Ok(container_paths)
}

pub(in super::super) fn pack_bundle_native_from_dir(dir: &Path, bundle: &Path) -> Result<(), String> {
    let packed = ffbuildtool::bundle::AssetBundle::from_directory(&dir.to_string_lossy())?;
    packed.to_file(&bundle.to_string_lossy(), 4, None)
}

pub(in super::super) fn pack_npc_source_assets_fallback_bundle(
    project: &Path,
    source_bundles: &[PathBuf],
    output_path_overrides: &BTreeMap<String, String>,
    hints: &NpcAssetHints,
    bundle_path: &Path,
) -> Result<(), String> {
    let temp_dir = native_build_temp_dir("npc_bundle_fallback")?;
    let temp_path = temp_dir.path();
    for source_path in source_bundles {
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
            let target = temp_path.join(name);
            if target.exists() {
                continue;
            }
            fs::copy(&path, &target)
                .map_err(|err| format!("{} -> {}: {err}", path.display(), target.display()))?;
        }
    }
    if !output_path_overrides.is_empty() {
        for file in extracted_files_in_dir(temp_path) {
            let asset_path = PathBuf::from(&file.path);
            let name = asset_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if name.ends_with(".json") {
                continue;
            }
            let Ok(asset) = fusionforge::Asset::from_path(&asset_path) else {
                continue;
            };
            let mut replacements = BTreeMap::<i64, Vec<u8>>::new();
            for info in asset.objects.values() {
                if asset.object_type_name(info) != "AssetBundle" {
                    continue;
                }
                let mut value = asset.read_object(0, info)?;
                let mut renamed = 0usize;
                if let Some(container) = value
                    .get_mut("m_Container")
                    .and_then(fusionforge::UnityValue::as_array_mut)
                {
                    let mut main_asset_name = None::<String>;
                    let mut extra_entries = Vec::new();
                    let mut seen_aliases = container
                        .iter()
                        .filter_map(|entry| {
                            fusionforge::pair_name_value(entry)
                                .map(|(path, _)| normalized_asset_path(path))
                        })
                        .collect::<BTreeSet<_>>();
                    for entry in container.iter_mut() {
                        let Some((path, _)) = fusionforge::pair_name_value(entry) else {
                            continue;
                        };
                        let normalized = normalized_asset_path(path);
                        let target_path = output_path_overrides
                            .get(&normalized)
                            .cloned()
                            .unwrap_or_else(|| path.to_string());
                        if target_path != path {
                            rename_pair_key(entry, &target_path);
                            renamed += 1;
                        }
                        main_asset_name.get_or_insert_with(|| target_path.clone());
                        for alias_path in npc_container_alias_paths(&target_path, hints) {
                            let normalized_alias = normalized_asset_path(&alias_path);
                            if !seen_aliases.insert(normalized_alias) {
                                continue;
                            }
                            let mut alias_entry = entry.clone();
                            rename_pair_key(&mut alias_entry, &alias_path);
                            extra_entries.push(alias_entry);
                            renamed += 1;
                        }
                    }
                    if !extra_entries.is_empty() {
                        container.extend(extra_entries);
                    }
                    if renamed > 0 {
                        if let Some(main_asset) = value
                            .get_mut("m_MainAsset")
                            .and_then(fusionforge::UnityValue::as_object_mut)
                        {
                            if let Some(name) = main_asset_name {
                                main_asset.insert(
                                    "name".to_string(),
                                    fusionforge::UnityValue::String(name),
                                );
                            }
                        }
                    }
                }
                if renamed > 0 {
                    replacements
                        .insert(info.path_id, asset.serialize_object_value(0, info, &value)?);
                }
            }
            if !replacements.is_empty() {
                let rebuilt = asset.rebuild_with_object_data(&replacements)?;
                fs::write(&asset_path, rebuilt)
                    .map_err(|err| format!("{}: {err}", asset_path.display()))?;
            }
        }
    }
    pack_bundle_native_from_dir(temp_path, bundle_path)
}

pub(in super::super) fn read_bundle_native(input_bundle: String) -> EditorResult<fusionforge::BundleReadSummary> {
    let path = PathBuf::from(input_bundle);
    fusionforge::inspect_bundle(path).map_err(|err| format!("fusionforge read failed: {err}"))
}

pub(in super::super) fn extract_bundle_native(
    input_bundle: String,
    output_dir: Option<String>,
) -> EditorResult<fusionforge::BundleExtractSummary> {
    let path = PathBuf::from(input_bundle);
    let output_path = output_dir
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let root = crate::repository_root().to_path_buf()
                .join("work")
                .join("sessions")
                .join("fusionforge");
            fusionforge::default_extract_dir(&path, &root)
        });
    fusionforge::extract_bundle(path, output_path)
        .map_err(|err| format!("fusionforge extract failed: {err}"))
}
