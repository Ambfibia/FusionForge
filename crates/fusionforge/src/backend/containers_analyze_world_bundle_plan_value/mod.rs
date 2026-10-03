use super::super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct SceneObject {
    pub(in super::super) id: String,
    pub(in super::super) name: String,
    pub(in super::super) source: ModelSource,
    pub(in super::super) position: Vec3,
    pub(in super::super) rotation: Vec3,
    pub(in super::super) scale: Vec3,
    pub(in super::super) collider_ids: Vec<String>,
    pub(in super::super) water_volume_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ClientBundleFile {
    pub(in super::super) path: String,
    pub(in super::super) name: String,
    pub(in super::super) extension: String,
    pub(in super::super) size: u64,
    pub(in super::super) modified_ms: Option<u128>,
    pub(in super::super) cache_dir: Option<String>,
    pub(in super::super) extracted_files: Vec<ClientExtractedFile>,
    pub(in super::super) assets: Vec<ClientAssetSummary>,
    pub(in super::super) errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct TableDataObjectSummary {
    pub(in super::super) asset: String,
    pub(in super::super) path_id: i64,
    pub(in super::super) name: String,
    pub(in super::super) object_type: String,
    pub(in super::super) section_count: usize,
    pub(in super::super) row_count: usize,
    pub(in super::super) string_count: usize,
    pub(in super::super) number_count: usize,
    pub(in super::super) sections: Vec<TableDataSectionSummary>,
    pub(in super::super) errors: Vec<String>,
}

pub(in super::super) fn project_source_table_data_bundle(project: &Path) -> Option<PathBuf> {
    let source_dir = project_source_dir_from_patch_config(project)?;
    let bundle = source_dir.join("TableData.resourceFile");
    if bundle.is_file() {
        Some(bundle)
    } else {
        None
    }
}

pub(in super::super) fn extract_bundle_cached(project_dir: &Path, bundle: &Path) -> Result<PathBuf, String> {
    let cache_root = project_dir.join("cache").join("extracted-bundles");
    let out_dir = cached_extract_dir(project_dir, bundle);
    let source_meta = source_cache_metadata(bundle)?;
    if out_dir.exists()
        && has_cached_payload(&out_dir)
        && read_cache_metadata(&out_dir).is_some_and(|cached| {
            cached.source_path == source_meta.source_path
                && cached.source_size == source_meta.source_size
                && cached.source_modified_ms == source_meta.source_modified_ms
        })
    {
        return Ok(out_dir);
    }
    if out_dir.exists() {
        if !out_dir.starts_with(&cache_root) {
            return Err(format!(
                "refusing to clear cache outside {}",
                cache_root.display()
            ));
        }
        make_cache_tree_writable(&out_dir)?;
        fs::remove_dir_all(&out_dir).map_err(|err| err.to_string())?;
    }
    fs::create_dir_all(&out_dir).map_err(|err| err.to_string())?;
    fusionforge::extract_bundle(bundle.to_path_buf(), out_dir.clone())
        .map_err(|err| format!("extract failed: {err}"))?;
    let metadata = serde_json::to_string_pretty(&source_meta).map_err(|err| err.to_string())?;
    fs::write(out_dir.join("cache-meta.json"), format!("{metadata}\n"))
        .map_err(|err| err.to_string())?;
    Ok(out_dir)
}

pub(in super::super) fn inspect_cached_bundle_assets(cache_dir: &Path) -> Vec<ClientAssetSummary> {
    let env = fusionforge::UnityEnvironment::from_dir(cache_dir);
    env.assets
        .iter()
        .enumerate()
        .map(|(asset_index, asset)| {
            let mut type_counts = std::collections::BTreeMap::<String, usize>::new();
            let mut container_paths = BTreeSet::new();
            for info in asset.objects.values() {
                let object_type = asset.object_type_name(info);
                *type_counts.entry(object_type.clone()).or_default() += 1;
                if object_type == "AssetBundle" {
                    if let Ok(body) = asset.read_object(asset_index, info) {
                        for (path, _) in fusionforge::value_array(body.get("m_Container"))
                            .iter()
                            .filter_map(fusionforge::pair_name_value)
                        {
                            container_paths.insert(path.to_string());
                        }
                    }
                }
            }
            ClientAssetSummary {
                name: asset.name.clone(),
                object_count: asset.objects.len(),
                type_counts,
                container_paths: container_paths.into_iter().collect(),
            }
        })
        .collect()
}

pub(in super::super) fn inspect_cached_bundle_fonts(
    cache_dir: &Path,
    container: &str,
    bundle_path: &str,
) -> Vec<ClientFontRecord> {
    use fusionforge::{object_name, UnityEnvironment, UnityValue};

    let env = UnityEnvironment::from_dir(cache_dir);
    let russian_codes = (0x0410_i64..=0x044f_i64).chain([0x0401, 0x0451]);
    let mut fonts = Vec::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for (path_id, info) in &asset.objects {
            if asset.object_type_name(info) != "Font" {
                continue;
            }
            let body = match asset.read_object(asset_index, info) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let rects = body
                .get("m_CharacterRects")
                .and_then(UnityValue::as_array)
                .unwrap_or(&[]);
            let codes = rects
                .iter()
                .filter_map(|rect| rect.get("index").and_then(UnityValue::as_i64))
                .collect::<std::collections::HashSet<_>>();
            let has_russian = russian_codes.clone().all(|code| codes.contains(&code));
            let texture = body.get("m_Texture").and_then(UnityValue::as_pointer);
            let name = object_name(&body);
            fonts.push(ClientFontRecord {
                asset: asset.name.clone(),
                container: container.to_string(),
                bundle_path: bundle_path.to_string(),
                cache_dir: cache_dir.to_string_lossy().to_string(),
                path_id: *path_id,
                family: normalize_font_family(&name),
                name,
                line_spacing: body.get("m_LineSpacing").and_then(UnityValue::as_f64),
                character_count: rects.len(),
                has_russian,
                texture_path_id: texture.map(|pointer| pointer.path_id),
            });
        }
    }
    fonts
}

pub(in super::super) fn source_bundle_signature(file: &ClientBundleFile) -> Option<(String, u64, u128)> {
    Some((
        file.path.replace('\\', "/").to_ascii_lowercase(),
        file.size,
        file.modified_ms?,
    ))
}

pub(in super::super) fn source_bundle_signatures(files: &[ClientBundleFile]) -> Option<Vec<(String, u64, u128)>> {
    files
        .iter()
        .map(source_bundle_signature)
        .collect::<Option<Vec<_>>>()
}

pub(in super::super) fn cached_bundle_payloads_are_current(index: &ClientFileIndex) -> bool {
    index.bundles.iter().all(|bundle| {
        let Some(cache_dir) = bundle.cache_dir.as_deref().map(PathBuf::from) else {
            return false;
        };
        if !has_cached_payload(&cache_dir) {
            return false;
        }
        let source_path = PathBuf::from(&bundle.path);
        let Ok(source_meta) = source_cache_metadata(&source_path) else {
            return false;
        };
        read_cache_metadata(&cache_dir).is_some_and(|cached| {
            cached.source_path == source_meta.source_path
                && cached.source_size == source_meta.source_size
                && cached.source_modified_ms == source_meta.source_modified_ms
        })
    })
}

pub(in super::super) fn infer_cache_container(cache_dir: &Path) -> String {
    if let Some(container) = cache_container_from_metadata(cache_dir) {
        return container;
    }
    fs::read_dir(cache_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|item| item.file_name().to_str().map(str::to_string))
        .find(|name| {
            name.ends_with(".assets")
                || name.ends_with(".dll")
                || name.eq_ignore_ascii_case("mainData")
        })
        .unwrap_or_else(|| {
            cache_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("bundle")
                .to_string()
        })
}

pub(in super::super) fn cache_container_from_metadata(cache_dir: &Path) -> Option<String> {
    let metadata = read_cache_metadata(cache_dir)?;
    Path::new(&metadata.source_path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
}

pub(in super::super) fn add_container_to_entries(entries: &mut [serde_json::Value], container: &str) {
    for entry in entries.iter_mut() {
        if let Some(object) = entry.as_object_mut() {
            object.insert("container".to_string(), json!(container));
        }
    }
}

pub(in super::super) fn walk_unity_strings(
    value: &fusionforge::UnityValue,
    parts: &mut Vec<String>,
    output: &mut Vec<(String, String)>,
) {
    match value {
        fusionforge::UnityValue::String(text) => output.push((path_to_string(parts), text.clone())),
        fusionforge::UnityValue::Object(values) => {
            for (key, item) in values {
                parts.push(key.clone());
                walk_unity_strings(item, parts, output);
                parts.pop();
            }
        }
        fusionforge::UnityValue::Array(values) => {
            for (index, item) in values.iter().enumerate() {
                parts.push(format!("[{index}]"));
                walk_unity_strings(item, parts, output);
                parts.pop();
            }
        }
        fusionforge::UnityValue::Pair(left, right) => {
            parts.push("left".to_string());
            walk_unity_strings(left, parts, output);
            parts.pop();
            parts.push("right".to_string());
            walk_unity_strings(right, parts, output);
            parts.pop();
        }
        _ => {}
    }
}

pub(in super::super) fn cache_container_aliases(project_path: &Path) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    let cache_root = project_path.join("cache").join("extracted-bundles");
    let Ok(entries) = fs::read_dir(cache_root) else {
        return aliases;
    };
    for entry in entries.flatten() {
        let cache_dir = entry.path();
        if !cache_dir.is_dir() {
            continue;
        }
        let Some(container) = cache_container_from_metadata(&cache_dir) else {
            continue;
        };
        if let Some(cache_name) = cache_dir.file_name().and_then(|value| value.to_str()) {
            aliases.insert(cache_name.to_string(), container.clone());
        }
        for file in extracted_files_in_dir(&cache_dir) {
            aliases.insert(file.name, container.clone());
        }
    }
    aliases
}

pub(in super::super) fn normalize_entry_container(entry: &mut JsonValue, aliases: &BTreeMap<String, String>) {
    let Some(object) = entry.as_object_mut() else {
        return;
    };
    let Some(container) = object.get("container").and_then(JsonValue::as_str) else {
        return;
    };
    if let Some(real_container) = aliases.get(container) {
        object.insert("container".to_string(), json!(real_container));
    }
}

pub(in super::super) fn translated_unity_containers(translation_json: &Path) -> Result<BTreeSet<String>, String> {
    let entries = translation_entries_from_file(translation_json).ok_or_else(|| {
        format!(
            "Could not read translation entries from {}",
            translation_json.display()
        )
    })?;
    let mut containers = BTreeSet::new();
    for entry in entries {
        if unity_translation_entry_key(&entry).is_none() {
            continue;
        }
        if translation_text(&entry)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
        {
            if let Some(container) = entry.get("container").and_then(JsonValue::as_str) {
                containers.insert(container.to_string());
            }
        }
    }
    Ok(containers)
}

pub(in super::super) fn active_bundle_cache_dirs(bundles: &[ClientBundleFile]) -> Vec<PathBuf> {
    bundles
        .iter()
        .filter_map(|bundle| bundle.cache_dir.as_deref())
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .collect()
}

pub(in super::super) fn is_unity_or_managed_file(name: &str) -> bool {
    name.eq_ignore_ascii_case("mainData") || name.ends_with(".assets") || name.ends_with(".dll")
}

pub(in super::super) fn analyze_world_bundle_plan(document: serde_json::Value) -> EditorResult<serde_json::Value> {
    Ok(analyze_world_bundle_plan_value(&document))
}

pub(in super::super) fn analyze_world_bundle_plan_value(document: &serde_json::Value) -> serde_json::Value {
    let tile_id = document
        .get("tileId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Map_00_00");
    let source_map = document
        .get("sourceMapBundle")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let source_resource = document
        .get("sourceResourceBundle")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let objects = document
        .get("objects")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut source_counts = BTreeMap::<String, usize>::new();
    for object in &objects {
        let source = object.get("source").unwrap_or(&JsonValue::Null);
        let key = source
            .get("uri")
            .or_else(|| source.get("bundle"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();
        if !key.is_empty() {
            *source_counts.entry(key).or_default() += 1;
        }
    }

    let mut decisions = vec![
        json!({
            "id": "terrain",
            "asset": tile_id,
            "sourceKind": "terrain",
            "targetBundleClass": "map",
            "targetContainer": source_map,
            "action": "patchTerrainData",
            "reason": "Terrain height/splat edits belong to the selected Map_XX_YY scene bundle.",
            "confidence": "high",
        }),
        json!({
            "id": "scene-transforms",
            "asset": tile_id,
            "sourceKind": "sceneGraph",
            "targetBundleClass": "map",
            "targetContainer": source_map,
            "action": "patchSceneObjects",
            "reason": "GameObject hierarchy and placed-instance transforms are scene-local.",
            "confidence": "high",
        }),
    ];

    for (index, object) in objects.iter().enumerate() {
        let source = object.get("source").unwrap_or(&JsonValue::Null);
        let kind = source
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("primitive");
        let bundle = source
            .get("bundle")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let uri = source
            .get("uri")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let object_name = object
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Placed object");
        let source_key = if uri.is_empty() { bundle } else { uri };
        let repeated = source_counts.get(source_key).copied().unwrap_or_default() > 1;
        let (target_class, target_container, action, reason) = if kind == "externalModel" {
            if repeated {
                (
                    "sharedCustom",
                    format!("WorldShared_{}.resourceFile", safe_segment(tile_id)),
                    "stageTemplateImport",
                    "The same imported model is used more than once; stage it once in a generated shared world bundle.",
                )
            } else {
                (
                    "newBundle",
                    format!("World_{}_Custom.resourceFile", safe_segment(tile_id)),
                    "stageTemplateImport",
                    "GLB/OBJ imports are converted through a Unity template and packed into a generated resourceFile.",
                )
            }
        } else if !bundle.is_empty() && bundle.eq_ignore_ascii_case(source_resource) {
            (
                "tileResource",
                source_resource.to_string(),
                "referenceExisting",
                "The selected model already belongs to this tile resource bundle.",
            )
        } else if bundle.to_ascii_lowercase().contains("shared")
            || bundle.to_ascii_lowercase().contains("freearea")
        {
            (
                "sharedExisting",
                bundle.to_string(),
                "referenceExisting",
                "The asset is already owned by an existing shared/common bundle.",
            )
        } else if !bundle.is_empty() {
            (
                "newBundleCandidate",
                format!("World_{}_Refs.resourceFile", safe_segment(tile_id)),
                "copyOrReference",
                "The model comes from a different local bundle; bundle analysis must avoid mutating its original owner.",
            )
        } else {
            (
                "map",
                source_map.to_string(),
                "generatePrimitive",
                "Generated primitive placement has no external asset dependency.",
            )
        };
        decisions.push(json!({
            "id": format!("object-{index}"),
            "asset": object_name,
            "sourceKind": kind,
            "sourceBundle": if bundle.is_empty() { JsonValue::Null } else { json!(bundle) },
            "sourceUri": if uri.is_empty() { JsonValue::Null } else { json!(uri) },
            "targetBundleClass": target_class,
            "targetContainer": target_container,
            "action": action,
            "reason": reason,
            "confidence": if target_class == "newBundleCandidate" { "medium" } else { "high" },
        }));
    }

    json!({
        "policy": "autoDedupeAndNewBundles",
        "tileId": tile_id,
        "sourceMapBundle": if source_map.is_empty() { JsonValue::Null } else { json!(source_map) },
        "sourceResourceBundle": if source_resource.is_empty() { JsonValue::Null } else { json!(source_resource) },
        "decisionCount": decisions.len(),
        "decisions": decisions,
    })
}

pub(in super::super) fn inspect_bundle_textures(
    bundle_path: String,
    project_dir: Option<String>,
) -> EditorResult<serde_json::Value> {
    use fusionforge::{
        decode_texture, image_to_data_url, object_name, UnityEnvironment, UnityValue,
    };

    let bundle = PathBuf::from(&bundle_path);
    let project = project_dir
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::repository_root().to_path_buf().join("work").join("fusionforge"));
    let extract_dir = if bundle.is_dir() {
        bundle.clone()
    } else {
        extract_bundle_cached(&project, &bundle)?
    };
    let env = UnityEnvironment::from_dir(&extract_dir);
    let mut assets = Vec::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        let mut textures = Vec::new();
        for (path_id, info) in &asset.objects {
            if asset.object_type_name(info) != "Texture2D" {
                continue;
            }
            let body = match asset.read_object(asset_index, info) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let decoded = decode_texture(&env, &body);
            let preview = decoded
                .as_ref()
                .and_then(|texture| texture.image())
                .and_then(|mut image| {
                    image::imageops::flip_vertical_in_place(&mut image);
                    image_to_data_url(image, 1024, false)
                });
            textures.push(json!({
                "asset": asset.name,
                "container": bundle.file_name().and_then(|value| value.to_str()).unwrap_or(""),
                "pathId": path_id,
                "name": object_name(&body),
                "width": body.get("m_Width").and_then(UnityValue::as_i64),
                "height": body.get("m_Height").and_then(UnityValue::as_i64),
                "format": body.get("m_TextureFormat").and_then(UnityValue::as_i64),
                "mipCount": body.get("m_MipCount").and_then(UnityValue::as_i64),
                "decoded": decoded.is_some(),
                "hasAlpha": decoded.as_ref().is_some_and(|texture| texture.has_alpha()),
                "hasPartialAlpha": decoded.as_ref().is_some_and(|texture| texture.has_partial_alpha()),
                "previewDataUrl": preview,
            }));
        }
        if !textures.is_empty() {
            assets.push(json!({ "asset": asset.name, "textures": textures }));
        }
    }
    Ok(json!({ "bundlePath": bundle_path, "cacheDir": extract_dir, "assets": assets }))
}

pub(in super::super) fn unity_value_owned_bytes(value: &fusionforge::UnityValue) -> Option<Vec<u8>> {
    value
        .as_bytes()
        .map(|bytes| bytes.to_vec())
        .or_else(|| value.as_str().map(|text| text.as_bytes().to_vec()))
}

pub(in super::super) fn preview_bundle_container_textures(
    bundle_path: String,
    project_dir: Option<String>,
    container_paths: Vec<String>,
    max_size: u32,
) -> EditorResult<Vec<NpcIconPreview>> {
    let bundle = PathBuf::from(&bundle_path);
    let project = project_dir
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::repository_root().to_path_buf().join("work").join("fusionforge"));
    let extract_dir = if bundle.is_dir() {
        bundle.clone()
    } else {
        extract_bundle_cached(&project, &bundle)?
    };
    let env = fusionforge::UnityEnvironment::from_dir(&extract_dir);
    let wanted = container_paths
        .iter()
        .map(|path| (normalized_asset_path(path), path.clone()))
        .filter(|(path, _)| !path.is_empty())
        .collect::<BTreeMap<_, _>>();
    let mut results = wanted
        .iter()
        .map(|(normalized, original)| {
            (
                normalized.clone(),
                NpcIconPreview {
                    icon_path: original.clone(),
                    bundle_path: bundle_path.clone(),
                    container_path: None,
                    asset: None,
                    path_id: None,
                    name: None,
                    width: None,
                    height: None,
                    data_url: None,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let body = asset.read_object(asset_index, info)?;
            let preload_table = fusionforge::value_array(body.get("m_PreloadTable")).to_vec();
            for entry in fusionforge::value_array(body.get("m_Container")) {
                let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                    continue;
                };
                let lower = normalized_asset_path(path);
                let Some((wanted_key, icon_path)) = matched_wanted_container_path(&wanted, &lower)
                else {
                    continue;
                };
                if results
                    .get(wanted_key)
                    .is_some_and(|preview| preview.data_url.is_some())
                {
                    continue;
                }
                if let Some(result) = results.get_mut(wanted_key) {
                    result.container_path = Some(path.to_string());
                }

                if let Some(pointer) = metadata
                    .get("asset")
                    .and_then(fusionforge::UnityValue::as_pointer)
                {
                    if let Ok(key) = env.resolve_pointer(pointer) {
                        if let Some(preview) =
                            texture_preview_from_key(&env, key.asset, key.path_id, max_size)
                        {
                            results.insert(
                                wanted_key.to_string(),
                                fill_icon_preview_from_texture_value(
                                    &preview,
                                    icon_path,
                                    &bundle_path,
                                    path,
                                ),
                            );
                            continue;
                        }
                    }
                }

                let (start, end) = metadata_preload_range(metadata, preload_table.len());
                for preload in &preload_table[start..end] {
                    let Some(pointer) = preload.as_pointer() else {
                        continue;
                    };
                    let Ok(key) = env.resolve_pointer(pointer) else {
                        continue;
                    };
                    let Some(preview) =
                        texture_preview_from_key(&env, key.asset, key.path_id, max_size)
                    else {
                        continue;
                    };
                    results.insert(
                        wanted_key.to_string(),
                        fill_icon_preview_from_texture_value(
                            &preview,
                            icon_path,
                            &bundle_path,
                            path,
                        ),
                    );
                    break;
                }
            }
        }
    }

    Ok(results.into_values().collect())
}

pub(in super::super) fn suppress_exact_kfm_parse_warning_for_serialized_hierarchy(
    result: &mut JsonValue,
    normalized_route: &str,
    matched_paths: &[String],
) -> Option<JsonValue> {
    if !normalized_route.ends_with(".kfm") || matched_paths.len() != 1 {
        return None;
    }
    let targets = result.get("exactContainerTargets")?.as_array()?;
    if targets.len() != 1 {
        return None;
    }
    let target = targets[0].clone();
    let object_type = target.get("objectType").and_then(JsonValue::as_str)?;
    if target.get("proof").and_then(JsonValue::as_str) != Some("directAssetBundleContainerPointer")
        || !matches!(object_type, "GameObject" | "Transform")
        || target
            .get("exactRoute")
            .and_then(JsonValue::as_str)
            .map(normalized_asset_path)
            .as_deref()
            != Some(normalized_route)
    {
        return None;
    }

    let roots = result.pointer("/modelHierarchy/roots")?.as_array()?;
    if roots.len() != 1 {
        return None;
    }
    let root = roots[0].clone();
    let target_matches_root = root.get("sourceAssetIndex").and_then(JsonValue::as_u64)
        == target.get("assetIndex").and_then(JsonValue::as_u64)
        && match object_type {
            "GameObject" => {
                root.get("gameObjectPathId").and_then(JsonValue::as_i64)
                    == target.get("pathId").and_then(JsonValue::as_i64)
            }
            "Transform" => {
                root.get("transformPathId").and_then(JsonValue::as_i64)
                    == target.get("pathId").and_then(JsonValue::as_i64)
            }
            _ => false,
        };
    if !target_matches_root {
        return None;
    }

    let warning_prefix = format!("{}: exact pointer graph exhausted ", matched_paths[0]);
    let warnings = result.get_mut("warnings")?.as_array_mut()?;
    let matching = warnings
        .iter()
        .enumerate()
        .filter(|(_, warning)| {
            warning
                .as_str()
                .is_some_and(|warning| warning.starts_with(&warning_prefix))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return None;
    }
    let suppressed_warning = warnings.remove(matching[0]).as_str().map(str::to_string)?;
    let evidence = json!({
        "schema": "ffone.exact-serialized-hierarchy-closure.v1",
        "proof": "directAssetBundleContainerPointerMatchesOnlyHierarchyRoot",
        "exactRoute": matched_paths[0],
        "containerAssetName": target.get("containerAssetName"),
        "assetIndex": target.get("assetIndex"),
        "assetName": target.get("assetName"),
        "objectType": object_type,
        "objectPathId": target.get("pathId"),
        "rootGameObjectPathId": root.get("gameObjectPathId"),
        "rootTransformPathId": root.get("transformPathId"),
        "rootName": root.get("name"),
        "suppressedWarning": suppressed_warning,
        "byteParseDisposition": "attempted-then-suppressed-as-non-authoritative",
        "warningDisposition": "legacy-kfm-byte-parse-is-non-authoritative-for-direct-serialized-hierarchy-route",
    });
    result.as_object_mut()?.insert(
        "exactSerializedHierarchyClosure".to_string(),
        evidence.clone(),
    );
    Some(evidence)
}
