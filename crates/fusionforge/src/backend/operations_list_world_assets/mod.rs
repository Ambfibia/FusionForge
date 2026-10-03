use super::super::*;

pub(in super::super) fn split_top_level_comma(data: &str) -> Option<(&str, &str)> {
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut brace = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    for (index, ch) in data.char_indices() {
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
            '(' => paren += 1,
            ')' => paren = paren.saturating_sub(1),
            '[' => bracket += 1,
            ']' => bracket = bracket.saturating_sub(1),
            '{' => brace += 1,
            '}' => brace = brace.saturating_sub(1),
            ',' if paren == 0 && bracket == 0 && brace == 0 => {
                return Some((&data[..index], &data[index + 1..]));
            }
            _ => {}
        }
    }
    None
}

pub(in super::super) fn replace_csharp_call_with_argument(data: &str, function_name: &str) -> String {
    let mut output = String::with_capacity(data.len());
    let mut cursor = 0usize;
    let needle = format!("{function_name}(");
    while let Some(relative) = data[cursor..].find(&needle) {
        let index = cursor + relative;
        let paren = index + function_name.len();
        let Some(close) = find_matching_paren(data, paren) else {
            break;
        };
        output.push_str(&data[cursor..index]);
        output.push_str(&data[paren + 1..close]);
        cursor = close + 1;
    }
    output.push_str(&data[cursor..]);
    output
}

pub(in super::super) fn contains_public_class(data: &str, name: &str) -> bool {
    data.match_indices("public class ").any(|(index, _)| {
        let class_name_start = index + "public class ".len();
        data[class_name_start..]
            .strip_prefix(name)
            .is_some_and(|tail| {
                tail.chars()
                    .next()
                    .is_some_and(|ch| ch.is_whitespace() || ch == ':' || ch == '<')
            })
    })
}

pub(in super::super) fn contains_token(data: &str, token: &str) -> bool {
    data.match_indices(token).any(|(index, _)| {
        let before = data[..index].chars().next_back();
        let after = data[index + token.len()..].chars().next();
        !before.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            && !after.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    })
}

pub(in super::super) fn normalize_decompiled_csproj(
    project_dir: &Path,
    assembly_path: &Path,
    csproj: &Path,
) -> EditorResult<()> {
    let assembly_parent = assembly_path.parent();
    stage_script_reference_dlls(project_dir, assembly_parent)?;
    let refs = collect_reference_assemblies(project_dir, assembly_parent);
    let mut data =
        fs::read_to_string(csproj).map_err(|err| format!("{}: {err}", csproj.display()))?;
    data = data.replace(
        "<LangVersion>Preview</LangVersion>",
        "<LangVersion>latest</LangVersion>",
    );
    for (name, path) in refs {
        let self_closing = format!(r#"<Reference Include="{name}" />"#);
        if data.contains(&self_closing) {
            let hint = path.to_string_lossy().replace('\\', "/");
            let block = format!(
                "<Reference Include=\"{name}\">\n      <HintPath>{hint}</HintPath>\n      <Private>False</Private>\n    </Reference>"
            );
            data = data.replace(&self_closing, &block);
            continue;
        }
        let include = format!(r#"<Reference Include="{name}">"#);
        if !data.contains(&include)
            && matches!(
                name.as_str(),
                "mscorlib" | "System" | "UnityEngine" | "Boo.Lang" | "UnityScript.Lang"
            )
        {
            let hint = path.to_string_lossy().replace('\\', "/");
            let block = format!(
                "    <Reference Include=\"{name}\">\n      <HintPath>{hint}</HintPath>\n      <Private>False</Private>\n    </Reference>\n"
            );
            if let Some(index) = data.rfind("  </ItemGroup>") {
                data.insert_str(index, &block);
            }
        }
    }
    fs::write(csproj, data).map_err(|err| format!("{}: {err}", csproj.display()))
}

pub(in super::super) fn scan_client_files(
    root: &Path,
    bundles: &mut Vec<ClientBundleFile>,
    unity_files: &mut Vec<ClientBundleFile>,
) -> Result<(), EditorError> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_client_files(&path, bundles, unity_files)?;
            continue;
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        let metadata = path.metadata().ok();
        let size = metadata.as_ref().map(|meta| meta.len()).unwrap_or(0);
        let modified_ms = metadata
            .as_ref()
            .and_then(|meta| metadata_modified_ms(meta).ok());
        let file = ClientBundleFile {
            path: path.to_string_lossy().to_string(),
            name,
            extension: extension.clone(),
            size,
            modified_ms,
            cache_dir: None,
            extracted_files: Vec::new(),
            assets: Vec::new(),
            errors: Vec::new(),
        };
        if extension.eq_ignore_ascii_case("unity3d")
            || extension.eq_ignore_ascii_case("resourceFile")
        {
            bundles.push(file);
        } else if extension.eq_ignore_ascii_case("assets") || extension.eq_ignore_ascii_case("dll")
        {
            unity_files.push(file);
        }
    }
    Ok(())
}

pub(in super::super) fn cached_extract_dir(project_dir: &Path, bundle: &Path) -> PathBuf {
    let cache_identity = fs::canonicalize(bundle).unwrap_or_else(|_| bundle.to_path_buf());
    project_dir
        .join("cache")
        .join("extracted-bundles")
        .join(path_hash(&cache_identity))
}

pub(in super::super) fn source_cache_metadata(bundle: &Path) -> Result<CacheMetadata, String> {
    let canonical = fs::canonicalize(bundle).unwrap_or_else(|_| bundle.to_path_buf());
    let metadata = canonical.metadata().map_err(|err| err.to_string())?;
    Ok(CacheMetadata {
        source_path: canonical.to_string_lossy().to_string(),
        source_size: metadata.len(),
        source_modified_ms: metadata_modified_ms(&metadata)?,
    })
}

pub(in super::super) fn make_cache_tree_writable(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        for entry in fs::read_dir(path).map_err(|err| format!("{}: {err}", path.display()))? {
            let entry = entry.map_err(|err| format!("{}: {err}", path.display()))?;
            make_cache_tree_writable(&entry.path())?;
        }
    }
    let metadata = fs::metadata(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)
            .map_err(|err| format!("{}: {err}", path.display()))?;
    }
    Ok(())
}

pub(in super::super) fn normalize_font_family(name: &str) -> String {
    let trimmed = name.trim();
    let without_small = trimmed
        .strip_suffix(" Small 1")
        .or_else(|| trimmed.strip_suffix(" Small"))
        .unwrap_or(trimmed);
    let family = without_small
        .trim_end_matches(|ch: char| {
            ch.is_ascii_digit() || ch == '_' || ch == '-' || ch.is_whitespace()
        })
        .trim();
    if family.to_ascii_uppercase().starts_with("JEFFE") {
        "JEFFE".to_string()
    } else if family.is_empty() {
        trimmed.to_string()
    } else {
        family.to_string()
    }
}

pub(in super::super) fn is_latin_or_cyrillic_text(value: &str) -> bool {
    value.chars().all(|ch| {
        !ch.is_alphabetic()
            || ch.is_ascii_alphabetic()
            || ('\u{0400}'..='\u{04FF}').contains(&ch)
            || ('\u{0500}'..='\u{052F}').contains(&ch)
    })
}

pub(in super::super) fn is_internal_identifier_source(value: &str) -> bool {
    let stripped = value.trim();
    if !stripped.contains('_') || stripped.chars().any(char::is_whitespace) {
        return false;
    }
    stripped
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

pub(in super::super) fn is_managed_code_expression_source(value: &str) -> bool {
    let stripped = value.trim();
    if stripped.is_empty() {
        return false;
    }
    let code_tokens = [
        "==",
        "!=",
        ">=",
        "<=",
        "&&",
        "||",
        "(int)",
        "sItemType.",
        "eItemType.",
        "eSlotType.",
        ".ItemType",
        ".iSlotType",
        ".iChangeGuideCount",
        "MENTOR_CHANGE_BASE_COST",
    ];
    code_tokens.iter().any(|token| stripped.contains(token))
        || (stripped.contains(" * ") && stripped.contains('.'))
}

pub(in super::super) fn is_localizable_source(value: &str) -> bool {
    let stripped = value.trim();
    if stripped.is_empty() {
        return false;
    }
    let lower = stripped.to_ascii_lowercase();
    !matches!(lower.as_str(), "0" | "-1" | "null" | "none")
        && !is_asset_filename_source(stripped)
        && !is_internal_identifier_source(stripped)
        && is_latin_or_cyrillic_text(value)
}

pub(in super::super) fn remove_translated_source_duplicate_entries(entries: &mut Vec<JsonValue>) -> usize {
    let mut translated_by_location = BTreeMap::<String, BTreeSet<String>>::new();
    for entry in entries.iter() {
        let Some(location) = translation_dedup_location_key(entry) else {
            continue;
        };
        let Some(translation) = translation_text(entry).filter(|value| !value.is_empty()) else {
            continue;
        };
        translated_by_location
            .entry(location)
            .or_default()
            .insert(normalized_translation_compare_text(translation));
    }

    let before = entries.len();
    entries.retain(|entry| {
        if translation_text(entry)
            .map(|value| !value.is_empty())
            .unwrap_or(false)
        {
            return true;
        }
        let Some(source) = entry.get("source").and_then(JsonValue::as_str) else {
            return true;
        };
        let Some(location) = translation_dedup_location_key(entry) else {
            return true;
        };
        !translated_by_location
            .get(&location)
            .is_some_and(|translations| {
                translations.contains(&normalized_translation_compare_text(source))
            })
    });
    before.saturating_sub(entries.len())
}

pub(in super::super) fn has_managed_assemblies(cache_dir: &Path) -> bool {
    extracted_files_in_dir(cache_dir)
        .iter()
        .any(|file| file.name.ends_with(".dll"))
}

pub(in super::super) fn unique_source_translations(entries: &[JsonValue]) -> BTreeMap<String, Option<String>> {
    let mut result = BTreeMap::<String, Option<String>>::new();
    for entry in entries {
        let Some(source) = entry.get("source").and_then(JsonValue::as_str) else {
            continue;
        };
        let Some(translation) = translation_text(entry).filter(|value| !value.is_empty()) else {
            continue;
        };
        result
            .entry(source.to_string())
            .and_modify(|current| {
                if current.as_deref() != Some(translation) {
                    *current = None;
                }
            })
            .or_insert_with(|| Some(translation.to_string()));
    }
    result
}

pub(in super::super) fn extracted_files_in_dir(cache_dir: &Path) -> Vec<ClientExtractedFile> {
    let Ok(entries) = fs::read_dir(cache_dir) else {
        return Vec::new();
    };
    let mut files = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            if !path.is_file() {
                return false;
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            name != "cache-meta.json" && !name.ends_with("-translation.index.json")
        })
        .map(|path| ClientExtractedFile {
            name: path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_string(),
            size: path.metadata().map(|meta| meta.len()).unwrap_or(0),
            path: path.to_string_lossy().to_string(),
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.name.cmp(&b.name));
    files
}

pub(in super::super) fn default_document(tile_id: Option<String>) -> TileDocument {
    let width = 65;
    let height = 65;
    let texture_resolution = 128;
    let mut heights = vec![0.0; (width * height) as usize];

    for z in 0..height {
        for x in 0..width {
            let fx = x as f32 / (width - 1) as f32 - 0.5;
            let fz = z as f32 / (height - 1) as f32 - 0.5;
            let ridge = (1.0 - (fx * fx + fz * fz).sqrt()).max(0.0);
            heights[(z * width + x) as usize] = ridge * 3.0;
        }
    }

    TileDocument {
        schema_version: 2,
        tile_id: tile_id.unwrap_or_else(|| "Map_00_00".to_string()),
        source_map_bundle: None,
        source_resource_bundle: None,
        terrain: TerrainDocument {
            width,
            height,
            world_size: 512.0,
            max_height: 20000.0,
            heights,
            texture_resolution,
            texture_map: vec![0; (texture_resolution * texture_resolution) as usize],
            texture_layers: vec![
                TerrainTextureLayer {
                    id: "grass".into(),
                    name: "Grass".into(),
                    color: "#4f8f57".into(),
                    unity_material_path_id: None,
                },
                TerrainTextureLayer {
                    id: "dirt".into(),
                    name: "Dirt".into(),
                    color: "#8c6a47".into(),
                    unity_material_path_id: None,
                },
                TerrainTextureLayer {
                    id: "rock".into(),
                    name: "Rock".into(),
                    color: "#777d82".into(),
                    unity_material_path_id: None,
                },
                TerrainTextureLayer {
                    id: "sand".into(),
                    name: "Sand".into(),
                    color: "#c2ad72".into(),
                    unity_material_path_id: None,
                },
            ],
            preview_texture_data_url: None,
            origin: None,
            axis_x: None,
            axis_z: None,
            height_axis: None,
            flip_x: None,
            reverse_winding: None,
        },
        objects: Vec::new(),
        colliders: Vec::new(),
        water_volumes: Vec::new(),
    }
}

pub(in super::super) fn scan_recursive(
    root: &Path,
    maps: &mut Vec<String>,
    resources: &mut Vec<String>,
) -> Result<(), EditorError> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_recursive(&path, maps, resources)?;
            continue;
        }

        let Some(name) = path.file_name().and_then(|x| x.to_str()) else {
            continue;
        };
        if name.starts_with("Map_") && name.ends_with(".unity3d") {
            maps.push(path.to_string_lossy().to_string());
        } else if name.starts_with("DongResources_") && name.ends_with(".resourceFile") {
            resources.push(path.to_string_lossy().to_string());
        }
    }
    Ok(())
}

pub(in super::super) fn migrate_world_document_json(document: &mut serde_json::Value) {
    let Some(object) = document.as_object_mut() else {
        return;
    };
    let schema_version = object
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1);
    if schema_version < 2 {
        object.insert("schemaVersion".to_string(), json!(2));
    }
    object
        .entry("objects".to_string())
        .or_insert_with(|| json!([]));
    object
        .entry("colliders".to_string())
        .or_insert_with(|| json!([]));
    object
        .entry("waterVolumes".to_string())
        .or_insert_with(|| json!([]));
    object
        .entry("previewMeshes".to_string())
        .or_insert_with(|| json!([]));
    object
        .entry("previewMaterials".to_string())
        .or_insert_with(|| json!({}));
    object
        .entry("previewTerrains".to_string())
        .or_insert_with(|| json!([]));
}

pub(in super::super) fn json_array_len(value: &serde_json::Value, path: &[&str]) -> usize {
    let mut cursor = value;
    for key in path {
        let Some(next) = cursor.get(*key) else {
            return 0;
        };
        cursor = next;
    }
    cursor.as_array().map(Vec::len).unwrap_or_default()
}

pub(in super::super) fn new_tile_document(tile_id: Option<String>) -> TileDocument {
    default_document(tile_id)
}

pub(in super::super) fn list_world_assets(
    project_dir: String,
    query: Option<String>,
    limit: Option<usize>,
) -> EditorResult<serde_json::Value> {
    let project = PathBuf::from(&project_dir);
    let index_path = project.join("cache").join("bundle-index.json");
    let index = serde_json::from_str::<ClientFileIndex>(
        &fs::read_to_string(&index_path)
            .map_err(|err| format!("{}: {err}", index_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", index_path.display()))?;
    let needle = query.unwrap_or_default().trim().to_ascii_lowercase();
    let limit = limit.unwrap_or(400).clamp(1, 2000);
    let mut entries = Vec::<JsonValue>::new();

    for bundle in &index.bundles {
        for asset in &bundle.assets {
            for path in &asset.container_paths {
                let kind = world_asset_kind(path, &asset.type_counts);
                if kind == "other" {
                    continue;
                }
                let searchable = format!("{} {} {} {}", bundle.name, asset.name, path, kind)
                    .to_ascii_lowercase();
                if !needle.is_empty() && !searchable.contains(&needle) {
                    continue;
                }
                entries.push(json!({
                    "id": stable_hash(&format!("{}|{}|{}", bundle.path, asset.name, path)),
                    "kind": kind,
                    "name": Path::new(path)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or(path),
                    "containerPath": path,
                    "bundlePath": bundle.path,
                    "bundleName": bundle.name,
                    "asset": asset.name,
                    "objectCount": asset.object_count,
                    "typeCounts": asset.type_counts,
                }));
            }
            if asset.container_paths.is_empty() {
                let kind = if asset.type_counts.get("Mesh").copied().unwrap_or_default() > 0 {
                    "model"
                } else if asset
                    .type_counts
                    .get("Material")
                    .copied()
                    .unwrap_or_default()
                    > 0
                {
                    "material"
                } else if asset
                    .type_counts
                    .get("Texture2D")
                    .copied()
                    .unwrap_or_default()
                    > 0
                {
                    "texture"
                } else {
                    "other"
                };
                if kind == "other" {
                    continue;
                }
                let searchable =
                    format!("{} {} {}", bundle.name, asset.name, kind).to_ascii_lowercase();
                if !needle.is_empty() && !searchable.contains(&needle) {
                    continue;
                }
                entries.push(json!({
                    "id": stable_hash(&format!("{}|{}", bundle.path, asset.name)),
                    "kind": kind,
                    "name": asset.name,
                    "containerPath": null,
                    "bundlePath": bundle.path,
                    "bundleName": bundle.name,
                    "asset": asset.name,
                    "objectCount": asset.object_count,
                    "typeCounts": asset.type_counts,
                }));
            }
            if entries.len() >= limit {
                break;
            }
        }
        if entries.len() >= limit {
            break;
        }
    }

    let model_count = entries
        .iter()
        .filter(|entry| entry.get("kind").and_then(JsonValue::as_str) == Some("model"))
        .count();
    let texture_count = entries
        .iter()
        .filter(|entry| entry.get("kind").and_then(JsonValue::as_str) == Some("texture"))
        .count();
    let material_count = entries
        .iter()
        .filter(|entry| entry.get("kind").and_then(JsonValue::as_str) == Some("material"))
        .count();
    Ok(json!({
        "projectDir": project_dir,
        "entries": entries,
        "groups": {
            "models": model_count,
            "textures": texture_count,
            "materials": material_count,
        }
    }))
}

pub(in super::super) fn stripped_world_authoring_document(document: &serde_json::Value) -> serde_json::Value {
    let mut value = document.clone();
    if let Some(object) = value.as_object_mut() {
        object.remove("previewMeshes");
        object.remove("previewMaterials");
        object.remove("previewTerrains");
        if let Some(terrain) = object
            .get_mut("terrain")
            .and_then(serde_json::Value::as_object_mut)
        {
            terrain.remove("previewTextureDataUrl");
            terrain.remove("splatAlphaDataUrls");
            if let Some(layers) = terrain
                .get_mut("textureLayers")
                .and_then(serde_json::Value::as_array_mut)
            {
                for layer in layers {
                    if let Some(layer) = layer.as_object_mut() {
                        layer.remove("textureDataUrl");
                    }
                }
            }
        }
    }
    value
}

pub(in super::super) fn imported_vec3_curves_to_preview(
    curves: &[fusionforge::modding::ImportedVec3Curve],
) -> Vec<JsonValue> {
    curves
        .iter()
        .map(|curve| {
            json!({
                "path": curve.path,
                "keys": curve.keys.iter().map(|key| {
                    json!({
                        "time": key.time,
                        "value": [key.value.0, key.value.1, key.value.2],
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect()
}

pub(in super::super) fn imported_quat_curves_to_preview(
    curves: &[fusionforge::modding::ImportedQuatCurve],
) -> Vec<JsonValue> {
    curves
        .iter()
        .map(|curve| {
            json!({
                "path": curve.path,
                "keys": curve.keys.iter().map(|key| {
                    json!({
                        "time": key.time,
                        "value": [key.value.0, key.value.1, key.value.2, key.value.3],
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect()
}

pub(in super::super) fn kfm_reference_paths(bytes: &[u8]) -> Vec<String> {
    kfm_reference_paths_exact(bytes)
        .into_iter()
        .take(128)
        .collect()
}

pub(crate) fn kfm_reference_paths_exact(bytes: &[u8]) -> Vec<String> {
    let strings = ascii_asset_strings(bytes, 4);
    strings
        .iter()
        .filter_map(|value| {
            let lower = value.replace('\\', "/").to_ascii_lowercase();
            if lower.ends_with(".nif") || lower.ends_with(".kf") || lower.ends_with(".kfm") {
                Some(value.replace('\\', "/"))
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
}

pub(in super::super) fn kfm_preview_from_bytes(path: &str, source_asset: &str, bytes: &[u8]) -> JsonValue {
    let references = kfm_reference_paths(bytes);
    kfm_preview_from_bytes_with_references(path, source_asset, bytes, &references)
}

pub(in super::super) fn kfm_preview_from_bytes_with_references(
    path: &str,
    source_asset: &str,
    bytes: &[u8],
    references: &[String],
) -> JsonValue {
    let clips = references
        .iter()
        .filter(|value| value.to_ascii_lowercase().ends_with(".kf"))
        .map(|value| {
            json!({
                "name": Path::new(value).file_stem().and_then(|name| name.to_str()).unwrap_or(value),
                "path": value,
                "source": "kfm-reference",
                "canPreviewPose": false,
                "previewSupport": "metadata-only",
                "previewUnsupportedReason": "KFM/KF controller and NIF skin playback are not decoded by the native preview yet.",
            })
        })
        .collect::<Vec<_>>();
    json!({
        "source": "kfm",
        "path": path,
        "asset": source_asset,
        "byteLength": bytes.len(),
        "references": references,
        "clips": clips,
        "canPreviewPose": false,
        "previewSupport": "metadata-only",
        "previewUnsupportedReason": "KFM/KF controller and NIF skin playback are not decoded by the native preview yet.",
    })
}
