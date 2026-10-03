use super::*;

// Historical overlay conformance fixtures still exercise the shared converters.
#[cfg(test)]
pub(super) fn cook_ffone_pack(
    project: &Path,
    source: &Path,
    output: &Path,
    locale: &str,
) -> Result<PathBuf, String> {
    cook_pack(Some(project), source, output, locale)
}

pub(super) fn cook_pack(
    overlay_project: Option<&Path>,
    manifest_or_dir: &Path,
    output_dir: &Path,
    locale: &str,
) -> Result<PathBuf, String> {
    validate_locale(locale)?;
    let manifest_path = resolve_launcher_manifest(manifest_or_dir)?;
    let manifest_bytes = read_regular_file(&manifest_path)?;
    let launcher: LauncherManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    validate_uuid(&launcher.uuid)?;
    let build_dir = manifest_path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", manifest_path.display()))?
        .to_path_buf();
    let project_dir = match overlay_project {
        Some(project) => canonical_directory(project, "overlay directory")?,
        None => build_dir.clone(),
    };
    let ffpatch = match overlay_project {
        Some(_) => {
            let path = project_dir.join("ffpatch.json");
            parse_json(&path, &read_regular_file(&path)?)?
        }
        None => json!({}),
    };
    let inventory = build_inventory(&build_dir, &launcher)?;
    let build_fingerprint = build_inventory_fingerprint(&inventory);
    let overlay_fingerprint = if overlay_project.is_some() {
        authoring_overlay_fingerprint(&project_dir, &ffpatch)?
    } else {
        blake3_hex(b"fusionforge.raw-source.no-overlays.v1")
    };
    let inventory_report = inventory
        .iter()
        .map(|entry| InventoryReportEntry {
            file: entry.name.clone(),
            size: entry.expected.size,
            sha256: entry.expected.hash.to_ascii_lowercase(),
        })
        .collect::<Vec<_>>();

    if output_dir.exists() {
        return Err(format!(
            "output path already exists: {}",
            output_dir.display()
        ));
    }
    let output_parent = output_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(output_parent)
        .map_err(|err| format!("{}: {err}", output_parent.display()))?;
    let output_parent = canonical_directory(output_parent, "output parent")?;
    let output_name = output_dir
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("invalid output directory name: {}", output_dir.display()))?;
    let output = output_parent.join(output_name);
    let staging = output_parent.join(format!(".{output_name}.ffone-staging"));
    let extraction = output_parent.join(format!(".{output_name}.ffone-extract"));
    let report_path = cook_report_path(&output)?;
    let report_temp = output_parent.join(format!(".{output_name}.cook-report.json.tmp"));
    for path in [&output, &staging, &extraction, &report_path, &report_temp] {
        if path.exists() {
            return Err(format!(
                "refusing to replace existing cooker output: {}",
                path.display()
            ));
        }
    }

    fs::create_dir(&staging).map_err(|err| format!("{}: {err}", staging.display()))?;
    if let Err(err) = fs::create_dir(&extraction) {
        cleanup_created(&staging);
        return Err(format!("{}: {err}", extraction.display()));
    }

    let roots = ErrorRoots {
        project: &project_dir,
        build: &build_dir,
        work: &extraction,
    };
    let cook_result = (|| {
        let provenance = Provenance::new(
            "ffone-native-cooker",
            vec![
                SourceFingerprint::new("authoring-overlay", overlay_fingerprint.clone()),
                SourceFingerprint::new("build-inventory", build_fingerprint.clone()),
            ],
        );
        let mut state = CookState::new(staging.clone(), locale, provenance);
        let mut report = CookReport {
            schema: COOK_REPORT_SCHEMA,
            profile: "core-v1",
            complete: false,
            native_only: true,
            build_uuid: launcher.uuid.clone(),
            locale: locale.to_string(),
            inventory: inventory_report,
            bundles: Vec::with_capacity(inventory.len()),
            overlay_errors: Vec::new(),
            overlay_warnings: Vec::new(),
            mappings: Vec::new(),
            coverage: CoverageCounters::default(),
            counts: BTreeMap::new(),
        };

        for (index, entry) in inventory.iter().enumerate() {
            verify_inventory_entry(entry)?;
            let mut bundle_report = BundleCookReport {
                bundle: entry.name.clone(),
                ..BundleCookReport::default()
            };
            process_bundle(
                index,
                entry,
                &extraction,
                &mut state,
                &mut bundle_report,
                &mut report.mappings,
                &roots,
            )?;
            report.bundles.push(bundle_report);
        }

        if overlay_project.is_some() {
            process_overlays(
                &project_dir,
                &ffpatch,
                locale,
                &mut state,
                &mut report.overlay_errors,
                &mut report.overlay_warnings,
                &mut report.mappings,
                &roots,
            )?;
        }
        state.emit_font_index()?;
        let error_count = report
            .bundles
            .iter()
            .map(|bundle| bundle.errors.len() as u64)
            .sum::<u64>()
            + report.overlay_errors.len() as u64;
        state.coverage.overlay_skipped = state
            .coverage
            .overlay_entries_scanned
            .saturating_sub(state.coverage.overlay_emitted);
        state.coverage.errors = error_count;
        state.emit_catalog()?;
        state
            .builder
            .write(&staging)
            .map_err(|err| format!("could not write native pack manifest: {err}"))?;
        report.mappings.sort_by(|left, right| {
            (&left.kind, &left.native_key, &left.native_path, &left.name).cmp(&(
                &right.kind,
                &right.native_key,
                &right.native_path,
                &right.name,
            ))
        });
        report.coverage = state.coverage.clone();
        report.counts = state.counts.clone();
        let report_bytes =
            pretty_json_bytes(&serde_json::to_value(&report).map_err(|err| err.to_string())?)?;
        fs::write(&report_temp, report_bytes)
            .map_err(|err| format!("{}: {err}", report_temp.display()))?;
        Ok::<(), String>(())
    })();

    if let Err(err) = cook_result {
        cleanup_created(&staging);
        cleanup_created(&extraction);
        cleanup_created(&report_temp);
        return Err(err);
    }

    if let Err(err) = fs::remove_dir_all(&extraction) {
        cleanup_created(&staging);
        cleanup_created(&report_temp);
        return Err(format!("could not remove {}: {err}", extraction.display()));
    }
    if let Err(err) = fs::rename(&staging, &output) {
        cleanup_created(&staging);
        cleanup_created(&report_temp);
        return Err(format!("could not publish {}: {err}", output.display()));
    }
    if let Err(err) = fs::rename(&report_temp, &report_path) {
        cleanup_created(&output);
        cleanup_created(&report_temp);
        return Err(format!(
            "could not publish {}: {err}",
            report_path.display()
        ));
    }
    Ok(output)
}

pub(super) fn cook_font(
    body: &UnityValue,
    name: &str,
    state: &mut CookState,
) -> Result<EmittedAsset, ObjectCookError> {
    let bytes = body
        .get("m_FontData")
        .and_then(UnityValue::as_bytes)
        .ok_or_else(|| ObjectCookError::Object("Font has no embedded font bytes".to_string()))?;
    let extension = font_extension(bytes).ok_or_else(|| {
        ObjectCookError::Object("Font payload is not recognized TTF or OTF".to_string())
    })?;
    state
        .emit_asset(
            "fonts",
            semantic_or(name, "font"),
            extension,
            ContentKind::Font,
            bytes,
        )
        .map_err(ObjectCookError::Output)
}

pub(super) fn process_overlays(
    project: &Path,
    ffpatch: &JsonValue,
    locale: &str,
    state: &mut CookState,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
    mappings: &mut Vec<CookMapping>,
    roots: &ErrorRoots<'_>,
) -> Result<(), String> {
    process_audio_overlay(project, ffpatch, state, errors, warnings, mappings, roots)?;
    process_font_overlay(project, ffpatch, state, errors, mappings, roots)?;
    process_localization_overlay(project, ffpatch, locale, state, errors, mappings, roots)?;
    process_table_overlay(project, ffpatch, state, errors, mappings, roots)?;
    Ok(())
}

pub(super) fn process_font_overlay(
    project: &Path,
    ffpatch: &JsonValue,
    state: &mut CookState,
    errors: &mut Vec<String>,
    mappings: &mut Vec<CookMapping>,
    roots: &ErrorRoots<'_>,
) -> Result<(), String> {
    let font_rel = config_relative(ffpatch, "FontDir", "fonts", errors)?;
    let font_root = project.join(&font_rel);
    let manifest_path = font_root.join("manifest.json");
    if !manifest_path.is_file() {
        return Ok(());
    }
    let manifest = read_json_value(&manifest_path).map_err(|err| roots.sanitize(&err))?;
    require_manifest_format(&manifest, "fftools.font-patch.v1", "font")?;
    let entries = require_entries(&manifest, "font")?;
    let mut seen_files = BTreeSet::new();
    let mut seen_semantics = BTreeSet::new();
    for entry in entries {
        state.coverage.overlay_entries_scanned += 1;
        let relative = entry
            .get("fontFile")
            .or_else(|| entry.get("file"))
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "font manifest entry has no fontFile".to_string())?;
        let family = required_string(entry, "family", "font entry")?;
        let relative_normalized = relative.replace('\\', "/");
        if !seen_files.insert(relative_normalized.clone()) {
            return Err(format!("duplicate font manifest file: {relative}"));
        }
        let path = resolve_overlay_file(project, &font_root, relative)
            .map_err(|err| roots.sanitize(&err))?;
        let bytes = read_regular_file(&path).map_err(|err| roots.sanitize(&err))?;
        let extension = font_extension(&bytes)
            .ok_or_else(|| format!("font entry {relative} is not recognized TTF or OTF"))?;
        let semantic_identity = native_asset_identity(ContentKind::Font, family, &bytes).key;
        if !seen_semantics.insert(semantic_identity) {
            return Err(format!(
                "duplicate font semantic entry: {family}/{relative}"
            ));
        }
        let include_russian = entry
            .get("includeRussian")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        let replace_ascii = entry
            .get("replaceAscii")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        let vertical_offset = entry
            .get("verticalOffset")
            .and_then(JsonValue::as_f64)
            .unwrap_or(0.0);
        if !vertical_offset.is_finite() {
            return Err(format!(
                "font entry {relative} has non-finite verticalOffset"
            ));
        }
        let target = entry.get("target").and_then(JsonValue::as_object);
        let target_name = target
            .and_then(|value| value.get("name"))
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty());
        let emitted = state.emit_asset("fonts", family, extension, ContentKind::Font, &bytes)?;
        state.register_font_metadata(
            family,
            &bytes,
            include_russian,
            replace_ascii,
            vertical_offset,
            target_name,
        );
        state.coverage.overlay_emitted += 1;
        mappings.push(CookMapping {
            kind: "font".to_string(),
            name: emitted.name,
            native_key: emitted.key,
            native_path: emitted.path,
            source: CookSourceDescriptor {
                file: Some(relative_normalized),
                path_id: target
                    .and_then(|value| value.get("pathId"))
                    .and_then(JsonValue::as_i64),
                context: Some(target_name.unwrap_or(family).to_string()),
                asset: target
                    .and_then(|value| value.get("asset"))
                    .and_then(JsonValue::as_str)
                    .map(str::to_string),
                ..CookSourceDescriptor::default()
            },
        });
    }
    Ok(())
}

pub(super) fn process_table_overlay(
    project: &Path,
    ffpatch: &JsonValue,
    state: &mut CookState,
    errors: &mut Vec<String>,
    mappings: &mut Vec<CookMapping>,
    roots: &ErrorRoots<'_>,
) -> Result<(), String> {
    let table_rel = config_relative(ffpatch, "TableDataDir", "tabledata", errors)?;
    let table_root = project.join(&table_rel);
    let manifest_path = table_root.join("manifest.json");
    if !manifest_path.is_file() {
        return Ok(());
    }
    let manifest = read_json_value(&manifest_path).map_err(|err| roots.sanitize(&err))?;
    require_manifest_format(&manifest, "fftools.tabledata-patch.v1", "tabledata")?;
    let entries = require_entries(&manifest, "tabledata")?;
    let mut tables = BTreeMap::<String, JsonValue>::new();
    let mut descriptors = BTreeMap::<String, (String, CookSourceDescriptor)>::new();
    let mut seen_files = BTreeSet::new();
    for entry in entries {
        state.coverage.overlay_entries_scanned += 1;
        let relative = required_string(entry, "file", "tabledata entry")?;
        let relative_normalized = relative.replace('\\', "/");
        if !seen_files.insert(relative_normalized.clone()) {
            return Err(format!("duplicate tabledata manifest file: {relative}"));
        }
        let path = resolve_overlay_file(project, &table_root, relative)
            .map_err(|err| roots.sanitize(&err))?;
        let patch = read_json_value(&path).map_err(|err| roots.sanitize(&err))?;
        require_manifest_format(&patch, "fftools.tabledata-object.v1", "tabledata object")?;
        let raw_value = patch
            .get("value")
            .ok_or_else(|| format!("tabledata patch {relative} has no value"))?;
        let cleaned = clean_table_value(raw_value, true)
            .ok_or_else(|| format!("tabledata patch {relative} has no native value"))?;
        let name = entry
            .get("name")
            .and_then(JsonValue::as_str)
            .or_else(|| patch.get("name").and_then(JsonValue::as_str))
            .unwrap_or("table");
        let semantic = neutral_semantic_label(name, "table");
        let cleaned_bytes = json_bytes(&cleaned)?;
        let key = stable_key("ffone.table.v1", &[semantic.as_bytes(), &cleaned_bytes]);
        if tables
            .insert(
                key.clone(),
                json!({ "key": key, "name": semantic, "value": cleaned }),
            )
            .is_some()
        {
            return Err(format!("duplicate tabledata semantic key: {key}"));
        }
        descriptors.insert(
            key,
            (
                semantic,
                CookSourceDescriptor {
                    bundle: entry
                        .get("container")
                        .and_then(JsonValue::as_str)
                        .map(str::to_string),
                    file: Some(relative_normalized),
                    path_id: entry
                        .get("pathId")
                        .or_else(|| entry.get("path_id"))
                        .and_then(JsonValue::as_i64),
                    ..CookSourceDescriptor::default()
                },
            ),
        );
    }
    if tables.is_empty() {
        return Err("tabledata manifest contains no entries".to_string());
    }
    let value = json!({
        "schema": "ffone.table-set.v1",
        "tables": tables.into_values().collect::<Vec<_>>(),
    });
    let bytes = json_bytes(&value)?;
    let emitted = state.emit_asset("tables", "table-set", "json", ContentKind::Table, &bytes)?;
    state.coverage.overlay_emitted += descriptors.len() as u64;
    for (key, (name, source)) in descriptors {
        mappings.push(CookMapping {
            kind: "table".to_string(),
            name,
            native_key: key,
            native_path: emitted.path.clone(),
            source,
        });
    }
    Ok(())
}

pub(super) fn clean_table_value(value: &JsonValue, root: bool) -> Option<JsonValue> {
    match value {
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => Some(value.clone()),
        JsonValue::String(text) => (!contains_legacy_text(text)).then(|| value.clone()),
        JsonValue::Array(values) => Some(JsonValue::Array(
            values
                .iter()
                .map(|value| clean_table_value(value, false).unwrap_or(JsonValue::Null))
                .collect(),
        )),
        JsonValue::Object(object) => {
            if object
                .get("__unityType")
                .and_then(JsonValue::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case("pointer"))
            {
                return None;
            }
            let mut cleaned = JsonMap::new();
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if forbidden_table_key(key, root) {
                    continue;
                }
                if let Some(value) = clean_table_value(&object[key], false) {
                    cleaned.insert(key.clone(), value);
                }
            }
            Some(JsonValue::Object(cleaned))
        }
    }
}

pub(super) fn forbidden_table_key(key: &str, root: bool) -> bool {
    let normalized = key
        .chars()
        .filter(|value| value.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if normalized.starts_with("unity")
        || matches!(
            normalized.as_str(),
            "pathid" | "fileid" | "assetindex" | "sourceasset" | "sourcebundle"
        )
    {
        return true;
    }
    matches!(
        normalized.as_str(),
        "mobjecthideflags" | "meditorhideflags" | "mgameobject" | "mscript" | "mextensionptr"
    ) || (root && matches!(normalized.as_str(), "mname" | "menabled"))
}

pub(super) fn authoring_overlay_fingerprint(project: &Path, ffpatch: &JsonValue) -> Result<String, String> {
    let mut records = BTreeMap::<String, Vec<u8>>::new();

    let audio_rel = configured_relative(ffpatch, "AudioDir", "audio")?;
    let audio_root = project.join(&audio_rel);
    let audio_manifest = audio_root.join("manifest.json");
    if let Some(path) = declared_overlay_manifest(
        &audio_manifest,
        ffpatch.get("AudioDir").is_some() || audio_root.is_dir(),
        "audio",
    )? {
        let manifest = read_json_value(&path)?;
        require_manifest_format(&manifest, "fftools.audio-patch.v1", "audio")?;
        for entry in require_entries(&manifest, "audio")? {
            let relative = required_string(entry, "file", "audio entry")?;
            let source = resolve_overlay_file(project, &audio_root, relative)?;
            let bytes = read_regular_file(&source)?;
            let declared_name = entry
                .get("name")
                .and_then(JsonValue::as_str)
                .unwrap_or_default();
            let name = if declared_name.trim().is_empty() {
                Path::new(relative)
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .and_then(|value| value.split_once("__").map(|(_, name)| name).or(Some(value)))
                    .unwrap_or("audio")
            } else {
                declared_name
            };
            let descriptor = json!({
                "name": neutral_semantic_label(name, "audio"),
                "container": entry.get("container").and_then(JsonValue::as_str).unwrap_or_default(),
                "asset": entry.get("asset").and_then(JsonValue::as_str).unwrap_or_default(),
                "pathId": entry.get("path_id").or_else(|| entry.get("pathId")).and_then(JsonValue::as_i64),
                "payloadBytes": bytes.len(),
                "payloadBlake3": blake3_hex(&bytes),
            });
            insert_fingerprint_record(
                &mut records,
                format!("audio/{}", relative.replace('\\', "/")),
                json_bytes(&descriptor)?,
            )?;
        }
    }

    let font_rel = configured_relative(ffpatch, "FontDir", "fonts")?;
    let font_root = project.join(&font_rel);
    let font_manifest = font_root.join("manifest.json");
    if let Some(path) = declared_overlay_manifest(
        &font_manifest,
        ffpatch.get("FontDir").is_some() || font_root.is_dir(),
        "font",
    )? {
        let manifest = read_json_value(&path)?;
        require_manifest_format(&manifest, "fftools.font-patch.v1", "font")?;
        for entry in require_entries(&manifest, "font")? {
            let relative = entry
                .get("fontFile")
                .or_else(|| entry.get("file"))
                .and_then(JsonValue::as_str)
                .ok_or_else(|| "font manifest entry has no fontFile".to_string())?;
            let source = resolve_overlay_file(project, &font_root, relative)?;
            let bytes = read_regular_file(&source)?;
            let family = required_string(entry, "family", "font entry")?;
            let target_name = entry
                .get("target")
                .and_then(JsonValue::as_object)
                .and_then(|target| target.get("name"))
                .and_then(JsonValue::as_str)
                .filter(|value| !value.is_empty());
            let descriptor = json!({
                "family": neutral_semantic_label(family, "font"),
                "includeRussian": entry.get("includeRussian").and_then(JsonValue::as_bool).unwrap_or(false),
                "replaceAscii": entry.get("replaceAscii").and_then(JsonValue::as_bool).unwrap_or(false),
                "verticalOffset": entry.get("verticalOffset").and_then(JsonValue::as_f64).unwrap_or(0.0),
                "target": target_name.map(|value| neutral_semantic_label(value, "font-target")),
                "payloadBytes": bytes.len(),
                "payloadBlake3": blake3_hex(&bytes),
            });
            insert_fingerprint_record(
                &mut records,
                format!("font/{}", relative.replace('\\', "/")),
                json_bytes(&descriptor)?,
            )?;
        }
    }

    let translation_rel = configured_relative(
        ffpatch,
        "TranslationJson",
        "translations/translation.index.json",
    )?;
    let translation = project.join(&translation_rel);
    if let Some(path) = declared_overlay_manifest(
        &translation,
        ffpatch.get("TranslationJson").is_some() || translation.is_file(),
        "translation",
    )? {
        let manifest = read_json_value(&path)?;
        require_manifest_format(&manifest, "fftools.translation.v1", "translation")?;
        for entry in require_entries(&manifest, "translation")? {
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
            let value = if raw_translation.is_empty() {
                source.clone()
            } else {
                repair_cp1251_mojibake(raw_translation)
                    .unwrap_or_else(|| raw_translation.to_string())
            };
            let id = entry
                .get("id")
                .and_then(JsonValue::as_str)
                .filter(|value| !value.is_empty());
            let context = id.map(str::to_string).unwrap_or_else(|| {
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
            });
            let native_key = stable_key("ffone.localization.v1", &[context.as_bytes()]);
            let descriptor = json!({
                "key": native_key,
                "source": source,
                "value": value,
            });
            insert_fingerprint_record(
                &mut records,
                format!("localization/{native_key}"),
                json_bytes(&descriptor)?,
            )?;
        }
    }

    let table_rel = configured_relative(ffpatch, "TableDataDir", "tabledata")?;
    let table_root = project.join(&table_rel);
    let table_manifest = table_root.join("manifest.json");
    if let Some(path) = declared_overlay_manifest(
        &table_manifest,
        ffpatch.get("TableDataDir").is_some() || table_root.is_dir(),
        "tabledata",
    )? {
        let manifest = read_json_value(&path)?;
        require_manifest_format(&manifest, "fftools.tabledata-patch.v1", "tabledata")?;
        for entry in require_entries(&manifest, "tabledata")? {
            let relative = required_string(entry, "file", "tabledata entry")?;
            let source = resolve_overlay_file(project, &table_root, relative)?;
            let patch = read_json_value(&source)?;
            require_manifest_format(&patch, "fftools.tabledata-object.v1", "tabledata object")?;
            let value = patch
                .get("value")
                .and_then(|value| clean_table_value(value, true))
                .ok_or_else(|| format!("tabledata patch {relative} has no native value"))?;
            let name = entry
                .get("name")
                .and_then(JsonValue::as_str)
                .or_else(|| patch.get("name").and_then(JsonValue::as_str))
                .unwrap_or("table");
            let semantic = neutral_semantic_label(name, "table");
            let descriptor = json!({ "name": semantic, "value": value });
            insert_fingerprint_record(
                &mut records,
                format!("table/{}", relative.replace('\\', "/")),
                json_bytes(&descriptor)?,
            )?;
        }
    }

    let mut hasher = Blake3Hasher::new();
    hasher.update(b"ffone.authoring-overlay.v1");
    for (label, bytes) in records {
        hasher.update(&[0]);
        hasher.update(label.as_bytes());
        hasher.update(&[0]);
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(blake3::hash(&bytes).as_bytes());
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub(super) fn insert_fingerprint_record(
    records: &mut BTreeMap<String, Vec<u8>>,
    label: String,
    bytes: Vec<u8>,
) -> Result<(), String> {
    if records.insert(label.clone(), bytes).is_some() {
        return Err(format!(
            "duplicate authoring overlay fingerprint record: {label}"
        ));
    }
    Ok(())
}

pub(super) fn config_relative(
    ffpatch: &JsonValue,
    key: &str,
    default: &str,
    _errors: &mut Vec<String>,
) -> Result<PathBuf, String> {
    configured_relative(ffpatch, key, default)
}

pub(super) fn configured_relative(ffpatch: &JsonValue, key: &str, default: &str) -> Result<PathBuf, String> {
    let value = ffpatch
        .get(key)
        .and_then(JsonValue::as_str)
        .unwrap_or(default);
    validate_source_relative(value).map_err(|err| format!("unsafe ffpatch {key}: {err}"))?;
    Ok(PathBuf::from(value.replace('\\', "/")))
}

pub(super) fn ensure_contained_regular_file(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|err| format!("{}: {err}", root.display()))?;
    let path = fs::canonicalize(path).map_err(|err| format!("{}: {err}", path.display()))?;
    if !path.starts_with(&root) {
        return Err("overlay file escapes the project directory".to_string());
    }
    let metadata =
        fs::symlink_metadata(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "overlay source is not a regular file: {}",
            path.display()
        ));
    }
    Ok(path)
}

pub(super) fn contains_legacy_text(value: &str) -> bool {
    let folded = value.to_ascii_lowercase();
    folded.contains(".resourcefile")
        || folded.contains(".unity3d")
        || [".nif", ".kfm", ".kf", ".cg", ".obj"]
            .iter()
            .any(|extension| contains_forbidden_extension(&folded, extension))
        || folded.contains("assetbundle://")
        || folded.contains("archive:/")
        || folded.contains("file:///")
        || folded.starts_with("\\\\")
        || looks_like_posix_absolute(value)
        || looks_like_windows_absolute(value)
}

pub(super) fn contains_forbidden_extension(value: &str, extension: &str) -> bool {
    value.match_indices(extension).any(|(index, _)| {
        let before = value[..index].bytes().next_back();
        let after = value[index + extension.len()..].bytes().next();
        (index == 0
            || before.is_some_and(|byte| is_filename_byte(byte) || matches!(byte, b'/' | b'\\')))
            && after.is_none_or(|byte| !byte.is_ascii_alphanumeric())
    })
}

pub(super) fn is_filename_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

pub(super) fn looks_like_windows_absolute(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
}
