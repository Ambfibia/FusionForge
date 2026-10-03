use super::super::*;

pub(in super::super) fn external_resource_import_specs(
    project: &Path,
    patch_config: &JsonValue,
) -> Result<Vec<ExternalResourceImportSpec>, String> {
    let Some(entries) = patch_config
        .get("ExternalResourceImports")
        .and_then(JsonValue::as_array)
    else {
        return Ok(Vec::new());
    };
    let allowed_sections = BTreeSet::from([
        "m_CharacterCreation",
        "m_CharacterSelection",
        "m_Tutorial",
        "m_FreeZone",
        "m_PaidZone",
    ]);
    let mut result = Vec::new();
    let mut names = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let source_value = external_import_value(entry, &["Source", "source"])
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("ExternalResourceImports[{index}] has no Source"))?;
        let source = resolve_project_path(project, source_value);
        if !source.is_file() {
            return Err(format!(
                "ExternalResourceImports[{index}] source was not found: {}",
                source.display()
            ));
        }
        let bundle_name = external_import_value(entry, &["BundleName", "bundleName"])
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                source
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(ToOwned::to_owned)
            })
            .ok_or_else(|| format!("ExternalResourceImports[{index}] has no BundleName"))?;
        let lower_name = bundle_name.to_ascii_lowercase();
        if Path::new(&bundle_name)
            .file_name()
            .and_then(|value| value.to_str())
            != Some(bundle_name.as_str())
            || !(lower_name.ends_with(".resourcefile") || lower_name.ends_with(".unity3d"))
        {
            return Err(format!(
                "ExternalResourceImports[{index}] BundleName must be a plain .resourceFile/.unity3d filename: {bundle_name}"
            ));
        }
        if !names.insert(lower_name) {
            return Err(format!(
                "ExternalResourceImports contains duplicate BundleName {bundle_name}"
            ));
        }
        let routes = external_import_value(entry, &["Routes", "routes"])
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter_map(JsonValue::as_str)
            .map(normalized_asset_path)
            .filter(|route| !route.is_empty())
            .collect::<BTreeSet<_>>();
        if routes.is_empty() {
            return Err(format!(
                "ExternalResourceImports[{index}] must select at least one exact route"
            ));
        }
        let mut sections = external_import_value(entry, &["Sections", "sections"])
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter_map(JsonValue::as_str)
            .map(str::trim)
            .filter(|section| !section.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        sections.sort();
        sections.dedup();
        if sections.is_empty()
            || sections
                .iter()
                .any(|section| !allowed_sections.contains(section.as_str()))
        {
            return Err(format!(
                "ExternalResourceImports[{index}] Sections must contain only base runtime phases"
            ));
        }
        let project_bundle = project.join("external-resources").join(&bundle_name);
        result.push(ExternalResourceImportSpec {
            source,
            bundle_name,
            routes,
            sections,
            project_bundle,
        });
    }
    Ok(result)
}

pub(in super::super) fn external_resource_import_fingerprint(
    spec: &ExternalResourceImportSpec,
) -> Result<String, String> {
    let canonical = spec
        .source
        .canonicalize()
        .map_err(|err| format!("{}: {err}", spec.source.display()))?;
    let (source_bytes, source_sha256) = sha256_file_content(&canonical)?;
    let document = json!({
        "format": "ffclient.external-resource-import.fingerprint.v2",
        "pipelineRevision": 3,
        "source": canonical.to_string_lossy(),
        "sourceBytes": source_bytes,
        "sourceSha256": source_sha256,
        "bundleName": spec.bundle_name,
        "routes": spec.routes,
    });
    let serialized = serde_json::to_string(&document).map_err(|err| err.to_string())?;
    Ok(stable_hash(&serialized))
}

pub(in super::super) fn select_external_import_pointer(
    env: &fusionforge::UnityEnvironment,
    pointer: &fusionforge::Pointer,
    selected: &mut BTreeSet<(usize, i64)>,
    queue: &mut VecDeque<(usize, i64)>,
) -> Result<(usize, i64), String> {
    if pointer.is_null() {
        return Err("selected route has a null pointer".to_string());
    }
    let key = env.resolve_pointer(pointer)?;
    if selected.insert((key.asset, key.path_id)) {
        queue.push_back((key.asset, key.path_id));
    }
    Ok((key.asset, key.path_id))
}

pub(in super::super) fn materialize_external_resource_import(spec: &ExternalResourceImportSpec) -> Result<bool, String> {
    let fingerprint = external_resource_import_fingerprint(spec)?;
    let metadata_path = external_resource_import_metadata_path(spec);
    let cached_fingerprint = fs::read_to_string(&metadata_path)
        .ok()
        .and_then(|text| serde_json::from_str::<JsonValue>(&text).ok())
        .and_then(|value| {
            value
                .get("fingerprint")
                .and_then(JsonValue::as_str)
                .map(str::to_string)
        });
    if spec.project_bundle.is_file()
        && fs::metadata(&spec.project_bundle)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false)
        && cached_fingerprint.as_deref() == Some(fingerprint.as_str())
    {
        return Ok(false);
    }

    let source_temp = native_build_temp_dir("external_resource_source")?;
    extract_bundle_native_to_dir(&spec.source, source_temp.path())?;
    let mut source_paths = extracted_files_in_dir(source_temp.path())
        .into_iter()
        .map(|file| PathBuf::from(file.path))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    source_paths.sort();
    let assets = source_paths
        .iter()
        .filter_map(|path| fusionforge::Asset::from_path(path).ok())
        .collect::<Vec<_>>();
    if assets.is_empty() {
        return Err(format!(
            "External resource source contains no readable serialized assets: {}",
            spec.source.display()
        ));
    }
    let env = fusionforge::UnityEnvironment::from_assets(assets);
    let mut selected = BTreeSet::<(usize, i64)>::new();
    let mut queue = VecDeque::<(usize, i64)>::new();
    let mut replacements = BTreeMap::<(usize, i64), fusionforge::UnityValue>::new();
    let mut matched = BTreeSet::<String>::new();

    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let body = asset.read_object(asset_index, info)?;
            let old_preloads = fusionforge::value_array(body.get("m_PreloadTable")).to_vec();
            let mut new_preloads = Vec::<fusionforge::UnityValue>::new();
            let mut new_container = Vec::<fusionforge::UnityValue>::new();
            let mut main_asset = None::<(String, fusionforge::UnityValue)>;
            for entry in fusionforge::value_array(body.get("m_Container")) {
                let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                    continue;
                };
                let route = normalized_asset_path(path);
                if !spec.routes.contains(&route) {
                    continue;
                }
                if !matched.insert(route.clone()) {
                    return Err(format!(
                        "External resource route is ambiguous in {}: {route}",
                        spec.source.display()
                    ));
                }
                let pointer = metadata
                    .get("asset")
                    .and_then(fusionforge::UnityValue::as_pointer)
                    .ok_or_else(|| {
                        format!("External resource route {route} has no asset pointer")
                    })?;
                let root_key =
                    select_external_import_pointer(&env, pointer, &mut selected, &mut queue)
                        .map_err(|err| format!("External resource route {route}: {err}"))?;
                if root_key.0 != asset_index {
                    return Err(format!(
                        "External resource route {route} crosses serialized assets; import it from a self-contained source bundle"
                    ));
                }
                let output_pointer = unity_local_pointer(asset_index, root_key.1);

                let preload_start = new_preloads.len();
                let (old_start, old_end) = metadata_preload_range(metadata, old_preloads.len());
                for preload in &old_preloads[old_start..old_end] {
                    let Some(preload_pointer) = preload.as_pointer() else {
                        continue;
                    };
                    if let Ok(preload_key) = select_external_import_pointer(
                        &env,
                        preload_pointer,
                        &mut selected,
                        &mut queue,
                    ) {
                        if preload_key.0 == asset_index {
                            new_preloads.push(unity_local_pointer(asset_index, preload_key.1));
                        }
                    }
                }
                if new_preloads.len() == preload_start {
                    new_preloads.push(output_pointer.clone());
                }
                let mut output_entry = entry.clone();
                if let Some(output_metadata) = pair_value_mut(&mut output_entry) {
                    if let Some(fields) = output_metadata.as_object_mut() {
                        fields.insert("asset".to_string(), output_pointer.clone());
                    }
                    set_unity_object_i64(output_metadata, "preloadIndex", preload_start as i64);
                    set_unity_object_i64(
                        output_metadata,
                        "preloadSize",
                        new_preloads.len().saturating_sub(preload_start) as i64,
                    );
                }
                if main_asset.is_none() {
                    main_asset = Some((path.to_string(), output_pointer));
                }
                new_container.push(output_entry);
            }
            if new_container.is_empty() {
                continue;
            }
            selected.insert((asset_index, info.path_id));
            let mut filtered = body.clone();
            if let Some(fields) = filtered.as_object_mut() {
                fields.insert(
                    "m_Container".to_string(),
                    fusionforge::UnityValue::Array(new_container),
                );
                fields.insert(
                    "m_PreloadTable".to_string(),
                    fusionforge::UnityValue::Array(new_preloads),
                );
                if let Some((path, pointer)) = main_asset {
                    if let Some(main) = fields
                        .get_mut("m_MainAsset")
                        .and_then(fusionforge::UnityValue::as_object_mut)
                    {
                        main.insert("name".to_string(), fusionforge::UnityValue::String(path));
                        main.insert("asset".to_string(), pointer);
                    }
                }
            }
            replacements.insert((asset_index, info.path_id), filtered);
        }
    }
    let missing = spec
        .routes
        .difference(&matched)
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "External resource source {} is missing selected route(s): {}",
            spec.source.display(),
            missing.join(", ")
        ));
    }

    while let Some((asset_index, path_id)) = queue.pop_front() {
        let Some(asset) = env.assets.get(asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(&path_id) else {
            continue;
        };
        if asset.object_type_name(info) == "AssetBundle" {
            continue;
        }
        let body = asset.read_object(asset_index, info)?;
        let mut pointers = Vec::new();
        collect_value_pointers(&body, &mut pointers);
        for pointer in pointers {
            if pointer.is_null() {
                continue;
            }
            if let Ok(key) = env.resolve_pointer(&pointer) {
                if selected.insert((key.asset, key.path_id)) {
                    queue.push_back((key.asset, key.path_id));
                }
            }
        }
    }

    let output_temp = native_build_temp_dir("external_resource_output")?;
    let mut written = 0usize;
    for (asset_index, asset) in env.assets.iter().enumerate() {
        let keep = selected
            .iter()
            .filter_map(|(selected_asset, path_id)| {
                (*selected_asset == asset_index).then_some(*path_id)
            })
            .collect::<BTreeSet<_>>();
        if keep.is_empty() {
            continue;
        }
        let mut data_replacements = BTreeMap::<i64, Vec<u8>>::new();
        for ((replacement_asset, path_id), value) in &replacements {
            if *replacement_asset != asset_index {
                continue;
            }
            let info = asset
                .objects
                .get(path_id)
                .ok_or_else(|| format!("{}#{} disappeared while importing", asset.name, path_id))?;
            data_replacements.insert(
                *path_id,
                asset.serialize_object_value(asset_index, info, value)?,
            );
        }
        let bytes = asset.rebuild_with_object_data_filtered(Some(&keep), &data_replacements)?;
        let target = output_temp.path().join(&asset.name);
        fs::write(&target, bytes).map_err(|err| format!("{}: {err}", target.display()))?;
        written += 1;
    }
    if written == 0 {
        return Err("External resource import selected no serialized asset files".to_string());
    }
    if let Some(parent) = spec.project_bundle.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    pack_bundle_native_from_dir(output_temp.path(), &spec.project_bundle)?;
    let metadata = json!({
        "format": "ffclient.external-resource-import.v1",
        "fingerprint": fingerprint,
        "source": spec.source.to_string_lossy(),
        "bundleName": spec.bundle_name,
        "routes": spec.routes,
        "sections": spec.sections,
    });
    fs::write(
        &metadata_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&metadata).map_err(|err| err.to_string())?
        ),
    )
    .map_err(|err| format!("{}: {err}", metadata_path.display()))?;
    Ok(true)
}

pub(in super::super) fn copy_external_resource_imports_to_build(
    project: &Path,
    patch_config: &JsonValue,
    out_dir: &Path,
) -> Result<Vec<ExternalResourceImportSpec>, String> {
    let specs = external_resource_import_specs(project, patch_config)?;
    for spec in &specs {
        let rebuilt = materialize_external_resource_import(spec)?;
        if rebuilt {
            eprintln!(
                "[ffclient:external] materialized {} from {}",
                spec.bundle_name,
                spec.source.display()
            );
        } else {
            eprintln!("[ffclient:external] reusing {}", spec.bundle_name);
        }
        let target = out_dir.join(&spec.bundle_name);
        fs::copy(&spec.project_bundle, &target).map_err(|err| {
            format!(
                "{} -> {}: {err}",
                spec.project_bundle.display(),
                target.display()
            )
        })?;
    }
    Ok(specs)
}
