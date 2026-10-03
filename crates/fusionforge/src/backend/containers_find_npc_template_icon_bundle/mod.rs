use super::super::*;

pub(in super::super) fn unity_environment_paths(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let key = path
                .to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase();
            if seen.insert(key) {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

#[derive(Debug, Clone)]
pub(in super::super) struct PreviewContainerEntry {
    pub(in super::super) asset_index: usize,
    pub(in super::super) path: String,
    pub(in super::super) metadata: fusionforge::UnityValue,
    pub(in super::super) preload_table: Rc<Vec<fusionforge::UnityValue>>,
}

pub(in super::super) fn cached_exact_object_pointers(
    env: &fusionforge::UnityEnvironment,
    key: (usize, i64),
) -> Rc<Vec<fusionforge::Pointer>> {
    let environment_identity = env as *const fusionforge::UnityEnvironment as usize;
    if let Some(cached) = LAST_EXACT_POINTER_GRAPH.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache
            .as_ref()
            .is_some_and(|(identity, _)| *identity != environment_identity)
        {
            *cache = Some((environment_identity, BTreeMap::new()));
        }
        cache.get_or_insert_with(|| (environment_identity, BTreeMap::new()));
        cache
            .as_ref()
            .and_then(|(_, pointers)| pointers.get(&key).cloned())
    }) {
        return cached;
    }

    let pointers = env
        .assets
        .get(key.0)
        .and_then(|asset| {
            asset
                .objects
                .get(&key.1)
                .and_then(|info| asset.read_object(key.0, info).ok())
        })
        .map(|body| {
            let mut pointers = Vec::new();
            collect_value_pointers(&body, &mut pointers);
            pointers
        })
        .unwrap_or_default();
    let pointers = Rc::new(pointers);
    LAST_EXACT_POINTER_GRAPH.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache
            .as_ref()
            .is_some_and(|(identity, _)| *identity != environment_identity)
        {
            *cache = Some((environment_identity, BTreeMap::new()));
        }
        cache.get_or_insert_with(|| (environment_identity, BTreeMap::new()));
        cache
            .as_mut()
            .expect("exact pointer graph cache initialized")
            .1
            .insert(key, pointers.clone());
    });
    pointers
}

pub(in super::super) fn cached_preview_container_entries(
    env: &fusionforge::UnityEnvironment,
) -> Rc<Vec<PreviewContainerEntry>> {
    let key = env
        .assets
        .iter()
        .map(|asset| (asset.name.to_lowercase(), asset.objects.len()))
        .collect::<Vec<_>>();
    let environment_identity = std::ptr::from_ref(env) as usize;
    LAST_PREVIEW_CONTAINER_INDEX.with(|cached| {
        let mut cached = cached.borrow_mut();
        if let Some((cached_identity, cached_key, entries)) = cached.as_ref() {
            if *cached_identity == environment_identity && cached_key == &key {
                return Rc::clone(entries);
            }
        }
        let mut entries = Vec::new();
        for (asset_index, asset) in env.assets.iter().enumerate() {
            for info in asset.objects.values() {
                if asset.object_type_name(info) != "AssetBundle" {
                    continue;
                }
                let Ok(body) = asset.read_object(asset_index, info) else {
                    continue;
                };
                let preload_table =
                    Rc::new(fusionforge::value_array(body.get("m_PreloadTable")).to_vec());
                for value in fusionforge::value_array(body.get("m_Container")) {
                    let Some((path, metadata)) = fusionforge::pair_name_value(value) else {
                        continue;
                    };
                    entries.push(PreviewContainerEntry {
                        asset_index,
                        path: path.to_string(),
                        metadata: metadata.clone(),
                        preload_table: Rc::clone(&preload_table),
                    });
                }
            }
        }
        entries.sort_by(|left, right| {
            normalized_asset_path(&left.path)
                .cmp(&normalized_asset_path(&right.path))
                .then_with(|| left.asset_index.cmp(&right.asset_index))
                .then_with(|| left.path.cmp(&right.path))
        });
        let entries = Rc::new(entries);
        *cached = Some((environment_identity, key, Rc::clone(&entries)));
        entries
    })
}

pub(in super::super) fn cached_unity_environment_from_extract_dirs(
    dirs: &[PathBuf],
) -> Rc<fusionforge::UnityEnvironment> {
    let paths = unity_environment_paths(dirs);
    let key = paths
        .iter()
        .map(|path| {
            let metadata = fs::metadata(path).ok();
            let bytes = metadata.as_ref().map_or(0, fs::Metadata::len);
            let modified = metadata
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |duration| duration.as_nanos());
            (
                path.to_string_lossy().replace('\\', "/").to_lowercase(),
                bytes,
                modified,
            )
        })
        .collect::<Vec<_>>();
    LAST_UNITY_ENVIRONMENT.with(|cached| {
        let mut cached = cached.borrow_mut();
        if let Some((cached_key, environment)) = cached.as_ref() {
            if cached_key == &key {
                return Rc::clone(environment);
            }
        }
        let environment = Rc::new(fusionforge::UnityEnvironment::from_paths(&paths));
        *cached = Some((key, Rc::clone(&environment)));
        environment
    })
}

pub(in super::super) fn dependency_bundle_extract_dir(project: &Path, bundle: &ClientBundleFile) -> Option<PathBuf> {
    bundle
        .cache_dir
        .as_deref()
        .map(PathBuf::from)
        .filter(|path| has_cached_payload(path))
        .or_else(|| extract_bundle_cached(project, Path::new(&bundle.path)).ok())
}

pub(in super::super) fn preview_container_dependency_refs(
    env: &fusionforge::UnityEnvironment,
    wanted: &BTreeSet<String>,
) -> BTreeSet<String> {
    if wanted.is_empty() {
        return BTreeSet::new();
    }

    let mut refs = BTreeSet::new();
    let mut selected = BTreeSet::<(usize, i64)>::new();
    let mut queue = VecDeque::<(usize, i64)>::new();

    let container_entries = cached_preview_container_entries(env);
    for entry in container_entries.iter() {
        let lower = normalized_asset_path(&entry.path);
        if !wanted
            .iter()
            .any(|expected| lower == *expected || lower.ends_with(&format!("/{expected}")))
        {
            continue;
        }
        if let Some(pointer) = entry
            .metadata
            .get("asset")
            .and_then(fusionforge::UnityValue::as_pointer)
        {
            add_pointer_dependency_root(env, pointer, &mut refs, &mut selected, &mut queue);
        }
        let (start, end) = metadata_preload_range(&entry.metadata, entry.preload_table.len());
        for preload in &entry.preload_table[start..end] {
            if let Some(pointer) = preload.as_pointer() {
                add_pointer_dependency_root(env, pointer, &mut refs, &mut selected, &mut queue);
            }
        }
    }

    while let Some((asset_index, path_id)) = queue.pop_front() {
        for pointer in cached_exact_object_pointers(env, (asset_index, path_id)).iter() {
            add_pointer_dependency_root(env, pointer, &mut refs, &mut selected, &mut queue);
        }
    }

    refs
}

pub(in super::super) fn inspect_bundle_fonts(
    bundle_path: String,
    project_dir: Option<String>,
) -> EditorResult<serde_json::Value> {
    let bundle = PathBuf::from(&bundle_path);
    let project = project_dir
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::repository_root().to_path_buf().join("work").join("fusionforge"));
    let extract_dir = extract_bundle_cached(&project, &bundle)?;
    let container = bundle
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("bundle");
    let fonts = inspect_cached_bundle_fonts(&extract_dir, container, &bundle_path);
    Ok(json!({ "bundlePath": bundle_path, "cacheDir": extract_dir, "fonts": fonts }))
}

pub(in super::super) fn unity_value_to_json(value: &fusionforge::UnityValue) -> JsonValue {
    match value {
        fusionforge::UnityValue::Bool(value) => json!(value),
        fusionforge::UnityValue::Int(value) => json!(value),
        fusionforge::UnityValue::UInt(value) => json!(value),
        fusionforge::UnityValue::Float(value) => json!(value),
        fusionforge::UnityValue::String(value) => json!(value),
        fusionforge::UnityValue::Bytes(value) => {
            json!({
                "__unityType": "bytes",
                "bytes": value.len(),
                "base64": base64::engine::general_purpose::STANDARD.encode(value),
            })
        }
        fusionforge::UnityValue::Array(values) => {
            JsonValue::Array(values.iter().map(unity_value_to_json).collect())
        }
        fusionforge::UnityValue::Object(values) => JsonValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), unity_value_to_json(value)))
                .collect(),
        ),
        fusionforge::UnityValue::Pair(left, right) => {
            json!({
                "__unityType": "pair",
                "items": [unity_value_to_json(left), unity_value_to_json(right)],
            })
        }
        fusionforge::UnityValue::Pointer(pointer) => {
            json!({
                "__unityType": "pointer",
                "fileId": pointer.file_id,
                "pathId": pointer.path_id,
            })
        }
    }
}

pub(in super::super) fn json_to_unity_value(
    value: &JsonValue,
    asset_index: usize,
) -> Result<fusionforge::UnityValue, String> {
    Ok(match value {
        JsonValue::Null => fusionforge::UnityValue::String(String::new()),
        JsonValue::Bool(value) => fusionforge::UnityValue::Bool(*value),
        JsonValue::Number(value) => {
            if let Some(value) = value.as_i64() {
                fusionforge::UnityValue::Int(value)
            } else if let Some(value) = value.as_u64() {
                fusionforge::UnityValue::UInt(value)
            } else {
                fusionforge::UnityValue::Float(value.as_f64().unwrap_or_default())
            }
        }
        JsonValue::String(value) => fusionforge::UnityValue::String(value.clone()),
        JsonValue::Array(values) => fusionforge::UnityValue::Array(
            values
                .iter()
                .map(|value| json_to_unity_value(value, asset_index))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        JsonValue::Object(values) => {
            let unity_type = values.get("__unityType").and_then(JsonValue::as_str);
            if unity_type == Some("bytes") {
                let base64 = values
                    .get("base64")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "byte patch value has no base64 field".to_string())?;
                fusionforge::UnityValue::Bytes(
                    base64::engine::general_purpose::STANDARD
                        .decode(base64)
                        .map_err(|err| err.to_string())?,
                )
            } else if unity_type == Some("pointer")
                || (values.len() <= 3
                    && values.contains_key("fileId")
                    && values.contains_key("pathId"))
            {
                fusionforge::UnityValue::Pointer(fusionforge::Pointer {
                    source_asset: asset_index,
                    file_id: values
                        .get("fileId")
                        .and_then(JsonValue::as_i64)
                        .unwrap_or_default() as i32,
                    path_id: values
                        .get("pathId")
                        .and_then(JsonValue::as_i64)
                        .unwrap_or_default(),
                })
            } else if unity_type == Some("pair") {
                let items = values
                    .get("items")
                    .and_then(JsonValue::as_array)
                    .ok_or_else(|| "pair patch value has no items array".to_string())?;
                if items.len() < 2 {
                    return Err("pair patch value needs two items".to_string());
                }
                fusionforge::UnityValue::Pair(
                    Box::new(json_to_unity_value(&items[0], asset_index)?),
                    Box::new(json_to_unity_value(&items[1], asset_index)?),
                )
            } else {
                fusionforge::UnityValue::Object(
                    values
                        .iter()
                        .filter(|(key, _)| key.as_str() != "__unityType")
                        .map(|(key, value)| {
                            Ok((key.clone(), json_to_unity_value(value, asset_index)?))
                        })
                        .collect::<Result<BTreeMap<_, _>, String>>()?,
                )
            }
        }
    })
}

pub(in super::super) fn is_unity_metadata_field(key: &str) -> bool {
    matches!(
        key,
        "m_ObjectHideFlags"
            | "m_ExtensionPtr"
            | "m_GameObject"
            | "m_Enabled"
            | "m_EditorHideFlags"
            | "m_Script"
            | "m_Name"
    )
}

pub(in super::super) fn container_name_from_bundle(bundle: &Path) -> String {
    bundle
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("TableData.resourceFile")
        .to_string()
}

pub(in super::super) fn unity_string_field(row: Option<&fusionforge::UnityValue>, key: &str) -> Option<String> {
    row.and_then(|value| value.get(key))
        .and_then(fusionforge::UnityValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(in super::super) fn set_optional_unity_string(
    row: &mut fusionforge::UnityValue,
    field: &str,
    value: Option<String>,
) -> Result<bool, String> {
    let Some(value) = value else {
        return Ok(false);
    };
    let object = row
        .as_object_mut()
        .ok_or_else(|| format!("NPC row is not an object while writing {field}"))?;
    if value.is_empty() && !object.contains_key(field) {
        return Ok(false);
    }
    object.insert(field.to_string(), fusionforge::UnityValue::String(value));
    Ok(true)
}

pub(in super::super) fn unity_usize_field(row: &fusionforge::UnityValue, key: &str) -> Option<usize> {
    row.get(key)
        .and_then(fusionforge::UnityValue::as_i64)
        .and_then(|value| usize::try_from(value).ok())
}

pub(in super::super) fn patch_unity_object_fields(
    row: &mut fusionforge::UnityValue,
    fields: Option<&serde_json::Map<String, JsonValue>>,
) -> Result<bool, String> {
    let Some(fields) = fields else {
        return Ok(false);
    };
    let object = row
        .as_object_mut()
        .ok_or_else(|| "NPC table row is not an object".to_string())?;
    let mut changed = false;
    for (field, value) in fields {
        object.insert(field.clone(), json_to_unity_value(value, 0)?);
        changed = true;
    }
    Ok(changed)
}

pub(in super::super) fn table_data_patch_body_for_object(
    project: &Path,
    bundle_path: &str,
    asset: &str,
    path_id: i64,
) -> Result<Option<fusionforge::UnityValue>, String> {
    let Some(path) = table_data_patch_path_for_object(project, bundle_path, asset, path_id)? else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    table_data_patch_value(&path, 0).map(Some)
}

pub(in super::super) fn set_unity_usize_field(row: &mut fusionforge::UnityValue, key: &str, value: usize) {
    if let Some(object) = row.as_object_mut() {
        object.insert(key.to_string(), fusionforge::UnityValue::Int(value as i64));
    }
}

pub(in super::super) fn set_unity_i64_field(row: &mut fusionforge::UnityValue, key: &str, value: i64) {
    if let Some(object) = row.as_object_mut() {
        object.insert(key.to_string(), fusionforge::UnityValue::Int(value));
    }
}

pub(in super::super) fn json_to_unity_value_like(
    value: &JsonValue,
    existing: &fusionforge::UnityValue,
) -> Result<fusionforge::UnityValue, String> {
    Ok(match existing {
        fusionforge::UnityValue::Float(_) => {
            fusionforge::UnityValue::Float(value.as_f64().unwrap_or_default())
        }
        fusionforge::UnityValue::UInt(_) => {
            fusionforge::UnityValue::UInt(value.as_i64().unwrap_or_default().max(0) as u64)
        }
        fusionforge::UnityValue::Int(_) => {
            fusionforge::UnityValue::Int(value.as_i64().unwrap_or_default())
        }
        fusionforge::UnityValue::Bool(_) => fusionforge::UnityValue::Bool(
            value
                .as_bool()
                .unwrap_or_else(|| value.as_i64().unwrap_or_default() != 0),
        ),
        fusionforge::UnityValue::String(_) => {
            fusionforge::UnityValue::String(value.as_str().unwrap_or_default().to_string())
        }
        _ => json_to_unity_value(value, 0)?,
    })
}

pub(in super::super) fn set_existing_unity_i64_field(row: &mut fusionforge::UnityValue, key: &str, value: i64) -> bool {
    let Some(object) = row.as_object_mut() else {
        return false;
    };
    if !object.contains_key(key) {
        return false;
    }
    object.insert(key.to_string(), fusionforge::UnityValue::Int(value));
    true
}

pub(in super::super) fn set_existing_unity_string_field(
    row: &mut fusionforge::UnityValue,
    key: &str,
    value: &str,
) -> bool {
    let Some(object) = row.as_object_mut() else {
        return false;
    };
    if !object.contains_key(key) {
        return false;
    }
    object.insert(
        key.to_string(),
        fusionforge::UnityValue::String(value.to_string()),
    );
    true
}

pub(in super::super) fn patch_existing_unity_object_fields(
    row: &mut fusionforge::UnityValue,
    fields: &BTreeMap<String, JsonValue>,
) -> Result<bool, String> {
    let object = row
        .as_object_mut()
        .ok_or_else(|| "NPC table row is not an object".to_string())?;
    let mut changed = false;
    for (field, value) in fields {
        if value.is_null() || blueprint_profile_protected_field(field) {
            continue;
        }
        let Some(existing) = object.get(field).cloned() else {
            continue;
        };
        object.insert(field.clone(), json_to_unity_value_like(value, &existing)?);
        changed = true;
    }
    Ok(changed)
}

pub(in super::super) fn npc_container_alias_paths(path: &str, hints: &NpcAssetHints) -> Vec<String> {
    let _ = path;
    let _ = hints;
    Vec::new()
}

pub(in super::super) fn staged_icon_container_should_retarget(
    path: &str,
    old_paths: &BTreeSet<String>,
    target_path: &str,
) -> bool {
    let path = normalized_asset_path(path);
    if path == target_path {
        return false;
    }
    if old_paths.contains(&path) {
        return true;
    }
    let Some(target_icon) = parse_npc_icon_asset_path(target_path) else {
        return false;
    };
    parse_npc_icon_asset_path(&path).is_some_and(|icon| icon.prefix == target_icon.prefix)
}

pub(in super::super) fn first_npc_character_source_bundle(
    project: &Path,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
) -> Option<PathBuf> {
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
                    .is_some_and(|name| {
                        let lower = name.to_ascii_lowercase();
                        lower.starts_with("character_") && lower.ends_with(".resourcefile")
                    })
        })
}

pub(in super::super) fn npc_blueprint_bundle_name(blueprint: &NpcBlueprint) -> String {
    let base = blueprint.name.trim();
    format!("Character_{}.resourceFile", safe_segment(base))
}

pub(in super::super) fn copy_serialized_assets_into_dir(source_dir: &Path, staged_dir: &Path) -> Result<(), String> {
    let mut copied = 0usize;
    for file in extracted_files_in_dir(source_dir) {
        let source_path = PathBuf::from(&file.path);
        let Some(name) = source_path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.ends_with(".json") {
            continue;
        }
        let target = staged_dir.join(name);
        fs::copy(&source_path, &target)
            .map_err(|err| format!("{} -> {}: {err}", source_path.display(), target.display()))?;
        copied += 1;
    }
    if copied == 0 {
        return Err(format!(
            "{} had no serialized asset files to copy",
            source_dir.display()
        ));
    }
    Ok(())
}

pub(in super::super) fn find_npc_template_icon_bundle(
    project: &Path,
    template_icon_paths: &BTreeSet<String>,
) -> Result<PathBuf, String> {
    let index_path = project.join("cache").join("bundle-index.json");
    if !index_path.is_file() {
        return Err(format!(
            "{} is missing; load/index the target client before converting an authoring model",
            index_path.display()
        ));
    }
    let index = serde_json::from_str::<ClientFileIndex>(
        &fs::read_to_string(&index_path)
            .map_err(|err| format!("{}: {err}", index_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", index_path.display()))?;
    let mut best = None::<(i32, String)>;
    for bundle in &index.bundles {
        let paths = bundle
            .assets
            .iter()
            .flat_map(|asset| asset.container_paths.iter())
            .map(|path| normalized_asset_path(path))
            .collect::<BTreeSet<_>>();
        let exact_matches = paths
            .iter()
            .filter(|path| {
                template_icon_paths
                    .iter()
                    .any(|expected| path == &expected || path.ends_with(&format!("/{expected}")))
            })
            .count() as i32;
        if exact_matches == 0 {
            continue;
        }
        let has_texture = bundle.assets.iter().any(|asset| {
            asset
                .type_counts
                .get("Texture2D")
                .copied()
                .unwrap_or_default()
                > 0
        });
        let has_mesh_filter = bundle.assets.iter().any(|asset| {
            asset
                .type_counts
                .get("MeshFilter")
                .copied()
                .unwrap_or_default()
                > 0
        });
        let has_game_object = bundle.assets.iter().any(|asset| {
            asset
                .type_counts
                .get("GameObject")
                .copied()
                .unwrap_or_default()
                > 0
        });
        let bundle_name = Path::new(&bundle.path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let score = exact_matches * 1000
            + if has_texture { 100 } else { 0 }
            + if has_mesh_filter { 40 } else { 0 }
            + if has_game_object { 20 } else { 0 }
            + if bundle_name == "icons.resourcefile" {
                200
            } else {
                0
            };
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, bundle.path.clone()));
        }
    }
    best.map(|(_, path)| PathBuf::from(path)).ok_or_else(|| {
        format!(
            "Could not find a template NPC icon bundle for icon path(s): {}",
            template_icon_paths
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

pub(in super::super) fn rename_assetbundle_container_paths(
    value: &mut fusionforge::UnityValue,
    wanted: &BTreeSet<String>,
    target_model_path: &str,
) -> usize {
    let mut renamed = 0usize;
    let mut main_asset_path = None::<String>;
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let Some(container) = object
        .get_mut("m_Container")
        .and_then(fusionforge::UnityValue::as_array_mut)
    else {
        return 0;
    };
    for entry in container {
        let Some(path) = fusionforge::pair_name_value(entry).map(|(path, _)| path.to_string())
        else {
            continue;
        };
        if !model_container_path_matches(&path, wanted) {
            continue;
        }
        let target_path = target_model_path_for_template_path(&path, target_model_path);
        rename_pair_key(entry, &target_path);
        main_asset_path.get_or_insert(target_path);
        renamed += 1;
    }
    if renamed > 0 {
        if let Some(main_asset) = object
            .get_mut("m_MainAsset")
            .and_then(fusionforge::UnityValue::as_object_mut)
        {
            main_asset.insert(
                "name".to_string(),
                fusionforge::UnityValue::String(
                    main_asset_path.unwrap_or_else(|| target_model_path.replace('\\', "/")),
                ),
            );
        }
    }
    renamed
}
