use super::super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct WorkspaceScan {
    pub(in super::super) root: String,
    pub(in super::super) map_bundles: Vec<String>,
    pub(in super::super) resource_bundles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct PatchPlan {
    pub(in super::super) tile_id: String,
    pub(in super::super) map_bundle: Option<String>,
    pub(in super::super) resource_bundle: Option<String>,
    pub(in super::super) terrain_height_samples: usize,
    pub(in super::super) texture_samples: usize,
    pub(in super::super) transform_edits: usize,
    pub(in super::super) collider_edits: usize,
    pub(in super::super) water_edits: usize,
    pub(in super::super) model_imports: usize,
    pub(in super::super) planned_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ClientProjectWorkspace {
    pub(in super::super) source_dir: String,
    pub(in super::super) project_dir: String,
    pub(in super::super) cache_dir: String,
    pub(in super::super) extracted_dir: String,
    pub(in super::super) translations_dir: String,
    pub(in super::super) textures_dir: String,
    pub(in super::super) audio_dir: String,
    pub(in super::super) patches_dir: String,
    pub(in super::super) build_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ScriptPatchEntry {
    pub(in super::super) assembly: String,
    pub(in super::super) source_file: String,
    pub(in super::super) edited_file: String,
    pub(in super::super) project_file: Option<String>,
    pub(in super::super) compiled_dll: Option<String>,
    pub(in super::super) target_file: Option<String>,
    pub(in super::super) content_hash: String,
    pub(in super::super) size: u64,
    pub(in super::super) modified_ms: u128,
    pub(in super::super) backend: String,
    pub(in super::super) notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct TableDataSectionPatchResult {
    pub(in super::super) path: String,
    pub(in super::super) offset: usize,
    pub(in super::super) rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct TableDataCellPatch {
    pub(in super::super) row: usize,
    pub(in super::super) field: String,
    pub(in super::super) value: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct NpcTextPatch {
    pub(in super::super) name: Option<String>,
    pub(in super::super) comment: Option<String>,
    pub(in super::super) comment1: Option<String>,
    pub(in super::super) comment2: Option<String>,
    pub(in super::super) barker_name: Option<String>,
    pub(in super::super) barker_comment: Option<String>,
    pub(in super::super) barker_comment1: Option<String>,
    pub(in super::super) barker_comment2: Option<String>,
}

#[derive(Debug, Default)]
pub(in super::super) struct ClientPatchConfig {
    pub(in super::super) font_ttf: Option<String>,
    pub(in super::super) font_dir: Option<String>,
    pub(in super::super) font_face: Option<String>,
    pub(in super::super) font_vertical_offset: Option<f64>,
    pub(in super::super) preserve_ascii_glyphs: bool,
    pub(in super::super) patch_unity_assets: bool,
    pub(in super::super) patch_table_data: bool,
    pub(in super::super) allow_texture_resize: bool,
    pub(in super::super) audio_dir: Option<String>,
    pub(in super::super) table_data_dir: Option<String>,
    pub(in super::super) server_tdata_path: Option<String>,
}

pub(in super::super) fn workspace_root_dir() -> Option<PathBuf> {
    crate::repository_root().to_path_buf()
        .parent()
        .map(Path::to_path_buf)
}

pub(in super::super) fn patch_config_from_json(value: &serde_json::Value) -> ClientPatchConfig {
    ClientPatchConfig {
        font_ttf: json_string(value, "FontTtf"),
        font_dir: json_string(value, "FontDir"),
        font_face: json_string(value, "FontFace"),
        font_vertical_offset: json_f64_string(value, "FontVerticalOffset"),
        preserve_ascii_glyphs: json_bool(value, "PreserveAsciiGlyphs", false),
        patch_unity_assets: json_bool(value, "PatchUnityAssets", true),
        patch_table_data: json_bool(value, "PatchTableData", true),
        allow_texture_resize: json_bool(value, "AllowTextureResize", false),
        audio_dir: json_string(value, "AudioDir"),
        table_data_dir: json_string(value, "TableDataDir"),
        server_tdata_path: normalized_server_tdata_path(
            json_string(value, "ServerTdataPath").as_deref(),
        ),
    }
}

pub(in super::super) fn project_patch_config_json(project: &Path) -> Option<JsonValue> {
    let path = project_patch_config_path(project);
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str::<JsonValue>(&data).ok()
}

pub(in super::super) fn project_source_dir_from_patch_config(project: &Path) -> Option<PathBuf> {
    let config = project_patch_config_json(project)?;
    let source = json_string(&config, "Source")?;
    let source_path = PathBuf::from(source);
    if source_path.is_dir() {
        Some(source_path)
    } else {
        None
    }
}

pub(in super::super) fn default_ffspy_backend_project() -> String {
    crate::repository_root().to_path_buf()
        .join("vendor")
        .join("FFSpy")
        .join("ICSharpCode.Decompiler.Console")
        .join("ICSharpCode.Decompiler.Console.csproj")
        .to_string_lossy()
        .to_string()
}

pub(in super::super) fn record_script_patch(
    project_dir: String,
    backend_project: String,
    assembly_path: String,
    source_file: String,
    notes: Option<String>,
) -> EditorResult<ScriptPatchManifest> {
    let project = PathBuf::from(project_dir);
    if !project.is_dir() {
        return Err(format!("{} is not a project directory", project.display()));
    }
    let file = PathBuf::from(source_file.trim());
    if !file.is_file() {
        return Err(format!(
            "Edited script file was not found: {}",
            file.display()
        ));
    }
    let data = fs::read(&file).map_err(|err| format!("{}: {err}", file.display()))?;
    let metadata = file
        .metadata()
        .map_err(|err| format!("{}: {err}", file.display()))?;
    let edited_file = path_relative_to_project(&project, &file)
        .unwrap_or_else(|| file.to_string_lossy().replace('\\', "/"));
    let mut manifest = load_script_patch_manifest(project.to_string_lossy().to_string())?;
    let entry = ScriptPatchEntry {
        assembly: assembly_path.replace('\\', "/"),
        source_file: file.to_string_lossy().replace('\\', "/"),
        edited_file,
        project_file: find_decompiled_csproj(&file)
            .map(|path| path.to_string_lossy().replace('\\', "/")),
        compiled_dll: None,
        target_file: PathBuf::from(&assembly_path)
            .file_name()
            .and_then(|value| value.to_str())
            .map(ToOwned::to_owned),
        content_hash: stable_bytes_hash(&data),
        size: metadata.len(),
        modified_ms: metadata_modified_ms(&metadata)?,
        backend: backend_project.replace('\\', "/"),
        notes: notes
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    };
    manifest.entries.retain(|item| {
        !(item.assembly.eq_ignore_ascii_case(&entry.assembly)
            && item.edited_file.eq_ignore_ascii_case(&entry.edited_file))
    });
    manifest.entries.push(entry);
    manifest.entries.sort_by_key(|entry| {
        (
            entry.assembly.to_ascii_lowercase(),
            entry.edited_file.to_ascii_lowercase(),
        )
    });

    let manifest_path = project_script_manifest_path(&project);
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let data = serde_json::to_string_pretty(&manifest).map_err(|err| err.to_string())?;
    fs::write(&manifest_path, format!("{data}\n"))
        .map_err(|err| format!("{}: {err}", manifest_path.display()))?;

    let mut patch_config = ensure_patch_project_config(&project)?;
    patch_config["ScriptPatchManifest"] = json!(PROJECT_SCRIPT_MANIFEST_RELATIVE);
    let patch_data = serde_json::to_string_pretty(&patch_config).map_err(|err| err.to_string())?;
    fs::write(project.join("ffpatch.json"), format!("{patch_data}\n"))
        .map_err(|err| err.to_string())?;

    Ok(manifest)
}

pub(in super::super) fn compile_script_project(
    project_dir: String,
    backend_project: String,
    assembly_path: String,
    source_file: String,
    notes: Option<String>,
) -> EditorResult<ScriptPatchManifest> {
    let project = PathBuf::from(project_dir);
    if !project.is_dir() {
        return Err(format!("{} is not a project directory", project.display()));
    }
    let assembly = PathBuf::from(assembly_path.trim());
    if !assembly.is_file() {
        return Err(format!("Assembly was not found: {}", assembly.display()));
    }
    let source = PathBuf::from(source_file.trim());
    if !source.exists() {
        return Err(format!("Script source was not found: {}", source.display()));
    }
    let csproj = find_decompiled_csproj(&source)
        .ok_or_else(|| format!("No .csproj found near {}", source.display()))?;
    let source_root = csproj.parent().unwrap_or_else(|| Path::new("."));
    let sanitized = sanitize_decompiled_sources(source_root)?;
    normalize_decompiled_csproj(&project, &assembly, &csproj)?;

    let output = Command::new("dotnet")
        .arg("build")
        .arg(&csproj)
        .arg("-c")
        .arg("Release")
        .output()
        .map_err(|err| format!("Failed to start dotnet build: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "Script project build failed with status {}.{}{}",
            output.status,
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(" stderr: {}", stderr.trim())
            },
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!(" stdout: {}", stdout.trim())
            }
        ));
    }

    let assembly_name = fusionforge::managed::managed_assembly_name(&assembly)?
        .or_else(|| {
            csproj
                .file_stem()
                .and_then(|value| value.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "Assembly".to_string());
    let built = source_root
        .join("bin")
        .join("Release")
        .join("net20")
        .join(format!("{assembly_name}.dll"));
    if !built.is_file() {
        return Err(format!("Compiled DLL was not found: {}", built.display()));
    }
    let target_file = assembly
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Assembly.dll")
        .to_string();
    let compiled_dir = project.join("scripts").join("compiled");
    fs::create_dir_all(&compiled_dir)
        .map_err(|err| format!("{}: {err}", compiled_dir.display()))?;
    let compiled_target = compiled_dir.join(&target_file);
    fs::copy(&built, &compiled_target).map_err(|err| {
        format!(
            "{} -> {}: {err}",
            built.display(),
            compiled_target.display()
        )
    })?;

    let mut manifest = record_script_patch(
        project.to_string_lossy().to_string(),
        backend_project,
        assembly.to_string_lossy().to_string(),
        source.to_string_lossy().to_string(),
        notes,
    )?;
    let assembly_key = assembly.to_string_lossy().replace('\\', "/");
    for entry in &mut manifest.entries {
        if entry.assembly.eq_ignore_ascii_case(&assembly_key) {
            entry.project_file = Some(csproj.to_string_lossy().replace('\\', "/"));
            entry.compiled_dll = Some(compiled_target.to_string_lossy().replace('\\', "/"));
            entry.target_file = Some(target_file.clone());
            if sanitized > 0 {
                let note = format!("sanitized {sanitized} decompiler artifact file(s)");
                entry.notes = Some(match entry.notes.take() {
                    Some(existing) if !existing.is_empty() => format!("{existing}; {note}"),
                    _ => note,
                });
            }
        }
    }
    write_script_patch_manifest(&project, &manifest)?;
    Ok(manifest)
}

pub(in super::super) fn build_patch_plan_from_json(document: &serde_json::Value) -> PatchPlan {
    let tile_id = document
        .get("tileId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Map_00_00")
        .to_string();
    let map_bundle = document
        .get("sourceMapBundle")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let resource_bundle = document
        .get("sourceResourceBundle")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let objects = document
        .get("objects")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let model_imports = objects
        .iter()
        .filter(|object| {
            object
                .get("source")
                .and_then(|source| source.get("kind"))
                .and_then(serde_json::Value::as_str)
                == Some("externalModel")
        })
        .count();
    let mut planned_steps = vec![
        "Load source map bundle as read-only input".to_string(),
        "Write world edits to fftools.world-patch.v1 staging manifest".to_string(),
        "Classify map-local, tile-resource, shared and new-bundle assets".to_string(),
    ];
    if model_imports > 0 {
        planned_steps.push(
            "Convert GLB/OBJ imports through a selected Unity template bundle at build time"
                .to_string(),
        );
    }

    PatchPlan {
        tile_id,
        map_bundle,
        resource_bundle,
        terrain_height_samples: json_array_len(document, &["terrain", "heights"]),
        texture_samples: json_array_len(document, &["terrain", "textureMap"]),
        transform_edits: objects.len(),
        collider_edits: json_array_len(document, &["colliders"]),
        water_edits: json_array_len(document, &["waterVolumes"]),
        model_imports,
        planned_steps,
    }
}

pub(in super::super) fn scan_workspace(root: String) -> EditorResult<WorkspaceScan> {
    let root_path = PathBuf::from(&root);
    if !root_path.exists() {
        return Err(EditorError::MissingPath(root).to_string());
    }

    let mut map_bundles = Vec::new();
    let mut resource_bundles = Vec::new();
    scan_recursive(&root_path, &mut map_bundles, &mut resource_bundles)
        .map_err(|err| err.to_string())?;
    map_bundles.sort();
    resource_bundles.sort();

    Ok(WorkspaceScan {
        root: root_path.to_string_lossy().to_string(),
        map_bundles,
        resource_bundles,
    })
}

pub(in super::super) fn export_patch_plan(path: String, document: serde_json::Value) -> EditorResult<PatchPlan> {
    let plan = build_patch_plan_from_json(&document);
    let data =
        serde_json::to_string_pretty(&plan).map_err(|err| EditorError::Json(err).to_string())?;
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    fs::write(&path, format!("{data}\n")).map_err(|err| EditorError::Io(err).to_string())?;
    Ok(plan)
}

pub(in super::super) fn default_table_data_project_dir(project_dir: Option<String>) -> PathBuf {
    project_dir
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::repository_root().to_path_buf().join("work").join("fusionforge"))
}

pub(in super::super) fn table_data_patch_value(
    path: &Path,
    asset_index: usize,
) -> Result<fusionforge::UnityValue, String> {
    let document = serde_json::from_str::<JsonValue>(
        &fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", path.display()))?;
    let value = document.get("value").unwrap_or(&document);
    json_to_unity_value(value, asset_index)
}

pub(in super::super) fn global_npc_source_import_project_dir(source_dir: &Path) -> PathBuf {
    crate::repository_root().to_path_buf()
        .join("work")
        .join("sources")
        .join("npc-imports")
        .join(npc_source_import_name(source_dir))
}

pub(in super::super) fn legacy_npc_source_import_project_dir(
    source_dir: &Path,
    target_project_dir: Option<&str>,
) -> PathBuf {
    let import_name = npc_source_import_name(source_dir);
    target_project_dir
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .map(|path| path.join("npc-imports").join(import_name.clone()))
        .unwrap_or_else(|| {
            source_dir
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(format!("{import_name}.ffclient"))
        })
}

pub(in super::super) fn npc_source_import_project_dir(
    source_dir: &Path,
    _target_project_dir: Option<String>,
) -> PathBuf {
    global_npc_source_import_project_dir(source_dir)
}

pub(in super::super) fn npc_source_import_project_candidates(
    source_dir: &Path,
    target_project_dir: Option<String>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let global = global_npc_source_import_project_dir(source_dir);
    candidates.push(global.clone());
    let legacy = legacy_npc_source_import_project_dir(source_dir, target_project_dir.as_deref());
    if legacy != global {
        candidates.push(legacy);
    }
    let legacy_without_target = legacy_npc_source_import_project_dir(source_dir, None);
    if !candidates.iter().any(|path| path == &legacy_without_target) {
        candidates.push(legacy_without_target);
    }
    candidates
}

pub(in super::super) fn stage_npc_text_patch(
    project_dir: String,
    bundle_path: String,
    asset: String,
    path_id: i64,
    npc_id: usize,
    patch: NpcTextPatch,
) -> EditorResult<String> {
    run_table_data_task(move || {
        stage_npc_text_patch_impl(project_dir, bundle_path, asset, path_id, npc_id, patch)
    })
}

pub(in super::super) fn stage_npc_text_patch_impl(
    project_dir: String,
    bundle_path: String,
    asset: String,
    path_id: i64,
    npc_id: usize,
    patch: NpcTextPatch,
) -> EditorResult<String> {
    use fusionforge::object_name;

    let bundle = PathBuf::from(&bundle_path);
    if !bundle.is_file() {
        return Err(EditorError::MissingPath(bundle_path).to_string());
    }
    let project = PathBuf::from(project_dir);
    let extract_dir = extract_bundle_cached(&project, &bundle)?;
    let asset_path = extracted_asset_path(&extract_dir, &asset)?;
    let asset_file = fusionforge::Asset::from_path(&asset_path)?;
    let info = asset_file
        .objects
        .get(&path_id)
        .ok_or_else(|| format!("{}#{} was not found", asset_file.name, path_id))?;
    let mut body = asset_file.read_object(0, info)?;
    let npc_table = npc_table_from_body_mut(&mut body)
        .ok_or_else(|| "selected TableData object has no m_pNpcTable".to_string())?;
    let (string_index, barker_index) = {
        let npc_data_rows = npc_table
            .get("m_pNpcData")
            .and_then(fusionforge::UnityValue::as_array)
            .ok_or_else(|| "m_pNpcTable.m_pNpcData is not an array".to_string())?;
        let npc_data = npc_data_rows
            .get(npc_id)
            .ok_or_else(|| format!("m_pNpcTable.m_pNpcData[{npc_id}] does not exist"))?;
        (
            unity_usize_field(npc_data, "m_iNpcName").unwrap_or(npc_id),
            unity_usize_field(npc_data, "m_iBarkerNumber"),
        )
    };
    let mut changed = false;
    let patched_npc_name = {
        let string_rows = npc_table
            .get_mut("m_pNpcStringData")
            .and_then(fusionforge::UnityValue::as_array_mut)
            .ok_or_else(|| "m_pNpcTable.m_pNpcStringData is not an array".to_string())?;
        let string_row = string_rows
            .get_mut(string_index)
            .ok_or_else(|| format!("NPC string row {string_index} does not exist"))?;
        changed |= set_optional_unity_string(string_row, "m_strName", patch.name.clone())?;
        changed |= set_optional_unity_string(string_row, "m_strComment", patch.comment.clone())?;
        changed |= set_optional_unity_string(string_row, "m_strComment1", patch.comment1.clone())?;
        changed |= set_optional_unity_string(string_row, "m_strComment2", patch.comment2.clone())?;
        unity_string_field(Some(string_row), "m_strName").unwrap_or_else(|| format!("NPC {npc_id}"))
    };

    if patch.barker_name.is_some()
        || patch.barker_comment.is_some()
        || patch.barker_comment1.is_some()
        || patch.barker_comment2.is_some()
    {
        let barker_rows = npc_table
            .get_mut("m_pNpcBarkerData")
            .and_then(fusionforge::UnityValue::as_array_mut)
            .ok_or_else(|| "m_pNpcTable.m_pNpcBarkerData is not an array".to_string())?;
        let Some(barker_index) = barker_index else {
            return Err(format!(
                "NPC {npc_id} has no linked m_iBarkerNumber; refusing to patch unrelated barker row"
            ));
        };
        let barker_row = barker_rows
            .get_mut(barker_index)
            .ok_or_else(|| format!("NPC barker row {barker_index} does not exist"))?;
        changed |= set_optional_unity_string(barker_row, "m_strName", patch.barker_name)?;
        changed |= set_optional_unity_string(barker_row, "m_strComment", patch.barker_comment)?;
        changed |= set_optional_unity_string(barker_row, "m_strComment1", patch.barker_comment1)?;
        changed |= set_optional_unity_string(barker_row, "m_strComment2", patch.barker_comment2)?;
    }

    if !changed {
        return Err("No NPC text fields were changed.".to_string());
    }

    let patch_name = format!("npc_{}__{}", npc_id, safe_segment(&patched_npc_name));
    let document = json!({
        "format": "fftools.tabledata-object.v1",
        "sourceBundle": bundle.to_string_lossy(),
        "container": container_name_from_bundle(&bundle),
        "asset": asset_file.name,
        "pathId": path_id,
        "name": object_name(&body),
        "objectType": asset_file.object_type_name(info),
        "value": unity_value_to_json(&body),
    });
    let target = write_table_data_patch_document(
        &project,
        &bundle_path,
        &asset,
        path_id,
        &patch_name,
        &document,
    )?;
    Ok(format!(
        "Staged NPC text patch: {}",
        target.to_string_lossy()
    ))
}

pub(in super::super) fn patch_npc_linked_row_fields(
    npc_table: &mut fusionforge::UnityValue,
    npc_id: usize,
    npc_row: &fusionforge::UnityValue,
    section: &str,
    index_field: &str,
    fields: Option<&serde_json::Map<String, JsonValue>>,
) -> Result<bool, String> {
    let Some(fields) = fields else {
        return Ok(false);
    };
    let index = unity_usize_field(npc_row, index_field).unwrap_or(npc_id);
    let rows = npc_target_rows_mut(npc_table, section)
        .ok_or_else(|| format!("m_pNpcTable.{section} is not an array"))?;
    let row = rows
        .get_mut(index)
        .ok_or_else(|| format!("m_pNpcTable.{section}[{index}] does not exist"))?;
    patch_unity_object_fields(row, Some(fields))
}

pub(in super::super) fn patch_table_section_row_fields(
    body: &mut fusionforge::UnityValue,
    section_path: &str,
    row_index: usize,
    fields: Option<&serde_json::Map<String, JsonValue>>,
) -> Result<bool, String> {
    let Some(fields) = fields else {
        return Ok(false);
    };
    let section = table_value_at_path_mut(body, section_path)
        .ok_or_else(|| format!("section was not found: {section_path}"))?;
    let rows = section
        .as_array_mut()
        .ok_or_else(|| format!("{section_path} is not an array"))?;
    let Some(row) = rows.get_mut(row_index) else {
        return Ok(false);
    };
    patch_unity_object_fields(row, Some(fields))
}
