use super::super::*;

pub(in super::super) const PROJECT_TRANSLATION_INDEX_RELATIVE: &str = "translations/translation.index.json";

pub(in super::super) fn project_translation_index_path(project: &Path) -> PathBuf {
    resolve_project_path(project, PROJECT_TRANSLATION_INDEX_RELATIVE)
}

pub(in super::super) fn cache_dir_has_unity_translation_inputs(cache_dir: &Path) -> bool {
    extracted_files_in_dir(cache_dir).into_iter().any(|file| {
        !file.name.ends_with(".dll")
            && file.name != "mainData"
            && file.name != "manifest.json"
            && file.name != "cache-meta.json"
            && !file.name.ends_with("-translation.index.json")
    })
}

pub(in super::super) fn generated_translation_index_file_ready(path: &Path, generator: &str) -> bool {
    fs::read_to_string(path)
        .is_ok_and(|text| text.contains("\"generatedBy\"") && text.contains(generator))
}

pub(in super::super) fn cached_translation_indexes_are_ready(index: &ClientFileIndex) -> bool {
    index.bundles.iter().all(|bundle| {
        let Some(cache_dir) = bundle.cache_dir.as_deref().map(PathBuf::from) else {
            return false;
        };
        if cache_dir_has_unity_translation_inputs(&cache_dir)
            && !generated_translation_index_file_ready(
                &cache_dir.join("unity-object-translation.index.json"),
                RUST_UNITY_TRANSLATION_GENERATOR,
            )
        {
            return false;
        }
        if has_managed_assemblies(&cache_dir)
            && !generated_translation_index_file_ready(
                &cache_dir.join("managed-ui-translation.index.json"),
                fusionforge::managed::MANAGED_TRANSLATION_GENERATOR,
            )
        {
            return false;
        }
        true
    })
}

pub(in super::super) fn translation_entries_from_file(path: &Path) -> Option<Vec<serde_json::Value>> {
    let text = fs::read_to_string(path).ok()?;
    let mut entries = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|value| {
            value
                .get("entries")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })?;
    sanitize_translation_entries(&mut entries);
    Some(entries)
}

pub(in super::super) fn translation_entries_from_generated_file(
    path: &Path,
    generator: &str,
) -> Option<Vec<serde_json::Value>> {
    let text = fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    if value.get("generatedBy").and_then(serde_json::Value::as_str) != Some(generator) {
        return None;
    }
    let mut entries = value
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .cloned()?;
    sanitize_translation_entries(&mut entries);
    Some(entries)
}

pub(in super::super) const RUST_UNITY_TRANSLATION_GENERATOR: &str = "fusionforge.rust-unity-strings.v5";

pub(in super::super) fn is_unsupported_unity_translation_field_path(field_path: &str) -> bool {
    (field_path.contains("GUILanguageData") && field_path.contains(".m_String["))
        || is_internal_npc_alias_field_path(field_path)
        || is_shiny_internal_name_field_path(field_path)
        || is_filter_dictionary_field_path(field_path)
        || is_internal_help_page_field_path(field_path)
        || is_internal_first_use_field_path(field_path)
        || is_internal_rules_field_path(field_path)
}

pub(in super::super) fn should_keep_translation_entry(entry: &JsonValue) -> bool {
    if entry
        .get("source")
        .and_then(JsonValue::as_str)
        .is_some_and(|source| !is_localizable_source(source))
    {
        return false;
    }
    let kind = entry.get("kind").and_then(JsonValue::as_str).unwrap_or("");
    if kind == "managed.ldstr"
        && entry
            .get("source")
            .and_then(JsonValue::as_str)
            .is_some_and(is_managed_code_expression_source)
    {
        return false;
    }
    if kind != "unity.objectString" {
        return true;
    }
    entry
        .get("fieldPath")
        .and_then(JsonValue::as_str)
        .map(|field_path| !is_unsupported_unity_translation_field_path(field_path))
        .unwrap_or(true)
}

pub(in super::super) fn translation_dedup_location_key(entry: &JsonValue) -> Option<String> {
    let object = entry.as_object()?;
    let mut location = serde_json::Map::new();
    for (key, value) in object {
        if matches!(
            key.as_str(),
            "id" | "source" | "Source" | "translation" | "Translation" | "original" | "Original"
        ) {
            continue;
        }
        location.insert(key.clone(), value.clone());
    }
    if location.is_empty() {
        return None;
    }
    serde_json::to_string(&JsonValue::Object(location)).ok()
}

pub(in super::super) fn sanitize_translation_entries(entries: &mut Vec<JsonValue>) {
    entries.retain(should_keep_translation_entry);
    normalize_translation_entries(entries);
    remove_translated_source_duplicate_entries(entries);
}

pub(in super::super) fn rust_unity_translation_entries(
    cache_dir: &Path,
    container: &str,
    asset_names: &[String],
) -> Vec<serde_json::Value> {
    if container.eq_ignore_ascii_case("TableData.resourceFile") {
        let cache_dir = cache_dir.to_path_buf();
        let container = container.to_string();
        let asset_names = asset_names.to_vec();
        return run_table_data_task(move || {
            Ok(rust_unity_translation_entries_impl(
                &cache_dir,
                &container,
                &asset_names,
            ))
        })
        .unwrap_or_default();
    }
    rust_unity_translation_entries_impl(cache_dir, container, asset_names)
}

pub(in super::super) fn rust_unity_translation_entries_impl(
    cache_dir: &Path,
    container: &str,
    asset_names: &[String],
) -> Vec<serde_json::Value> {
    let env = fusionforge::UnityEnvironment::from_dir(cache_dir);
    let wanted = asset_names.iter().cloned().collect::<BTreeSet<_>>();
    let mut entries = Vec::<serde_json::Value>::new();

    for (asset_index, asset) in env.assets.iter().enumerate() {
        if !wanted.is_empty() && !wanted.contains(&asset.name) {
            continue;
        }
        for (path_id, info) in &asset.objects {
            let obj_type = asset.object_type_name(info);
            if !matches!(obj_type.as_str(), "TextAsset" | "MonoBehaviour") {
                continue;
            }
            let Ok(body) = asset.read_object(asset_index, info) else {
                continue;
            };
            if obj_type == "TextAsset" {
                let source = body
                    .get("m_Script")
                    .and_then(|value| {
                        value
                            .as_bytes()
                            .map(|bytes| String::from_utf8_lossy(bytes).to_string())
                            .or_else(|| value.as_str().map(str::to_string))
                    })
                    .unwrap_or_default();
                if !is_localizable_source(&source) {
                    continue;
                }
                let name = body
                    .get("m_Name")
                    .and_then(fusionforge::UnityValue::as_str)
                    .unwrap_or("");
                let id_hash =
                    stable_hash(&format!("{container}|{}|{path_id}|{source}", asset.name));
                entries.push(json!({
                    "id": format!("unity.textAsset:{container}:{}:{path_id}:{id_hash}", asset.name),
                    "kind": "unity.textAsset",
                    "container": container,
                    "file": asset.name,
                    "pathId": path_id,
                    "name": name,
                    "source": source,
                    "translation": "",
                }));
                continue;
            }

            let object_name = body
                .get("m_Name")
                .and_then(fusionforge::UnityValue::as_str)
                .unwrap_or("");
            let mut strings = Vec::<(String, String)>::new();
            walk_unity_strings(&body, &mut Vec::new(), &mut strings);
            for (field_path, source) in strings {
                if !should_export_unity_field_path(&field_path) || !is_localizable_source(&source) {
                    continue;
                }
                let id_hash = stable_hash(&format!(
                    "{container}|{}|{path_id}|{field_path}|{source}",
                    asset.name
                ));
                entries.push(json!({
                    "id": format!("unity.objectString:{container}:{}:{path_id}:{id_hash}", asset.name),
                    "kind": "unity.objectString",
                    "container": container,
                    "file": asset.name,
                    "pathId": path_id,
                    "objectName": object_name,
                    "objectType": obj_type,
                    "fieldPath": field_path,
                    "source": source,
                    "translation": "",
                }));
            }
        }
    }
    entries
}

pub(in super::super) fn npc_import_translation_entries(project_path: &Path) -> Vec<serde_json::Value> {
    let mut manifests = Vec::new();
    collect_npc_import_manifest_paths(&project_path.join("npcs"), &mut manifests);
    let mut entries = Vec::new();
    for manifest_path in manifests {
        let Ok(manifest_text) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<JsonValue>(&manifest_text) else {
            continue;
        };
        let Some(patch_file) = manifest
            .get("tableDataPatch")
            .and_then(|value| value.get("file"))
            .and_then(JsonValue::as_str)
        else {
            continue;
        };
        let patch_path = resolve_project_path(project_path, patch_file);
        let Ok(patch_text) = fs::read_to_string(&patch_path) else {
            continue;
        };
        let Ok(patch_document) = serde_json::from_str::<JsonValue>(&patch_text) else {
            continue;
        };
        let value = patch_document.get("value").unwrap_or(&patch_document);
        let Ok(body) = json_to_unity_value(value, 0) else {
            continue;
        };
        let container = patch_document
            .get("container")
            .and_then(JsonValue::as_str)
            .unwrap_or("TableData.resourceFile");
        let file = patch_document
            .get("asset")
            .and_then(JsonValue::as_str)
            .unwrap_or("CustomAssetBundle");
        let path_id = patch_document
            .get("pathId")
            .or_else(|| patch_document.get("path_id"))
            .and_then(JsonValue::as_i64)
            .unwrap_or_default();
        let object_name = patch_document
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or("xdtdatas");
        let object_type = patch_document
            .get("objectType")
            .and_then(JsonValue::as_str)
            .unwrap_or("MonoBehaviour");
        let mut strings = Vec::<(String, String)>::new();
        walk_unity_strings(&body, &mut Vec::new(), &mut strings);
        for (field_path, source) in strings {
            if !should_export_unity_field_path(&field_path) || !is_localizable_source(&source) {
                continue;
            }
            let id_hash = stable_hash(&format!(
                "{container}|{file}|{path_id}|{field_path}|{source}"
            ));
            entries.push(json!({
                "id": format!("unity.objectString:{container}:{file}:{path_id}:{id_hash}"),
                "kind": "unity.objectString",
                "container": container,
                "file": file,
                "pathId": path_id,
                "objectName": object_name,
                "objectType": object_type,
                "fieldPath": field_path,
                "source": source,
                "translation": "",
            }));
        }
    }
    entries
}

pub(in super::super) fn collect_translation_entries(cache_dir: &Path, container: &str) -> Vec<serde_json::Value> {
    let out_path = cache_dir.join("unity-object-translation.index.json");
    if let Some(mut entries) =
        translation_entries_from_generated_file(&out_path, RUST_UNITY_TRANSLATION_GENERATOR)
    {
        add_container_to_entries(&mut entries, container);
        return entries;
    }

    let asset_names = extracted_files_in_dir(cache_dir)
        .into_iter()
        .filter(|file| {
            !file.name.ends_with(".dll")
                && file.name != "mainData"
                && file.name != "manifest.json"
                && file.name != "cache-meta.json"
                && !file.name.ends_with("-translation.index.json")
        })
        .map(|file| file.name)
        .collect::<Vec<_>>();
    if asset_names.is_empty() {
        return Vec::new();
    }

    let mut entries = rust_unity_translation_entries(cache_dir, container, &asset_names);
    add_container_to_entries(&mut entries, container);
    let document = json!({
        "format": "fftools.translation.v1",
        "generatedBy": RUST_UNITY_TRANSLATION_GENERATOR,
        "entries": entries,
    });
    if let Ok(data) = serde_json::to_string_pretty(&document) {
        let _ = fs::write(&out_path, format!("{data}\n"));
    }
    entries
}

pub(in super::super) fn collect_managed_translation_entries(cache_dir: &Path) -> Vec<serde_json::Value> {
    if !has_managed_assemblies(cache_dir) {
        return Vec::new();
    }
    let out_path = cache_dir.join("managed-ui-translation.index.json");
    if let Some(entries) = translation_entries_from_generated_file(
        &out_path,
        fusionforge::managed::MANAGED_TRANSLATION_GENERATOR,
    ) {
        return entries;
    }
    let mut options = fusionforge::managed::ManagedExportOptions {
        assemblies: vec![
            "Assembly - CSharp.dll".to_string(),
            "Assembly - CSharp - first pass.dll".to_string(),
            "Assembly - UnityScript - first pass.dll".to_string(),
        ],
        ui_only: true,
        min_length: 2,
        ..Default::default()
    };
    if out_path.exists() {
        options.merge_paths.push(out_path.clone());
    }
    if let Err(err) = fusionforge::managed::export_translation_index(cache_dir, &out_path, &options)
    {
        eprintln!(
            "Managed string export failed for {}: {err}",
            cache_dir.display()
        );
        return translation_entries_from_file(&out_path).unwrap_or_default();
    }
    translation_entries_from_file(&out_path).unwrap_or_default()
}

pub(in super::super) fn collect_cached_translation_indexes(cache_dirs: &[PathBuf]) -> Vec<serde_json::Value> {
    let mut output = Vec::new();
    for cache_dir in cache_dirs {
        let Ok(entries) = fs::read_dir(cache_dir) else {
            continue;
        };
        let container = infer_cache_container(cache_dir);
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if !name.ends_with("-translation.index.json") {
                continue;
            }
            if let Some(mut entries) = translation_entries_from_file(&path) {
                sanitize_translation_entries(&mut entries);
                add_container_to_entries(&mut entries, &container);
                output.extend(entries);
            }
        }
    }
    output
}

pub(in super::super) fn deduplicate_translation_entries(entries: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    let mut seen = BTreeSet::<String>::new();
    let mut unique = Vec::new();
    for entry in entries {
        let key = translation_entry_key(&entry);
        if seen.insert(key) {
            unique.push(entry);
        }
    }
    unique
}

pub(in super::super) fn unity_translation_entry_key(entry: &JsonValue) -> Option<String> {
    let kind = entry.get("kind").and_then(JsonValue::as_str).unwrap_or("");
    if !matches!(kind, "unity.textAsset" | "unity.objectString") {
        return None;
    }
    let string_field = |name: &str| {
        entry
            .get(name)
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            .to_string()
    };
    let path_id = entry
        .get("pathId")
        .or_else(|| entry.get("path_id"))
        .and_then(JsonValue::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    Some(format!(
        "fields\0{}\0{}\0{}\0{}\0{}\0{}",
        kind,
        string_field("container"),
        string_field("file"),
        path_id,
        string_field("fieldPath"),
        string_field("source")
    ))
}

pub(in super::super) fn translation_entry_key(entry: &JsonValue) -> String {
    if let Some(key) = unity_translation_entry_key(entry) {
        return key;
    }
    if let Some(key) = managed_translation_entry_key(entry) {
        return key;
    }

    if let Some(id) = entry.get("id").and_then(JsonValue::as_str) {
        let container = entry
            .get("container")
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        return format!("id\0{container}\0{id}");
    }

    serde_json::to_string(entry).unwrap_or_default()
}

pub(in super::super) fn managed_translation_entry_key(entry: &JsonValue) -> Option<String> {
    let kind = entry.get("kind").and_then(JsonValue::as_str).unwrap_or("");
    if kind != "managed.ldstr" {
        return None;
    }
    let string_field = |name: &str| {
        entry
            .get(name)
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            .to_string()
    };
    let il_offset = entry
        .get("ilOffset")
        .or_else(|| entry.get("il_offset"))
        .and_then(JsonValue::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_default();
    Some(format!(
        "managed\0{}\0{}\0{}\0{}\0{}\0{}",
        string_field("container"),
        string_field("file"),
        string_field("type"),
        string_field("method"),
        il_offset,
        string_field("source")
    ))
}

pub(in super::super) fn translation_text(entry: &JsonValue) -> Option<&str> {
    entry
        .get("translation")
        .or_else(|| entry.get("Translation"))
        .and_then(JsonValue::as_str)
}

pub(in super::super) fn set_translation_text(entry: &mut JsonValue, translation: &str) {
    if let Some(object) = entry.as_object_mut() {
        object.insert("translation".to_string(), json!(translation));
        object.remove("Translation");
    }
}

pub(in super::super) fn merge_translation_entry(mut generated: JsonValue, existing: &JsonValue) -> JsonValue {
    if let Some(translation) = translation_text(existing).filter(|value| !value.is_empty()) {
        set_translation_text(&mut generated, translation);
    }
    generated
}

pub(in super::super) fn better_translation_entry(current: JsonValue, candidate: JsonValue) -> JsonValue {
    let current_has_translation = translation_text(&current)
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    let candidate_has_translation = translation_text(&candidate)
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    if candidate_has_translation && !current_has_translation {
        candidate
    } else {
        current
    }
}

pub(in super::super) fn load_project_translation_entries(project_path: &Path, index_path: &Path) -> Vec<JsonValue> {
    let container_aliases = cache_container_aliases(project_path);
    let mut by_key = BTreeMap::<String, JsonValue>::new();
    for path in [
        project_path.join("translation.json"),
        index_path.to_path_buf(),
    ] {
        if let Some(entries) = translation_entries_from_file(&path) {
            for mut entry in entries {
                normalize_entry_container(&mut entry, &container_aliases);
                let key = translation_entry_key(&entry);
                by_key
                    .entry(key)
                    .and_modify(|current| {
                        *current = better_translation_entry(current.clone(), entry.clone());
                    })
                    .or_insert(entry);
            }
        }
    }
    by_key.into_values().collect()
}

pub(in super::super) fn merge_translation_entries(
    existing_entries: Vec<JsonValue>,
    generated_entries: Vec<JsonValue>,
) -> Vec<JsonValue> {
    let source_translations = unique_source_translations(&existing_entries);
    let generated_by_key = generated_entries
        .into_iter()
        .map(|entry| (translation_entry_key(&entry), entry))
        .collect::<BTreeMap<_, _>>();
    let mut used_generated = BTreeSet::<String>::new();
    let mut seen = BTreeSet::<String>::new();
    let mut merged = Vec::<JsonValue>::new();

    for existing in existing_entries {
        let key = translation_entry_key(&existing);
        if !seen.insert(key.clone()) {
            continue;
        }
        if let Some(generated) = generated_by_key.get(&key) {
            used_generated.insert(key);
            merged.push(merge_translation_entry(generated.clone(), &existing));
        } else {
            merged.push(existing);
        }
    }

    for (key, mut generated) in generated_by_key {
        if used_generated.contains(&key) || !seen.insert(key) {
            continue;
        }
        if translation_text(&generated)
            .map(|value| value.is_empty())
            .unwrap_or(true)
        {
            if let Some(source) = generated.get("source").and_then(JsonValue::as_str) {
                if let Some(Some(translation)) = source_translations.get(source) {
                    set_translation_text(&mut generated, translation);
                }
            }
        }
        merged.push(generated);
    }

    merged
}

pub(in super::super) fn normalize_translation_entry(entry: &mut JsonValue) {
    let Some(translation) = translation_text(entry) else {
        return;
    };
    let Some(repaired) = fusionforge::managed::repair_cp1251_mojibake(translation) else {
        return;
    };
    set_translation_text(entry, &repaired);
}

pub(in super::super) fn normalize_translation_entries(entries: &mut [JsonValue]) {
    for entry in entries {
        normalize_translation_entry(entry);
    }
}

pub(in super::super) fn normalize_translation_document(document: &mut JsonValue) {
    if let Some(entries) = document
        .get_mut("entries")
        .and_then(JsonValue::as_array_mut)
    {
        sanitize_translation_entries(entries);
    }
}

pub(in super::super) fn write_translation_document(path: &Path, mut entries: Vec<JsonValue>) -> Result<(), String> {
    sanitize_translation_entries(&mut entries);
    let document = json!({
        "format": "fftools.translation.v1",
        "entries": entries,
    });
    let data = serde_json::to_string_pretty(&document).map_err(|err| err.to_string())?;
    fs::write(path, format!("{data}\n")).map_err(|err| format!("{}: {err}", path.display()))
}

pub(in super::super) fn project_from_translation_index_path(path: &Path) -> Option<PathBuf> {
    if path.file_name().and_then(|value| value.to_str()) != Some("translation.index.json") {
        return None;
    }
    let translations_dir = path.parent()?;
    if translations_dir
        .file_name()
        .and_then(|value| value.to_str())
        != Some("translations")
    {
        return None;
    }
    translations_dir.parent().map(Path::to_path_buf)
}

pub(in super::super) fn build_translation_index_for_bundles(
    project_path: &Path,
    bundles: &[ClientBundleFile],
) -> EditorResult<(PathBuf, usize)> {
    let translations_dir = project_path.join("translations");
    fs::create_dir_all(&translations_dir).map_err(|err| EditorError::Io(err).to_string())?;
    let translation_index_path = project_translation_index_path(project_path);
    let mut existing_entries =
        load_project_translation_entries(project_path, &translation_index_path);
    sanitize_translation_entries(&mut existing_entries);
    let cache_dirs = active_bundle_cache_dirs(bundles);
    for cache_dir in &cache_dirs {
        let container = infer_cache_container(cache_dir);
        let _ = collect_translation_entries(cache_dir, &container);
        if has_managed_assemblies(cache_dir) {
            let mut entries = collect_managed_translation_entries(cache_dir);
            add_container_to_entries(&mut entries, &container);
        }
    }
    let mut generated_entries =
        deduplicate_translation_entries(collect_cached_translation_indexes(&cache_dirs));
    generated_entries.extend(npc_import_translation_entries(project_path));
    generated_entries = deduplicate_translation_entries(generated_entries);
    sanitize_translation_entries(&mut generated_entries);
    let translation_entries = merge_translation_entries(existing_entries, generated_entries);
    let count = translation_entries.len();
    write_translation_document(&translation_index_path, translation_entries)?;
    Ok((translation_index_path, count))
}

pub(in super::super) fn build_translation_index(project_path: &Path) -> EditorResult<(PathBuf, usize)> {
    let Some(index) = load_project_client_index(project_path) else {
        return Err(format!(
            "Client bundle index is missing: {}",
            project_path
                .join("cache")
                .join("bundle-index.json")
                .display()
        ));
    };
    if !client_index_matches_project_source(project_path, &index) {
        return Err("Client bundle index does not match ffpatch.json Source.".to_string());
    }
    if !cached_bundle_payloads_are_current(&index) {
        return Err("Client bundle cache is stale for the indexed source build.".to_string());
    }
    build_translation_index_for_bundles(project_path, &index.bundles)
}

pub(in super::super) fn rebuild_translation_index(project_dir: String) -> EditorResult<serde_json::Value> {
    let project_path = PathBuf::from(&project_dir);
    if !project_path.exists() || !project_path.is_dir() {
        return Err(EditorError::MissingPath(project_dir).to_string());
    }
    let (path, count, reindexed) = match build_translation_index(&project_path) {
        Ok((path, count)) => (path, count, false),
        Err(err) => {
            let Some(source_path) = project_source_dir_from_patch_config(&project_path) else {
                return Err(err);
            };
            let index = index_client_project(
                source_path.to_string_lossy().to_string(),
                project_path.to_string_lossy().to_string(),
            )?;
            (
                PathBuf::from(index.translation_index_path),
                index.translation_count,
                true,
            )
        }
    };
    Ok(json!({
        "path": path,
        "count": count,
        "reindexed": reindexed,
    }))
}

pub(in super::super) fn gltf_clean_translation(value: [f32; 3]) -> (f64, f64, f64) {
    (
        -f64::from(value[0]),
        f64::from(value[2]),
        f64::from(value[1]),
    )
}

pub(in super::super) fn load_translation_document(path: String) -> EditorResult<serde_json::Value> {
    let data = fs::read_to_string(&path).map_err(|err| EditorError::Io(err).to_string())?;
    let mut document: serde_json::Value =
        serde_json::from_str(&data).map_err(|err| EditorError::Json(err).to_string())?;
    normalize_translation_document(&mut document);
    Ok(document)
}
