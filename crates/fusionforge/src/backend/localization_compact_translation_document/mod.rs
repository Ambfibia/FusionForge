use super::super::*;

pub(in super::super) fn save_translation_document(path: String, mut document: serde_json::Value) -> EditorResult<()> {
    normalize_translation_document(&mut document);
    let data = serde_json::to_string_pretty(&document)
        .map_err(|err| EditorError::Json(err).to_string())?;
    let path = PathBuf::from(path);
    fs::write(&path, format!("{data}\n")).map_err(|err| EditorError::Io(err).to_string())?;
    if let Some(project) = project_from_translation_index_path(&path) {
        let mut patch_config = ensure_patch_project_config(&project)?;
        patch_config["TranslationJson"] = json!(PROJECT_TRANSLATION_INDEX_RELATIVE);
        let patch_data = serde_json::to_string_pretty(&patch_config)
            .map_err(|err| EditorError::Json(err).to_string())?;
        fs::write(project.join("ffpatch.json"), format!("{patch_data}\n"))
            .map_err(|err| EditorError::Io(err).to_string())?;
        let legacy_translation_path = project.join("translation.json");
        if legacy_translation_path.exists() {
            fs::remove_file(&legacy_translation_path)
                .map_err(|err| EditorError::Io(err).to_string())?;
        }
    }
    Ok(())
}

pub(in super::super) fn translation_source_text(entry: &JsonValue) -> Option<&str> {
    entry
        .get("source")
        .or_else(|| entry.get("Source"))
        .or_else(|| entry.get("original"))
        .or_else(|| entry.get("Original"))
        .and_then(JsonValue::as_str)
}

pub(in super::super) fn translation_value_text(entry: &JsonValue) -> Option<&str> {
    entry
        .get("translation")
        .or_else(|| entry.get("Translation"))
        .and_then(JsonValue::as_str)
}

pub(in super::super) fn normalized_translation_compare_text(value: &str) -> String {
    fusionforge::managed::repair_cp1251_mojibake(value).unwrap_or_else(|| value.to_string())
}

pub(in super::super) fn compact_translation_document(document: &JsonValue) -> EditorResult<JsonValue> {
    let entries = document
        .get("entries")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "Translation document has no entries array.".to_string())?;
    let translated_sources = entries
        .iter()
        .filter_map(translation_value_text)
        .filter(|value| !value.is_empty())
        .map(normalized_translation_compare_text)
        .collect::<BTreeSet<_>>();
    let mut by_original = BTreeMap::<String, Option<String>>::new();
    for entry in entries {
        let Some(original) = translation_source_text(entry).filter(|value| !value.is_empty())
        else {
            continue;
        };
        let translation = translation_value_text(entry)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if translation.is_none()
            && (is_empty_translated_source(original)
                || translated_sources.contains(&normalized_translation_compare_text(original)))
        {
            continue;
        }
        by_original
            .entry(original.to_string())
            .and_modify(|current| {
                if current.as_ref() != translation.as_ref() {
                    *current = None;
                }
            })
            .or_insert(translation);
    }
    let entries = by_original
        .into_iter()
        .map(|(original, translation)| {
            json!({
                "original": original,
                "translation": translation.unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "format": "fftools.translation.compact.v1",
        "entries": entries,
    }))
}

pub(in super::super) fn export_compact_translation_document(
    path: String,
    mut document: JsonValue,
) -> EditorResult<JsonValue> {
    normalize_translation_document(&mut document);
    let compact = compact_translation_document(&document)?;
    let count = compact
        .get("entries")
        .and_then(JsonValue::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let data =
        serde_json::to_string_pretty(&compact).map_err(|err| EditorError::Json(err).to_string())?;
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    fs::write(&path, format!("{data}\n")).map_err(|err| EditorError::Io(err).to_string())?;
    Ok(json!({
        "path": path,
        "count": count,
    }))
}

pub(in super::super) fn compact_translation_entries(document: &JsonValue) -> EditorResult<&Vec<JsonValue>> {
    if let Some(entries) = document.as_array() {
        return Ok(entries);
    }
    document
        .get("entries")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "Compact translation document has no entries array.".to_string())
}

pub(in super::super) fn load_compact_translation_document(
    path: String,
    mut base_document: JsonValue,
) -> EditorResult<JsonValue> {
    normalize_translation_document(&mut base_document);
    let data = fs::read_to_string(&path).map_err(|err| EditorError::Io(err).to_string())?;
    let compact: JsonValue =
        serde_json::from_str(&data).map_err(|err| EditorError::Json(err).to_string())?;
    let compact_entries = compact_translation_entries(&compact)?;
    let mut translations = BTreeMap::<String, String>::new();
    for entry in compact_entries {
        let Some(original) = translation_source_text(entry).filter(|value| !value.is_empty())
        else {
            continue;
        };
        let translation = translation_value_text(entry).unwrap_or_default();
        translations.insert(original.to_string(), translation.to_string());
    }
    let entries = base_document
        .get_mut("entries")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| "Translation document has no entries array.".to_string())?;
    let mut updated = 0usize;
    for entry in entries {
        let Some(original) = translation_source_text(entry).map(str::to_string) else {
            continue;
        };
        let Some(translation) = translations.get(&original) else {
            continue;
        };
        if entry.get("translation").and_then(JsonValue::as_str) != Some(translation.as_str()) {
            set_translation_text(entry, translation);
            updated += 1;
        }
    }
    normalize_translation_document(&mut base_document);
    Ok(json!({
        "path": path,
        "entries": compact_entries.len(),
        "updated": updated,
        "document": base_document,
    }))
}
