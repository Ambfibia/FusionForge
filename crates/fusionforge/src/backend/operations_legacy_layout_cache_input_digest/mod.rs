use super::super::*;

pub(in super::super) fn prune_unreferenced_table_data_patches(project: &Path) -> Result<(), String> {
    let table_data_root = project.join("tabledata");
    let manifest_path = table_data_root.join("manifest.json");
    if !manifest_path.exists() {
        return Ok(());
    }
    let manifest = serde_json::from_str::<JsonValue>(
        &fs::read_to_string(&manifest_path).map_err(|err| EditorError::Io(err).to_string())?,
    )
    .unwrap_or_else(|_| json!({ "entries": [] }));
    let Some(entries) = manifest.get("entries").and_then(JsonValue::as_array) else {
        return Ok(());
    };

    let mut active_files = BTreeSet::new();
    let mut managed_prefixes = BTreeSet::<(String, String)>::new();
    for entry in entries {
        let Some(file) = manifest_entry_file(entry) else {
            continue;
        };
        active_files.insert(file.clone());
        if let Some(path_id) = entry
            .get("pathId")
            .or_else(|| entry.get("path_id"))
            .and_then(JsonValue::as_i64)
        {
            let (parent, _) = table_data_patch_file_parent_and_name(&file);
            managed_prefixes.insert((parent.to_string(), format!("{path_id}__")));
        }
    }
    if managed_prefixes.is_empty() {
        return Ok(());
    }

    let mut files = BTreeSet::new();
    collect_file_keys(&table_data_root, &table_data_root, &mut files)?;
    for file in files {
        if file == "manifest.json" || active_files.contains(&file) || !file.ends_with(".json") {
            continue;
        }
        let (parent, name) = table_data_patch_file_parent_and_name(&file);
        let is_managed_stale = managed_prefixes
            .iter()
            .any(|(managed_parent, prefix)| managed_parent == parent && name.starts_with(prefix));
        if !is_managed_stale {
            continue;
        }
        let path = path_from_normalized_relative(&table_data_root, &file)?;
        if path.is_file() {
            fs::remove_file(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        }
    }
    remove_empty_dirs(&table_data_root, &table_data_root)?;
    Ok(())
}

pub(in super::super) fn safe_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub(in super::super) fn stage_font_settings(
    project_dir: String,
    font_path: String,
    font_face: String,
    include_russian: bool,
    replace_ascii: bool,
    vertical_offset: f64,
) -> EditorResult<String> {
    let project = PathBuf::from(project_dir);
    let mut patch_config = ensure_patch_project_config(&project)?;
    let source = PathBuf::from(font_path);
    if !source.is_file() {
        return Err(EditorError::MissingPath(source.to_string_lossy().to_string()).to_string());
    }
    let fonts_dir = project.join("fonts");
    fs::create_dir_all(&fonts_dir).map_err(|err| EditorError::Io(err).to_string())?;
    let target = fonts_dir.join(source.file_name().unwrap_or_default());
    fs::copy(&source, &target).map_err(|err| EditorError::Io(err).to_string())?;
    let relative_font = target
        .strip_prefix(&project)
        .unwrap_or(&target)
        .to_string_lossy()
        .to_string();
    let config_path = fonts_dir.join("font-settings.json");
    let config = json!({
        "format": "fftools.font_settings.v1",
        "fontTtf": relative_font,
        "fontFace": font_face,
        "includeRussian": include_russian,
        "replaceAscii": replace_ascii,
        "verticalOffset": vertical_offset,
    });
    let data =
        serde_json::to_string_pretty(&config).map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(&config_path, format!("{data}\n")).map_err(|err| EditorError::Io(err).to_string())?;
    patch_config["FontTtf"] = json!(relative_font);
    patch_config["FontFace"] = json!(font_face);
    patch_config["FontVerticalOffset"] = json!(vertical_offset.to_string());
    patch_config["PreserveAsciiGlyphs"] = json!(!replace_ascii);
    patch_config["IncludeRussianGlyphs"] = json!(include_russian);
    let patch_data = serde_json::to_string_pretty(&patch_config)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(project.join("ffpatch.json"), format!("{patch_data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    Ok(format!(
        "Staged font settings: {}",
        config_path.to_string_lossy()
    ))
}

pub(in super::super) fn stage_font_replacement(
    project_dir: String,
    family: String,
    font_path: String,
    font_face: String,
    include_russian: bool,
    replace_ascii: bool,
    vertical_offset: f64,
    target: Option<ClientFontRecord>,
) -> EditorResult<String> {
    let project = PathBuf::from(project_dir);
    let mut patch_config = ensure_patch_project_config(&project)?;
    let source = PathBuf::from(font_path);
    if !source.is_file() {
        return Err(EditorError::MissingPath(source.to_string_lossy().to_string()).to_string());
    }
    let family = normalize_font_family(&family);
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("ttf");
    let fonts_dir = project.join("fonts");
    fs::create_dir_all(&fonts_dir).map_err(|err| EditorError::Io(err).to_string())?;
    let target_path = fonts_dir.join(format!(
        "{}.{}",
        safe_segment(&family),
        safe_segment(extension)
    ));
    fs::copy(&source, &target_path).map_err(|err| EditorError::Io(err).to_string())?;
    let relative_font = target_path
        .strip_prefix(&project)
        .unwrap_or(&target_path)
        .to_string_lossy()
        .replace('\\', "/");

    let manifest_path = fonts_dir.join("manifest.json");
    let mut manifest = if manifest_path.exists() {
        serde_json::from_str::<serde_json::Value>(
            &fs::read_to_string(&manifest_path).map_err(|err| EditorError::Io(err).to_string())?,
        )
        .unwrap_or_else(|_| json!({ "format": "fftools.font-patch.v1", "entries": [] }))
    } else {
        json!({ "format": "fftools.font-patch.v1", "entries": [] })
    };
    let entries = manifest["entries"]
        .as_array_mut()
        .ok_or_else(|| "Font manifest entries is not an array".to_string())?;
    entries.retain(|item| item.get("family") != Some(&json!(family)));
    entries.push(json!({
        "family": family,
        "fontFile": relative_font,
        "fontFace": font_face,
        "includeRussian": include_russian,
        "replaceAscii": replace_ascii,
        "verticalOffset": vertical_offset,
        "target": target,
    }));
    let data = serde_json::to_string_pretty(&manifest)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(&manifest_path, format!("{data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;

    patch_config["FontDir"] = json!("fonts");
    patch_config["PreserveAsciiGlyphs"] = json!(!replace_ascii);
    patch_config["FontVerticalOffset"] = json!(vertical_offset.to_string());
    let patch_data = serde_json::to_string_pretty(&patch_config)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(project.join("ffpatch.json"), format!("{patch_data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    Ok(format!(
        "Staged font replacement for {family}: {}",
        target_path.to_string_lossy()
    ))
}

pub(in super::super) fn native_build_temp_dir(label: &str) -> Result<NativeBuildTempDir, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let root = env::temp_dir()
        .join("fusionforge-native-build")
        .join(format!("{label}_{nanos:x}"));
    let extracted = root.join("bundle");
    fs::create_dir_all(&extracted).map_err(|err| err.to_string())?;
    Ok(NativeBuildTempDir { root, extracted })
}

pub(in super::super) fn normalized_relative_file(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|err| format!("{} under {}: {err}", path.display(), root.display()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

pub(in super::super) fn build_source_snapshot(source: &Path) -> Result<BuildSourceSnapshot, String> {
    let mut files = BTreeMap::new();
    collect_source_snapshot_files(source, source, &mut files)?;
    Ok(BuildSourceSnapshot {
        format: "fftools.build-source-snapshot.v1".to_string(),
        source_dir: source.to_string_lossy().to_string(),
        files,
        patched_files: BTreeSet::new(),
    })
}

pub(in super::super) fn remove_empty_dirs(root: &Path, current: &Path) -> Result<(), String> {
    if !current.is_dir() {
        return Ok(());
    }
    let children = fs::read_dir(current)
        .map_err(|err| format!("{}: {err}", current.display()))?
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    for child in children {
        if child.is_dir() {
            remove_empty_dirs(root, &child)?;
        }
    }
    if current != root
        && fs::read_dir(current)
            .map_err(|err| format!("{}: {err}", current.display()))?
            .next()
            .is_none()
    {
        fs::remove_dir(current).map_err(|err| format!("{}: {err}", current.display()))?;
    }
    Ok(())
}

pub(in super::super) fn restore_pristine_build_output(
    source: &Path,
    out_dir: &Path,
) -> Result<BuildSourceSnapshot, String> {
    // Do not preserve generated files and do not trust patched_files here: a process may
    // have stopped before that metadata was persisted. Forced sync both removes layout-only
    // files and restores every source file byte-for-byte.
    sync_build_output_from_source_preserving(source, out_dir, &BTreeSet::new(), true)
}

pub(in super::super) fn newest_modified_ms(paths: &[PathBuf]) -> Result<Option<u128>, String> {
    let mut newest: Option<u128> = None;
    for path in paths {
        let metadata = path
            .metadata()
            .map_err(|err| format!("{}: {err}", path.display()))?;
        let modified = metadata_modified_ms(&metadata)?;
        newest = Some(newest.map_or(modified, |current| current.max(modified)));
    }
    Ok(newest)
}

pub(in super::super) fn configured_npc_icon_fallbacks(
    patch_config: &JsonValue,
) -> Result<BTreeMap<usize, usize>, String> {
    let Some(entries) = patch_config
        .get("NpcIconFallbacks")
        .and_then(JsonValue::as_array)
    else {
        return Ok(BTreeMap::new());
    };
    let mut result = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let npc_id = entry
            .get("NpcId")
            .or_else(|| entry.get("npcId"))
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("NpcIconFallbacks[{index}] has no valid NpcId"))?;
        let fallback_npc_id = entry
            .get("FallbackNpcId")
            .or_else(|| entry.get("fallbackNpcId"))
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("NpcIconFallbacks[{index}] has no valid FallbackNpcId"))?;
        if npc_id == fallback_npc_id {
            return Err(format!(
                "NpcIconFallbacks[{index}] points NPC {npc_id} at itself"
            ));
        }
        if result.insert(npc_id, fallback_npc_id).is_some() {
            return Err(format!(
                "NpcIconFallbacks contains duplicate target NPC {npc_id}"
            ));
        }
    }
    Ok(result)
}

pub(in super::super) fn sha256_file_content(path: &Path) -> Result<(u64, String), String> {
    let mut file = fs::File::open(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| format!("{}: {err}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes = bytes.saturating_add(read as u64);
        hasher.update(&buffer[..read]);
    }
    Ok((bytes, format!("{:x}", hasher.finalize())))
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn legacy_layout_cache_input_digest(
    project: &Path,
    source: &Path,
    source_snapshot: &BuildSourceSnapshot,
    patch_config: &JsonValue,
    translation_json: &Path,
    texture_dir: &Path,
    texture_manifest: &Path,
    texture_entries: &[JsonValue],
    audio_dir: &Path,
    audio_manifest: &Path,
    audio_entries: &[JsonValue],
    table_data_dir: &Path,
    table_data_manifest: &Path,
    table_data_entries: &[JsonValue],
    external_resource_imports: &[ExternalResourceImportSpec],
) -> Result<String, String> {
    let mut inputs = LegacyLayoutInputFiles::default();

    for relative in source_snapshot.files.keys() {
        let lower = relative.to_ascii_lowercase();
        if !(lower.ends_with(".resourcefile") || lower.ends_with(".unity3d")) {
            continue;
        }
        inputs.add_file(
            format!("source/{}", relative.replace('\\', "/")),
            &path_from_normalized_relative(source, relative)?,
        )?;
    }
    if inputs.by_canonical_path.is_empty() {
        return Err("legacy layout cache input has no pristine Unity bundles".to_string());
    }

    inputs.add_file("translation/index", translation_json)?;
    inputs.add_manifest_entries("textures", texture_dir, texture_manifest, texture_entries)?;
    inputs.add_manifest_entries("audio", audio_dir, audio_manifest, audio_entries)?;
    inputs.add_manifest_entries(
        "tabledata",
        table_data_dir,
        table_data_manifest,
        table_data_entries,
    )?;

    if let Some(font_ttf) = json_string(patch_config, "FontTtf") {
        inputs.add_file("fonts/ttf", &resolve_project_path(project, &font_ttf))?;
    }
    if let Some(font_dir) = json_string(patch_config, "FontDir") {
        inputs.add_tree("fonts/dir", &resolve_project_path(project, &font_dir))?;
    }

    if let Some(script_manifest) = json_string(patch_config, "ScriptPatchManifest") {
        let manifest_path = resolve_project_path(project, &script_manifest);
        inputs.add_file("scripts/manifest", &manifest_path)?;
        let manifest = load_script_patch_manifest_from_path(&manifest_path)?;
        for (index, entry) in manifest.entries.iter().enumerate() {
            if let Some(compiled_dll) = entry
                .compiled_dll
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                inputs.add_file(
                    format!("scripts/entry-{index}/compiled-dll"),
                    &resolve_project_path(project, compiled_dll),
                )?;
            }
        }
    }

    let npc_import_dir = npc_import_dir_from_patch_config(project, patch_config);
    if npc_import_dir.is_dir() {
        let mut manifests = Vec::new();
        collect_npc_import_manifest_paths(&npc_import_dir, &mut manifests);
        manifests.sort();
        manifests.dedup();
        for (index, manifest_path) in manifests.iter().enumerate() {
            let manifest_data = fs::read_to_string(manifest_path)
                .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
            let manifest = serde_json::from_str::<JsonValue>(&manifest_data)
                .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
            if manifest.get("format").and_then(JsonValue::as_str) != Some("fftools.npc-import.v1") {
                continue;
            }
            let blueprint = manifest
                .get("blueprint")
                .cloned()
                .ok_or_else(|| format!("{} has no blueprint", manifest_path.display()))
                .and_then(|value| {
                    serde_json::from_value::<NpcBlueprint>(value)
                        .map_err(|err| format!("{}: {err}", manifest_path.display()))
                })?;
            let label = format!("npcs/import-{index}");
            for (input_index, input) in
                npc_standalone_bundle_inputs(project, manifest_path, &manifest, &blueprint)?
                    .into_iter()
                    .enumerate()
            {
                if input.is_dir() {
                    inputs.add_tree(&format!("{label}/input-{input_index}/tree"), &input)?;
                } else if input.is_file() {
                    inputs.add_file(format!("{label}/input-{input_index}"), &input)?;
                }
            }
            let context = npc_import_manifest_context(project, &manifest);
            if let Some(path) = context
                .source_table_data_bundle
                .as_deref()
                .filter(|path| path.is_file())
            {
                inputs.add_file(format!("{label}/source-tabledata"), path)?;
            }
            if let Some(path) = context
                .table_data_patch_path
                .as_deref()
                .filter(|path| path.is_file())
            {
                inputs.add_file(format!("{label}/tabledata-patch"), path)?;
            }
        }
    }
    if let Some(npc_bundle_dir) = json_string(patch_config, "NpcBundleDir") {
        inputs.add_tree(
            "npcs/prebuilt-bundles",
            &resolve_project_path(project, &npc_bundle_dir),
        )?;
    }

    for (index, spec) in external_resource_imports.iter().enumerate() {
        if spec.source.is_dir() {
            inputs.add_tree(&format!("external-import-{index}/source"), &spec.source)?;
        } else {
            inputs.add_file(format!("external-import-{index}/source"), &spec.source)?;
        }
    }

    inputs.digest(patch_config)
}

pub(in super::super) fn current_npc_build_output_files(
    project: &Path,
    patch_config: &JsonValue,
) -> Result<BTreeSet<String>, String> {
    let npc_import_dir = npc_import_dir_from_patch_config(project, patch_config);
    let mut output_files = BTreeSet::new();
    if !npc_import_dir.is_dir() {
        return Ok(output_files);
    }
    let mut manifests = Vec::new();
    collect_npc_import_manifest_paths(&npc_import_dir, &mut manifests);
    for manifest_path in manifests {
        let manifest = serde_json::from_str::<JsonValue>(
            &fs::read_to_string(&manifest_path)
                .map_err(|err| format!("{}: {err}", manifest_path.display()))?,
        )
        .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
        if manifest.get("format").and_then(JsonValue::as_str) != Some("fftools.npc-import.v1") {
            continue;
        }
        if manifest.get("bundleStrategy").and_then(JsonValue::as_str)
            == Some("tabledata-only-npc-link")
        {
            continue;
        }
        if let Some(bundle_name) = manifest
            .get("bundleName")
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            output_files.insert(bundle_name.replace('\\', "/"));
            continue;
        }
        if let Some(blueprint) = manifest
            .get("blueprint")
            .cloned()
            .and_then(|value| serde_json::from_value::<NpcBlueprint>(value).ok())
        {
            output_files.insert(npc_blueprint_bundle_name(&blueprint));
        }
    }
    Ok(output_files)
}

pub(in super::super) fn contains_cyrillic_text(value: &str) -> bool {
    value
        .chars()
        .any(|ch| matches!(ch, '\u{0400}'..='\u{04ff}' | '\u{0500}'..='\u{052f}'))
}

pub(in super::super) fn is_empty_translated_source(original: &str) -> bool {
    fusionforge::managed::repair_cp1251_mojibake(original).is_some()
        || contains_cyrillic_text(original)
}

pub(in super::super) fn inspect_bundles(
    repo_root: Option<String>,
    map_bundle: Option<String>,
    resource_bundle: Option<String>,
    build_root: Option<String>,
    neighbor_radius: Option<u8>,
) -> EditorResult<serde_json::Value> {
    let repo_root_path = repo_root
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_repo_root);
    let repo_root_path = if repo_root_path.is_relative() {
        default_repo_root().join(repo_root_path)
    } else {
        repo_root_path
    };

    fusionforge::inspect_world_bundles(fusionforge::WorldInspectOptions {
        repo_root: repo_root_path,
        map_bundle: map_bundle
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from),
        resource_bundle: resource_bundle
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from),
        build_root: build_root
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from),
        neighbor_radius: neighbor_radius.unwrap_or_default(),
    })
}

pub(in super::super) fn inspect_world_scene(
    repo_root: Option<String>,
    map_bundle: Option<String>,
    resource_bundle: Option<String>,
    build_root: Option<String>,
    neighbor_radius: Option<u8>,
) -> EditorResult<serde_json::Value> {
    inspect_bundles(
        repo_root,
        map_bundle,
        resource_bundle,
        build_root,
        neighbor_radius,
    )
}

pub(in super::super) fn inspect_legacy_layout_cache_key(
    project_dir: &Path,
    source_dir: &Path,
) -> Result<JsonValue, String> {
    let project = project_dir
        .canonicalize()
        .map_err(|err| format!("{}: {err}", project_dir.display()))?;
    let source = source_dir
        .canonicalize()
        .map_err(|err| format!("{}: {err}", source_dir.display()))?;
    let config_path = project.join("ffpatch.json");
    let patch_config = serde_json::from_str::<JsonValue>(
        &fs::read_to_string(&config_path)
            .map_err(|err| format!("{}: {err}", config_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", config_path.display()))?;
    let config = patch_config_from_json(&patch_config);
    let translation_json = resolve_project_path(
        &project,
        &json_string(&patch_config, "TranslationJson")
            .unwrap_or_else(|| PROJECT_TRANSLATION_INDEX_RELATIVE.to_string()),
    );
    let texture_dir = project.join("textures");
    let texture_manifest = texture_dir.join("manifest.json");
    let texture_entries = manifest_entries(&texture_manifest, "fftools.texture-patch.v1")?;
    let audio_dir = config
        .audio_dir
        .as_deref()
        .map(|value| resolve_project_path(&project, value))
        .unwrap_or_else(|| project.join("audio"));
    let audio_manifest = audio_dir.join("manifest.json");
    let audio_entries = manifest_entries(&audio_manifest, "fftools.audio-patch.v1")?;
    let table_data_dir = config
        .table_data_dir
        .as_deref()
        .map(|value| resolve_project_path(&project, value))
        .unwrap_or_else(|| project.join("tabledata"));
    let table_data_manifest = table_data_dir.join("manifest.json");
    let table_data_entries = manifest_entries(&table_data_manifest, "fftools.tabledata-patch.v1")?;
    let source_snapshot = build_source_snapshot(&source)?;
    let external_resource_imports = external_resource_import_specs(&project, &patch_config)?;
    let stable_input_digest = legacy_layout_cache_input_digest(
        &project,
        &source,
        &source_snapshot,
        &patch_config,
        &translation_json,
        &texture_dir,
        &texture_manifest,
        &texture_entries,
        &audio_dir,
        &audio_manifest,
        &audio_entries,
        &table_data_dir,
        &table_data_manifest,
        &table_data_entries,
        &external_resource_imports,
    )?;
    let result_cache_key =
        legacy_bundle_layout::result_cache_key(&patch_config, &stable_input_digest)?;
    Ok(json!({
        "format": "ffclient.legacy-layout-cache-key-inspection.v1",
        "project": project,
        "source": source,
        "sourceUnityFiles": source_snapshot.files.keys().filter(|name| {
            let lower = name.to_ascii_lowercase();
            lower.ends_with(".resourcefile") || lower.ends_with(".unity3d")
        }).count(),
        "audioManifestEntries": audio_entries.len(),
        "stableInputSha256": stable_input_digest,
        "resultCacheKey": result_cache_key,
    }))
}

/// Canonical workspace root after the CLI crate moved two levels below it.
pub(crate) fn repository_root() -> &'static std::path::Path {
    static ROOT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(||std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("FusionForge workspace root")).as_path()
}
