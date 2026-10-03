use super::*;

pub(super) const TEXT_ASSET_KIND: &str = "unity.textAsset";

pub(super) const FONT_MANIFEST_FORMAT: &str = "fftools.font-patch.v1";

pub fn patch_asset_loader_downloads(args: &[String]) -> Result<(), String> {
    let usage = "patch-asset-loader-downloads <extracted-unityweb-dir> <bundle-name>...";
    let root = required_path(args, 0, usage)?;
    if args.len() < 2 {
        return Err(usage.to_string());
    }
    let count = managed::patch_asset_loader_downloads(&root, &args[1..])?;
    println!("Added {count} AssetLoader download entries.");
    Ok(())
}

pub fn patch_unity_asset_strings(args: &[String]) -> Result<(), String> {
    let usage = "patch-unity-asset-strings <extracted-dir> <translation-json> [--container name] [--allow-missing] [--status-file path]";
    let root = required_path(args, 0, usage)?;
    let translation_json = required_path(args, 1, usage)?;
    let container = option_value(args, "--container").unwrap_or_default();
    let allow_missing = has_flag(args, "--allow-missing");
    let status_file = option_value(args, "--status-file").map(PathBuf::from);
    let spec = read_json(&translation_json)?;
    let mut by_file: BTreeMap<String, Vec<JsonValue>> = BTreeMap::new();
    let mut skipped_unsupported = 0usize;
    for entry in spec
        .get("entries")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        let kind = entry.get("kind").and_then(JsonValue::as_str).unwrap_or("");
        if kind != TEXT_ASSET_KIND && kind != OBJECT_STRING_KIND {
            continue;
        }
        if kind == OBJECT_STRING_KIND && is_unsupported_object_string_entry(entry) {
            skipped_unsupported += 1;
            continue;
        }
        if !container.is_empty()
            && entry
                .get("container")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                != container
        {
            continue;
        }
        if !has_text(entry.get("translation")) {
            continue;
        }
        let Some(file) = entry.get("file").and_then(JsonValue::as_str) else {
            continue;
        };
        by_file
            .entry(file.replace('\\', "/"))
            .or_default()
            .push(entry.clone());
    }
    if by_file.is_empty() {
        println!("No filled Unity asset translations to apply.");
        write_status(
            status_file.as_deref(),
            json!({ "applied": 0, "skippedUnsupported": skipped_unsupported }),
        )?;
        return Ok(());
    }

    let mut total = 0usize;
    for (file_name, entries) in by_file {
        let path = root.join(&file_name);
        if !path.exists() {
            if allow_missing {
                continue;
            }
            return Err(format!("{} was not found", path.display()));
        }
        let asset = Asset::from_path(&path)?;
        let asset_index = 0usize;
        let mut replacements = BTreeMap::new();
        let mut entries_by_path_id = BTreeMap::<i64, Vec<JsonValue>>::new();
        for entry in entries {
            let path_id = entry_i64(&entry, "pathId")?;
            entries_by_path_id.entry(path_id).or_default().push(entry);
        }
        for (path_id, entries) in entries_by_path_id {
            let Some(info) = asset.objects.get(&path_id) else {
                if allow_missing {
                    continue;
                }
                return Err(format!("{file_name}: pathId {path_id} was not found"));
            };
            let mut value = asset.read_object(asset_index, info)?;
            let mut object_patched = false;
            for entry in entries {
                let kind = entry.get("kind").and_then(JsonValue::as_str).unwrap_or("");
                let source = entry
                    .get("source")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("");
                let translation = entry
                    .get("translation")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
                    .to_string();
                let patched = if kind == TEXT_ASSET_KIND {
                    patch_text_asset_value(&mut value, source, translation, allow_missing)?
                } else {
                    let parts = entry_path_parts(&entry)
                        .ok_or_else(|| format!("{file_name}: object string entry has no path"))?;
                    patch_value_path(&mut value, &parts, source, translation, allow_missing)?
                };
                if patched {
                    object_patched = true;
                    total += 1;
                }
            }
            if object_patched {
                replacements.insert(path_id, value);
            }
        }
        if !replacements.is_empty() {
            let bytes = asset.rebuild_with_object_values(asset_index, &replacements)?;
            fs::write(&path, bytes).map_err(|err| format!("{}: {err}", path.display()))?;
        }
    }
    println!("Applied {total} Unity asset translations.");
    write_status(
        status_file.as_deref(),
        json!({ "applied": total, "skippedUnsupported": skipped_unsupported }),
    )?;
    Ok(())
}

pub(super) fn patch_text_asset_value(
    value: &mut UnityValue,
    source: &str,
    translation: String,
    allow_missing: bool,
) -> Result<bool, String> {
    if let Some(script) = value.get_mut("m_Script") {
        if script.as_str() == Some(source) {
            *script = UnityValue::String(translation);
            return Ok(true);
        }
    }
    if allow_missing {
        Ok(false)
    } else {
        Err("TextAsset source text does not match".to_string())
    }
}

pub(super) fn patch_value_path(
    value: &mut UnityValue,
    parts: &[JsonValue],
    source: &str,
    translation: String,
    allow_missing: bool,
) -> Result<bool, String> {
    let Some(target) = value_by_json_path_mut(value, parts) else {
        if allow_missing {
            return Ok(false);
        }
        return Err("object string path was not found".to_string());
    };
    if target.as_str() != Some(source) {
        if allow_missing {
            return Ok(false);
        }
        return Err("object string source text does not match".to_string());
    }
    *target = UnityValue::String(translation);
    Ok(true)
}

pub(super) fn is_unsupported_object_string_field_path(field_path: &str) -> bool {
    (field_path.contains("GUILanguageData") && field_path.contains(".m_String["))
        || (field_path.starts_with("m_pNpcTable.m_pNpcStringData[")
            && field_path.ends_with(".m_strComment2"))
        || ((field_path.starts_with("m_pFilterTable.m_pWhiteFilterData[")
            || field_path.starts_with("m_pFilterTable.m_pBlackFilterData[")
            || field_path.starts_with("m_pFilterTable.m_pNameFilterData["))
            && field_path.ends_with(".m_strText"))
        || (field_path.starts_with("m_pHelpTable.m_pHelpPageString[")
            && (field_path.ends_with(".m_strComment")
                || field_path.ends_with(".m_strComment1")
                || field_path.ends_with(".m_strComment2")))
        || (field_path.starts_with("m_pFirstUseTable.m_pFirstUseString[")
            && (field_path.ends_with(".m_strComment1") || field_path.ends_with(".m_strComment2")))
        || (field_path.starts_with("m_pRulesTable.m_pRulesString[")
            && (field_path.ends_with(".m_strComment")
                || field_path.ends_with(".m_strComment1")
                || field_path.ends_with(".m_strComment2")))
}

pub(super) fn is_unsupported_object_string_path_parts(parts: &[JsonValue]) -> bool {
    if parts.len() < 4 {
        return false;
    }
    if parts.last().and_then(JsonValue::as_u64).is_none() {
        return false;
    }
    let Some(string_field) = parts.get(parts.len() - 2).and_then(JsonValue::as_str) else {
        return false;
    };
    if string_field != "m_String" {
        return false;
    }
    parts
        .iter()
        .any(|part| part.as_str() == Some("m_pGUILanguageData"))
}

pub(super) fn entry_path_parts(entry: &JsonValue) -> Option<Vec<JsonValue>> {
    if let Some(parts) = entry.get("path").and_then(JsonValue::as_array) {
        return Some(parts.clone());
    }
    entry
        .get("fieldPath")
        .and_then(JsonValue::as_str)
        .and_then(parse_field_path)
}

pub(super) fn parse_field_path(path: &str) -> Option<Vec<JsonValue>> {
    let mut parts = Vec::new();
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        let mut rest = segment;
        if let Some(bracket) = rest.find('[') {
            if bracket > 0 {
                parts.push(JsonValue::String(rest[..bracket].to_string()));
            }
            rest = &rest[bracket..];
        } else {
            parts.push(JsonValue::String(rest.to_string()));
            continue;
        }

        while !rest.is_empty() {
            if !rest.starts_with('[') {
                return None;
            }
            let end = rest.find(']')?;
            let index = rest[1..end].parse::<u64>().ok()?;
            parts.push(json!(index));
            rest = &rest[end + 1..];
        }
    }
    Some(parts)
}

pub(super) fn value_by_json_path_mut<'a>(
    value: &'a mut UnityValue,
    parts: &[JsonValue],
) -> Option<&'a mut UnityValue> {
    let mut current = value;
    for part in parts {
        if let Some(key) = part.as_str() {
            current = current.as_object_mut()?.get_mut(key)?;
        } else if let Some(index) = part.as_u64() {
            current = current.as_array_mut()?.get_mut(index as usize)?;
        } else {
            return None;
        }
    }
    Some(current)
}

pub(super) fn font_manifest_entries(
    json: &JsonValue,
    base: Option<&Path>,
) -> Result<HashMap<String, JsonValue>, String> {
    let base = base.unwrap_or(Path::new("."));
    let mut result = HashMap::new();
    for entry in json
        .get("entries")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        let family = normalize_font_family(
            entry
                .get("family")
                .and_then(JsonValue::as_str)
                .unwrap_or(""),
        );
        if family.is_empty() {
            continue;
        }
        let mut entry = entry.clone();
        if let Some(font_file) = entry.get("fontFile").and_then(JsonValue::as_str) {
            let path = PathBuf::from(font_file);
            let path = if path.is_absolute() {
                path
            } else {
                let candidate = base.join(&path);
                if candidate.exists() {
                    candidate
                } else {
                    base.parent()
                        .map(|parent| parent.join(&path))
                        .unwrap_or(candidate)
                }
            };
            if path.exists() {
                entry.as_object_mut().unwrap().insert(
                    "resolvedFontPath".to_string(),
                    JsonValue::String(path.to_string_lossy().to_string()),
                );
            }
        }
        result.insert(family, entry);
    }
    Ok(result)
}

pub(super) fn required_path(args: &[String], index: usize, usage: &str) -> Result<PathBuf, String> {
    args.get(index)
        .filter(|value| !value.starts_with("--"))
        .map(PathBuf::from)
        .ok_or_else(|| usage.to_string())
}
