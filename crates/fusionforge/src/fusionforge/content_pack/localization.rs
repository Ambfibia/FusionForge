use super::*;

pub(super) const DEFAULT_LOCALE: &str = "ru-RU";

pub(super) fn process_localization_overlay(
    project: &Path,
    ffpatch: &JsonValue,
    locale: &str,
    state: &mut CookState,
    errors: &mut Vec<String>,
    mappings: &mut Vec<CookMapping>,
    roots: &ErrorRoots<'_>,
) -> Result<(), String> {
    let translation_rel = config_relative(
        ffpatch,
        "TranslationJson",
        "translations/translation.index.json",
        errors,
    )?;
    let path = project.join(&translation_rel);
    if !path.is_file() {
        return Ok(());
    }
    let manifest = read_json_value(&path).map_err(|err| roots.sanitize(&err))?;
    require_manifest_format(&manifest, "fftools.translation.v1", "translation")?;
    let raw_entries = require_entries(&manifest, "translation")?;
    struct LocalizationCandidate {
        source: String,
        value: String,
        descriptor: CookSourceDescriptor,
    }
    let mut candidates = BTreeMap::<String, LocalizationCandidate>::new();
    for entry in raw_entries {
        state.coverage.overlay_entries_scanned += 1;
        let source = entry
            .get("source")
            .or_else(|| entry.get("Source"))
            .or_else(|| entry.get("original"))
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "translation entry has no source".to_string())?;
        let raw_translation = entry
            .get("translation")
            .or_else(|| entry.get("Translation"))
            .or_else(|| entry.get("value"))
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "translation entry has no translation".to_string())?;
        let source = repair_cp1251_mojibake(source).unwrap_or_else(|| source.to_string());
        if source.is_empty() {
            return Err("translation entry has empty source".to_string());
        }
        let translation = if raw_translation.is_empty() {
            state.coverage.localization_fallbacks += 1;
            source.clone()
        } else {
            repair_cp1251_mojibake(raw_translation).unwrap_or_else(|| raw_translation.to_string())
        };
        if contains_legacy_text(&source) || contains_legacy_text(&translation) {
            return Err("translation entry contains a legacy locator".to_string());
        }
        let id = entry
            .get("id")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let fallback_context = || {
            [
                entry.get("kind").and_then(JsonValue::as_str).unwrap_or(""),
                entry
                    .get("objectName")
                    .and_then(JsonValue::as_str)
                    .unwrap_or(""),
                entry
                    .get("fieldPath")
                    .and_then(JsonValue::as_str)
                    .unwrap_or(""),
                &source,
            ]
            .join("\u{1f}")
        };
        let context = id.clone().unwrap_or_else(fallback_context);
        let key = stable_key("ffone.localization.v1", &[context.as_bytes()]);
        let descriptor = CookSourceDescriptor {
            file: Some(translation_rel.to_string_lossy().replace('\\', "/")),
            id,
            context: Some(context),
            ..CookSourceDescriptor::default()
        };
        if candidates
            .insert(
                key.clone(),
                LocalizationCandidate {
                    source,
                    value: translation,
                    descriptor,
                },
            )
            .is_some()
        {
            return Err(format!("duplicate localization semantic key: {key}"));
        }
    }
    let mut native_entries = Vec::with_capacity(candidates.len());
    for (key, candidate) in &candidates {
        native_entries.push(json!({
            "key": key,
            "source": candidate.source,
            "value": candidate.value,
        }));
    }
    if native_entries.is_empty() {
        return Err("translation manifest contains no entries".to_string());
    }
    let value = json!({
        "schema": "ffone.localization.v1",
        "locale": locale,
        "entries": native_entries,
    });
    let bytes = json_bytes(&value)?;
    let emitted = state.emit_asset(
        "localization",
        &format!("strings-{locale}"),
        "json",
        ContentKind::Localization,
        &bytes,
    )?;
    state.coverage.overlay_emitted += candidates.len() as u64;
    for (key, candidate) in candidates {
        mappings.push(CookMapping {
            kind: "localization".to_string(),
            name: candidate.source,
            native_key: key,
            native_path: emitted.path.clone(),
            source: candidate.descriptor,
        });
    }
    Ok(())
}

pub(super) fn validate_locale(locale: &str) -> Result<(), String> {
    let valid = !locale.is_empty()
        && locale.len() <= 35
        && locale.is_ascii()
        && locale.split('-').all(|part| {
            !part.is_empty()
                && part.len() <= 8
                && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        });
    if !valid {
        return Err(format!("invalid locale: {locale}"));
    }
    Ok(())
}
