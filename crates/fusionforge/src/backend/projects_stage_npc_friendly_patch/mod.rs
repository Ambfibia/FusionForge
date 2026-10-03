use super::super::*;

pub(in super::super) fn stage_npc_friendly_patch(
    project_dir: String,
    bundle_path: String,
    asset: String,
    path_id: i64,
    npc_id: usize,
    patch: JsonValue,
) -> EditorResult<String> {
    run_table_data_task(move || {
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
        let mut body = if let Some(body) =
            table_data_patch_body_for_object(&project, &bundle_path, &asset, path_id)?
        {
            body
        } else {
            asset_file.read_object(0, info)?
        };
        let patch_object = patch
            .as_object()
            .ok_or_else(|| "friendly NPC patch is not an object".to_string())?;
        let mut changed = false;

        {
            let npc_table = npc_table_from_body_mut(&mut body)
                .ok_or_else(|| "selected TableData object has no m_pNpcTable".to_string())?;
            let npc_row_snapshot = npc_target_rows_mut(npc_table, "m_pNpcData")
                .and_then(|rows| rows.get(npc_id).cloned())
                .ok_or_else(|| format!("m_pNpcTable.m_pNpcData[{npc_id}] does not exist"))?;
            let npc_rows = npc_target_rows_mut(npc_table, "m_pNpcData")
                .ok_or_else(|| "m_pNpcTable.m_pNpcData is not an array".to_string())?;
            changed |= patch_unity_object_fields(
                npc_rows
                    .get_mut(npc_id)
                    .ok_or_else(|| format!("m_pNpcTable.m_pNpcData[{npc_id}] does not exist"))?,
                patch_object.get("npcData").and_then(JsonValue::as_object),
            )?;
            changed |= patch_unity_object_fields(
                npc_target_rows_mut(npc_table, "m_pNpcStringData")
                    .and_then(|rows| rows.get_mut(npc_id))
                    .ok_or_else(|| {
                        format!("m_pNpcTable.m_pNpcStringData[{npc_id}] does not exist")
                    })?,
                patch_object
                    .get("stringData")
                    .and_then(JsonValue::as_object),
            )?;
            changed |= patch_unity_object_fields(
                npc_target_rows_mut(npc_table, "m_pNpcBarkerData")
                    .and_then(|rows| rows.get_mut(npc_id))
                    .ok_or_else(|| {
                        format!("m_pNpcTable.m_pNpcBarkerData[{npc_id}] does not exist")
                    })?,
                patch_object
                    .get("barkerData")
                    .and_then(JsonValue::as_object),
            )?;
            changed |= patch_npc_linked_row_fields(
                npc_table,
                npc_id,
                &npc_row_snapshot,
                "m_pNpcMeshData",
                "m_iMesh",
                patch_object.get("meshData").and_then(JsonValue::as_object),
            )?;
            changed |= patch_npc_linked_row_fields(
                npc_table,
                npc_id,
                &npc_row_snapshot,
                "m_pNpcIconData",
                "m_iIcon1",
                patch_object.get("iconData").and_then(JsonValue::as_object),
            )?;
        }

        changed |= patch_table_section_row_fields(
            &mut body,
            "m_pAnimationTable.m_pNpcData",
            npc_id,
            patch_object
                .get("animationData")
                .and_then(JsonValue::as_object),
        )?;

        if !changed {
            return Err("No friendly NPC fields were changed.".to_string());
        }

        let name = patch_object
            .get("stringData")
            .and_then(JsonValue::as_object)
            .and_then(|fields| fields.get("m_strName"))
            .and_then(JsonValue::as_str)
            .unwrap_or("friendly_npc");
        let patch_name = format!("friendly_npc_{}__{}", npc_id, safe_segment(name));
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
            "Staged friendly NPC patch: {}",
            target.to_string_lossy()
        ))
    })
}

pub(in super::super) fn load_staged_npc_icon_patch(
    project: &Path,
    npc_id: usize,
) -> Result<
    (
        PathBuf,
        JsonValue,
        PathBuf,
        JsonValue,
        fusionforge::UnityValue,
    ),
    String,
> {
    let manifest_path = staged_npc_import_manifest_for_id(project, npc_id)?;
    let manifest = serde_json::from_str::<JsonValue>(
        &fs::read_to_string(&manifest_path)
            .map_err(|err| format!("{}: {err}", manifest_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    let context = npc_import_manifest_context(project, &manifest);
    let patch_path = context
        .table_data_patch_path
        .filter(|path| path.is_file())
        .ok_or_else(|| format!("NPC {npc_id} staged manifest has no TableData patch file"))?;
    let patch_document = serde_json::from_str::<JsonValue>(
        &fs::read_to_string(&patch_path)
            .map_err(|err| format!("{}: {err}", patch_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", patch_path.display()))?;
    let body = json_to_unity_value(patch_document.get("value").unwrap_or(&patch_document), 0)?;
    Ok((manifest_path, manifest, patch_path, patch_document, body))
}

pub(in super::super) fn retarget_staged_npc_import_table_patch(
    project: &Path,
    npc_id: usize,
    patch_path: &Path,
    copied_sections: &[String],
) -> Result<(), String> {
    let mut manifests = Vec::new();
    collect_npc_import_manifest_paths(&project.join("npcs"), &mut manifests);
    let relative_patch = patch_path
        .strip_prefix(project)
        .unwrap_or(patch_path)
        .to_string_lossy()
        .replace('\\', "/");
    for manifest_path in manifests {
        let Ok(data) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(mut document) = serde_json::from_str::<JsonValue>(&data) else {
            continue;
        };
        if document
            .pointer("/blueprint/npcId")
            .and_then(JsonValue::as_u64)
            != Some(npc_id as u64)
        {
            continue;
        }
        let Some(table_patch) = document
            .get_mut("tableDataPatch")
            .and_then(JsonValue::as_object_mut)
        else {
            continue;
        };
        table_patch.insert("file".to_string(), json!(relative_patch));
        let sections = table_patch
            .entry("copiedSections".to_string())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| format!("{} copiedSections is not an array", manifest_path.display()))?;
        for section in copied_sections {
            if !sections.iter().any(|value| value.as_str() == Some(section)) {
                sections.push(json!(section));
            }
        }
        let updated = serde_json::to_string_pretty(&document).map_err(|err| err.to_string())?;
        fs::write(&manifest_path, format!("{updated}\n"))
            .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    }
    Ok(())
}

pub(in super::super) fn save_table_data_section_patch(
    project_dir: String,
    bundle_path: String,
    asset: String,
    path_id: i64,
    name: String,
    section_path: String,
    offset: usize,
    edits: Vec<TableDataCellPatch>,
) -> EditorResult<TableDataSectionPatchResult> {
    run_table_data_task(move || {
        save_table_data_section_patch_impl(
            project_dir,
            bundle_path,
            asset,
            path_id,
            name,
            section_path,
            offset,
            edits,
        )
    })
}

pub(in super::super) fn save_table_data_section_patch_impl(
    project_dir: String,
    bundle_path: String,
    asset: String,
    path_id: i64,
    name: String,
    section_path: String,
    offset: usize,
    edits: Vec<TableDataCellPatch>,
) -> EditorResult<TableDataSectionPatchResult> {
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
    let section = table_value_at_path_mut(&mut body, &section_path)
        .ok_or_else(|| format!("section was not found: {section_path}"))?;
    let table_rows = section
        .as_array_mut()
        .ok_or_else(|| format!("{section_path} is not an array"))?;
    let mut edited_rows = BTreeSet::new();
    for edit in &edits {
        let row_index = offset
            .checked_add(edit.row)
            .ok_or_else(|| "row patch offset overflowed".to_string())?;
        if row_index >= table_rows.len() {
            return Err(format!(
                "row patch index {row_index} exceeds {} rows",
                table_rows.len()
            ));
        }
        let value = json_to_unity_value(&edit.value, 0)?;
        if edit.field == "__value" {
            table_rows[row_index] = value;
        } else {
            let row = table_rows[row_index]
                .as_object_mut()
                .ok_or_else(|| format!("row {row_index} is not an object"))?;
            row.insert(edit.field.clone(), value);
        }
        edited_rows.insert(row_index);
    }
    let saved_rows = edited_rows.len();

    let object_name = object_name(&body);
    let patch_name = if name.trim().is_empty() {
        object_name.clone()
    } else {
        name
    };
    let document = json!({
        "format": "fftools.tabledata-object.v1",
        "sourceBundle": bundle.to_string_lossy(),
        "container": container_name_from_bundle(&bundle),
        "asset": asset_file.name,
        "pathId": path_id,
        "name": object_name,
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
    Ok(TableDataSectionPatchResult {
        path: target.to_string_lossy().to_string(),
        offset,
        rows: saved_rows,
    })
}

pub(in super::super) fn stage_table_data_patch(
    project_dir: String,
    bundle_path: String,
    asset: String,
    path_id: i64,
    name: String,
    patch_path: String,
) -> EditorResult<String> {
    let project = PathBuf::from(project_dir);
    let source = PathBuf::from(&patch_path);
    if !source.is_file() {
        return Err(EditorError::MissingPath(patch_path).to_string());
    }
    let source_document = serde_json::from_str::<JsonValue>(
        &fs::read_to_string(&source).map_err(|err| EditorError::Io(err).to_string())?,
    )
    .map_err(|err| EditorError::Json(err).to_string())?;
    if let Some(document_asset) = source_document.get("asset").and_then(JsonValue::as_str) {
        if document_asset != asset {
            return Err(format!(
                "Patch JSON targets asset {document_asset}, selected asset is {asset}"
            ));
        }
    }
    if let Some(document_path_id) = source_document.get("pathId").and_then(JsonValue::as_i64) {
        if document_path_id != path_id {
            return Err(format!(
                "Patch JSON targets pathId {document_path_id}, selected pathId is {path_id}"
            ));
        }
    }
    let target = write_table_data_patch_document(
        &project,
        &bundle_path,
        &asset,
        path_id,
        &name,
        &source_document,
    )?;
    Ok(format!(
        "Staged TableData patch: {}",
        target.to_string_lossy()
    ))
}

pub(in super::super) fn table_data_patch_file_parent_and_name(relative: &str) -> (&str, &str) {
    relative
        .rsplit_once('/')
        .map(|(parent, name)| (parent, name))
        .unwrap_or(("", relative))
}

pub(in super::super) fn write_table_data_patch_document(
    project: &Path,
    bundle_path: &str,
    asset: &str,
    path_id: i64,
    name: &str,
    document: &JsonValue,
) -> Result<PathBuf, String> {
    let mut patch_config = ensure_patch_project_config(project)?;
    let container = container_name_from_bundle(Path::new(bundle_path));
    let table_data_root = project.join("tabledata");
    let relative_file = PathBuf::from(safe_segment(&container))
        .join(safe_segment(asset))
        .join(format!("{path_id}__{}.json", safe_segment(&name)));
    let target = table_data_root.join(&relative_file);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    let relative_file_string = relative_file.to_string_lossy().replace('\\', "/");
    let document_data =
        serde_json::to_string_pretty(document).map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(&target, format!("{document_data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;

    let manifest_path = table_data_root.join("manifest.json");
    let mut manifest = if manifest_path.exists() {
        serde_json::from_str::<JsonValue>(
            &fs::read_to_string(&manifest_path).map_err(|err| EditorError::Io(err).to_string())?,
        )
        .unwrap_or_else(|_| json!({ "format": "fftools.tabledata-patch.v1", "entries": [] }))
    } else {
        json!({ "format": "fftools.tabledata-patch.v1", "entries": [] })
    };
    manifest["format"] = json!("fftools.tabledata-patch.v1");
    let entries = manifest["entries"]
        .as_array_mut()
        .ok_or_else(|| "TableData manifest entries is not an array".to_string())?;
    let obsolete_files = entries
        .iter()
        .filter(|entry| {
            entry.get("container").and_then(JsonValue::as_str) == Some(container.as_str())
                && entry.get("asset").and_then(JsonValue::as_str) == Some(asset)
                && entry.get("pathId").and_then(JsonValue::as_i64) == Some(path_id)
        })
        .filter_map(manifest_entry_file)
        .filter(|file| file != &relative_file_string)
        .collect::<Vec<_>>();
    entries.retain(|entry| {
        entry.get("container") != Some(&json!(container))
            || entry.get("asset") != Some(&json!(asset))
            || entry.get("pathId") != Some(&json!(path_id))
    });
    entries.push(json!({
        "container": container,
        "asset": asset,
        "pathId": path_id,
        "name": name,
        "file": relative_file_string,
    }));
    let manifest_data = serde_json::to_string_pretty(&manifest)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(&manifest_path, format!("{manifest_data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    for file in obsolete_files {
        let path = path_from_normalized_relative(&table_data_root, &file)?;
        if path.is_file() {
            fs::remove_file(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        }
    }
    prune_unreferenced_table_data_patches(project)?;
    patch_config["PatchTableData"] = json!(true);
    patch_config["TableDataDir"] = json!("tabledata");
    let patch_data = serde_json::to_string_pretty(&patch_config)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(project.join("ffpatch.json"), format!("{patch_data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    Ok(target)
}

pub(in super::super) fn patch_table_data_assets_native(
    extracted_dir: &Path,
    table_data_dir: &Path,
    entries: &[JsonValue],
    container: &str,
) -> Result<bool, String> {
    let mut by_asset = BTreeMap::<String, BTreeMap<i64, PathBuf>>::new();
    for entry in entries {
        if entry.get("container").and_then(JsonValue::as_str) != Some(container) {
            continue;
        }
        let asset = entry
            .get("asset")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "TableData manifest entry has no asset".to_string())?;
        let path_id = entry
            .get("pathId")
            .or_else(|| entry.get("path_id"))
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| "TableData manifest entry has no pathId".to_string())?;
        let file = entry
            .get("file")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "TableData manifest entry has no file".to_string())?;
        by_asset
            .entry(asset.to_string())
            .or_default()
            .insert(path_id, table_data_dir.join(file));
    }
    let mut patched = 0usize;
    for (asset, replacements) in by_asset {
        patched += patch_table_data_asset_file(extracted_dir, &asset, &replacements)?;
    }
    Ok(patched > 0)
}

pub(in super::super) fn ensure_patch_project_config(project: &Path) -> Result<serde_json::Value, String> {
    fs::create_dir_all(project).map_err(|err| err.to_string())?;
    let translation_path = project_translation_index_path(project);
    let legacy_translation_path = project.join("translation.json");
    if let Some(parent) = translation_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    if !translation_path.exists() {
        if legacy_translation_path.exists() {
            fs::copy(&legacy_translation_path, &translation_path).map_err(|err| err.to_string())?;
        } else {
            fs::write(
                &translation_path,
                "{\n  \"format\": \"fftools.translation.v1\",\n  \"entries\": []\n}\n",
            )
            .map_err(|err| err.to_string())?;
        }
    }
    if legacy_translation_path.exists() {
        fs::remove_file(&legacy_translation_path).map_err(|err| err.to_string())?;
    }
    let patch_config_path = project.join("ffpatch.json");
    let mut patch_config = if patch_config_path.exists() {
        serde_json::from_str::<serde_json::Value>(
            &fs::read_to_string(&patch_config_path).map_err(|err| err.to_string())?,
        )
        .unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };
    patch_config["TranslationJson"] = json!(PROJECT_TRANSLATION_INDEX_RELATIVE);
    if patch_config.get("PreloadNpcBundles").is_none() {
        patch_config["PreloadNpcBundles"] = json!(false);
    }
    if patch_config.get("LoadNpcBundlesInWorld").is_none() {
        patch_config["LoadNpcBundlesInWorld"] = json!(true);
    }
    if patch_config.get("NpcBundleManifestSections").is_none() {
        patch_config["NpcBundleManifestSections"] = json!(["m_FreeZone", "m_PaidZone"]);
    }
    if patch_config.get("BundleLayout").is_none() {
        patch_config["BundleLayout"] = json!({
            "Enabled": false,
            "MaxPartBytes": 25165824,
            "MaxBundlesPerPart": 12,
            "CoreSharedName": "CoreShared.resourceFile",
            "TutorialAudioName": "TutorialAudio.resourceFile",
            "UiAudioName": "UiAudio.resourceFile",
            "NpcVoiceSharedName": "NpcVoiceShared.resourceFile",
            "WorldSharedPrefix": "WorldShared",
            "DongResourcesPrefix": "DongResources",
            "NpcPackPrefix": "NPC_Pack",
            "HnpcPackPrefix": "HNPC_Pack",
            "NanoPackPrefix": "Nano_Pack",
            "PlayerCharacterPackPrefix": "PlayerCharacter_Pack",
            "ItemsPackPrefix": "Items_Pack",
            "IconsPackPrefix": "Icons_Pack",
            "DedupeExact": true,
            "Report": "bundle-layout-report.json"
        });
    }
    Ok(patch_config)
}

pub(in super::super) fn seed_consolidated_npc_table_patch(
    project: &Path,
    patch_path: &Path,
) -> Result<(String, i64, JsonValue, fusionforge::UnityValue), String> {
    let document = serde_json::from_str::<JsonValue>(
        &fs::read_to_string(patch_path)
            .map_err(|err| format!("{}: {err}", patch_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", patch_path.display()))?;
    let asset = document
        .get("asset")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("{} has no asset", patch_path.display()))?
        .to_string();
    let path_id = document
        .get("pathId")
        .or_else(|| document.get("path_id"))
        .and_then(JsonValue::as_i64)
        .ok_or_else(|| format!("{} has no pathId", patch_path.display()))?;
    let source_bundle = document
        .get("sourceBundle")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("{} has no sourceBundle", patch_path.display()))?;
    let body =
        read_table_data_object_body(project, &PathBuf::from(source_bundle), &asset, path_id)?;
    Ok((asset, path_id, document, body))
}

pub(in super::super) fn seed_active_table_data_patch(
    project: &Path,
) -> Result<Option<(String, i64, JsonValue, fusionforge::UnityValue)>, String> {
    for patch_path in table_data_manifest_patch_paths(project)? {
        if patch_path.exists() {
            return seed_consolidated_npc_table_patch(project, &patch_path).map(Some);
        }
    }
    Ok(None)
}

pub(in super::super) fn apply_npc_build_table_patch_step(
    project: &Path,
    manifest_label: &str,
    consolidated_table_patch: &mut Option<(String, i64, JsonValue, fusionforge::UnityValue)>,
    build_blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
) -> Result<(), String> {
    if consolidated_table_patch.is_none() {
        if let Some(patch_path) = context
            .table_data_patch_path
            .as_ref()
            .filter(|path| path.exists())
        {
            let seed_project = project.to_path_buf();
            let seed_patch_path = patch_path.clone();
            *consolidated_table_patch = Some(
                run_table_data_task(move || {
                    seed_consolidated_npc_table_patch(&seed_project, &seed_patch_path)
                })
                .map_err(|err| {
                    format!("{manifest_label}: seed_consolidated_npc_table_patch: {err}")
                })?,
            );
        }
    }
    if let Some((asset, path_id, document, body)) = consolidated_table_patch.take() {
        let refresh_project = project.to_path_buf();
        let refresh_blueprint = build_blueprint.clone();
        let refresh_context = context.clone();
        let (body, _copied_sections) = run_table_data_task(move || {
            let mut body = body;
            let copied = apply_npc_import_to_table_body(
                &refresh_project,
                &refresh_blueprint,
                &refresh_context,
                &mut body,
            )?;
            Ok((body, copied))
        })
        .map_err(|err| format!("{manifest_label}: apply_npc_import_to_table_body: {err}"))?;
        *consolidated_table_patch = Some((asset, path_id, document, body));
    }
    Ok(())
}

pub(in super::super) fn npc_import_dir_from_patch_config(project: &Path, patch_config: &JsonValue) -> PathBuf {
    json_string(patch_config, "NpcImportDir")
        .map(|value| resolve_project_path(project, &value))
        .unwrap_or_else(|| project.join("npcs"))
}

pub(in super::super) fn patch_strings_native(
    extracted_dir: &Path,
    translation_path: &Path,
    container: &str,
) -> Result<bool, String> {
    let status_path = build_status_path(extracted_dir, "string", container);
    let args = vec![
        extracted_dir.to_string_lossy().to_string(),
        translation_path.to_string_lossy().to_string(),
        "--container".to_string(),
        container.to_string(),
        "--allow-missing".to_string(),
        "--status-file".to_string(),
        status_path.to_string_lossy().to_string(),
    ];
    if container.eq_ignore_ascii_case("TableData.resourceFile") {
        run_table_data_task(move || fusionforge::build_patch::patch_unity_asset_strings(&args))?;
    } else {
        fusionforge::build_patch::patch_unity_asset_strings(&args)?;
    }
    Ok(status_patched_count(&status_path, "applied") > 0)
}

pub(in super::super) fn patch_main_fonts_native(
    extracted_dir: &Path,
    _out_dir: &Path,
    project: &Path,
    config: &ClientPatchConfig,
) -> Result<(), String> {
    if config.font_ttf.is_none()
        && config.font_dir.is_none()
        && config.font_face.is_none()
        && config.font_vertical_offset.is_none()
        && !config.preserve_ascii_glyphs
    {
        return Ok(());
    }
    let input = extracted_dir.join("sharedassets0.assets");
    if !input.exists() {
        return Ok(());
    }
    let report_temp = native_build_temp_dir("font_patch_report")?;
    let report = report_temp.path().join("font_patch_report.json");
    let mut args = vec![
        input.to_string_lossy().to_string(),
        input.to_string_lossy().to_string(),
        "--report".to_string(),
        report.to_string_lossy().to_string(),
    ];
    if let Some(font_ttf) = config.font_ttf.as_ref() {
        args.push("--ttf-font".to_string());
        args.push(
            resolve_project_path(project, font_ttf)
                .to_string_lossy()
                .to_string(),
        );
    }
    if let Some(font_dir) = config.font_dir.as_ref() {
        let font_dir = resolve_project_path(project, font_dir);
        args.push("--ttf-font-dir".to_string());
        args.push(font_dir.to_string_lossy().to_string());
        let manifest = font_dir.join("manifest.json");
        if manifest.exists() {
            args.push("--font-manifest".to_string());
            args.push(manifest.to_string_lossy().to_string());
        }
    }
    if let Some(face) = config.font_face.as_ref() {
        args.push("--font-face".to_string());
        args.push(face.clone());
    }
    if let Some(offset) = config.font_vertical_offset {
        if offset != 0.0 {
            args.push("--vertical-offset".to_string());
            args.push(offset.to_string());
        }
    }
    if config.preserve_ascii_glyphs {
        args.push("--preserve-ascii-glyphs".to_string());
    }
    fusionforge::build_patch::patch_gui_fonts(&args)
}
