use super::super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct NpcGeneratedIconSaveResult {
    pub(in super::super) path: String,
    pub(in super::super) relative_file: String,
    pub(in super::super) width: u32,
    pub(in super::super) height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct NpcImportStageResult {
    pub(in super::super) npc_id: usize,
    pub(in super::super) table_data_patch_path: String,
    pub(in super::super) import_manifest_path: String,
    pub(in super::super) bundle_name: String,
    pub(in super::super) copied_sections: Vec<String>,
    pub(in super::super) warnings: Vec<String>,
}

pub(in super::super) fn save_tile_document(path: String, mut document: serde_json::Value) -> EditorResult<()> {
    migrate_world_document_json(&mut document);
    let data = serde_json::to_string_pretty(&document)
        .map_err(|err| EditorError::Json(err).to_string())?;
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    fs::write(&path, format!("{data}\n")).map_err(|err| EditorError::Io(err).to_string())
}

pub(in super::super) fn npc_source_import_name(source_dir: &Path) -> String {
    let source_name = source_dir
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("source-build");
    format!("{}-{}", safe_segment(source_name), path_hash(source_dir))
}

pub(in super::super) fn staged_npc_import_ids(project: &Path) -> BTreeSet<usize> {
    fn scan_dir(dir: &Path, ids: &mut BTreeSet<usize>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_dir(&path, ids);
                continue;
            }
            if !path.is_file() {
                continue;
            }
            let Ok(data) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<JsonValue>(&data) else {
                continue;
            };
            let id = value
                .get("blueprint")
                .and_then(|blueprint| blueprint.get("npcId"))
                .and_then(JsonValue::as_u64)
                .or_else(|| value.get("npcId").and_then(JsonValue::as_u64));
            if let Some(id) = id {
                ids.insert(id as usize);
            }
        }
    }

    let mut ids = BTreeSet::new();
    scan_dir(&project.join("npcs"), &mut ids);
    ids
}

pub(in super::super) fn allocate_import_npc_id(project: &Path, npc_table: &fusionforge::UnityValue) -> usize {
    let mut used = npc_table_occupied_ids(npc_table);
    used.extend(staged_npc_import_ids(project));
    let mut npc_id = used
        .iter()
        .next_back()
        .copied()
        .unwrap_or(999)
        .saturating_add(1);
    while used.contains(&npc_id) {
        npc_id = npc_id.saturating_add(1);
    }
    npc_id
}

pub(in super::super) fn npc_copy_source_row<'a>(
    source_table: &'a fusionforge::UnityValue,
    section: &str,
    index: usize,
) -> Option<&'a fusionforge::UnityValue> {
    npc_table_array(source_table, section).get(index)
}

pub(in super::super) fn copy_npc_linked_row(
    source_table: &fusionforge::UnityValue,
    target_table: &mut fusionforge::UnityValue,
    section: &str,
    source_index: usize,
    preferred_target_index: usize,
) -> Result<Option<usize>, String> {
    let Some(source_row) = npc_copy_source_row(source_table, section, source_index).cloned() else {
        return Ok(None);
    };
    let empty = empty_npc_row_like(&source_row);
    if source_row == empty && source_index == 0 {
        return Ok(Some(0));
    }
    let Some(target_rows) = npc_target_rows_mut(target_table, section) else {
        return Ok(None);
    };
    while target_rows.len() <= preferred_target_index {
        target_rows.push(empty.clone());
    }
    let target_row = &target_rows[preferred_target_index];
    if *target_row == source_row || *target_row == empty {
        target_rows[preferred_target_index] = source_row;
        return Ok(Some(preferred_target_index));
    }

    if let Some((index, row)) = target_rows
        .iter_mut()
        .enumerate()
        .find(|(_, row)| **row == source_row || **row == empty)
    {
        *row = source_row;
        return Ok(Some(index));
    }

    target_rows.push(source_row);
    Ok(Some(target_rows.len() - 1))
}

pub(in super::super) fn copy_npc_linked_string_field(
    source_table: &fusionforge::UnityValue,
    target_table: &mut fusionforge::UnityValue,
    npc_row: &mut fusionforge::UnityValue,
    field: &str,
    preferred_target_index: usize,
    copied: &mut Vec<String>,
) -> Result<(), String> {
    let Some(source_index) = unity_usize_field(npc_row, field).filter(|index| *index > 0) else {
        return Ok(());
    };
    let Some(target_index) = copy_npc_linked_row(
        source_table,
        target_table,
        "m_pNpcStringData",
        source_index,
        preferred_target_index,
    )?
    else {
        return Ok(());
    };
    set_unity_usize_field(npc_row, field, target_index);
    copied.push("m_pNpcStringData".to_string());
    Ok(())
}

pub(in super::super) fn copy_npc_table_rows(
    source_table: &fusionforge::UnityValue,
    target_table: &mut fusionforge::UnityValue,
    source_id: usize,
    target_id: usize,
) -> Result<Vec<String>, String> {
    source_table
        .as_object()
        .ok_or_else(|| "source m_pNpcTable is not an object".to_string())?;
    target_table
        .as_object_mut()
        .ok_or_else(|| "target m_pNpcTable is not an object".to_string())?;
    let mut copied = Vec::new();

    let mut npc_row = npc_copy_source_row(source_table, "m_pNpcData", source_id)
        .cloned()
        .ok_or_else(|| format!("source NPC data row {source_id} does not exist"))?;
    let source_name_index = unity_usize_field(&npc_row, "m_iNpcName").unwrap_or(source_id);
    set_unity_usize_field(&mut npc_row, "m_iNpcNumber", target_id);
    set_unity_usize_field(&mut npc_row, "m_iNpcName", target_id);
    let source_comment_index = unity_usize_field(&npc_row, "m_iComment");
    if source_comment_index == Some(source_id) || source_comment_index == Some(source_name_index) {
        set_unity_usize_field(&mut npc_row, "m_iComment", target_id);
    }

    if copy_npc_row_to_index(
        source_table,
        target_table,
        "m_pNpcStringData",
        source_name_index,
        target_id,
    )? {
        copied.push("m_pNpcStringData".to_string());
    }

    if let Some(source_comment_index) =
        source_comment_index.filter(|index| *index != source_id && *index != source_name_index)
    {
        copy_npc_linked_string_field(
            source_table,
            target_table,
            &mut npc_row,
            "m_iComment",
            source_comment_index,
            &mut copied,
        )?;
    }
    for field in [
        "m_iActiveSkill1String",
        "m_iActiveSkill2String",
        "m_iSupportSkillString",
        "m_iCorruptionString",
        "m_iMegaString",
    ] {
        let preferred_target_index = unity_usize_field(&npc_row, field).unwrap_or(target_id);
        copy_npc_linked_string_field(
            source_table,
            target_table,
            &mut npc_row,
            field,
            preferred_target_index,
            &mut copied,
        )?;
    }

    if let Some(source_index) =
        unity_usize_field(&npc_row, "m_iBarkerNumber").filter(|index| *index > 0)
    {
        if let Some(target_index) = copy_npc_linked_row(
            source_table,
            target_table,
            "m_pNpcBarkerData",
            source_index,
            target_id,
        )? {
            set_unity_usize_field(&mut npc_row, "m_iBarkerNumber", target_index);
            copied.push("m_pNpcBarkerData".to_string());
        }
    }

    if let Some(source_index) = unity_usize_field(&npc_row, "m_iMesh") {
        let preferred_target_index = npc_table_array(target_table, "m_pNpcMeshData").len();
        if let Some(target_index) = copy_npc_linked_row(
            source_table,
            target_table,
            "m_pNpcMeshData",
            source_index,
            preferred_target_index,
        )? {
            set_unity_usize_field(&mut npc_row, "m_iMesh", target_index);
            copied.push("m_pNpcMeshData".to_string());
        }
    }
    if let Some(source_index) = unity_usize_field(&npc_row, "m_iIcon1") {
        let preferred_target_index = npc_table_array(target_table, "m_pNpcIconData").len();
        if let Some(target_index) = copy_npc_linked_row(
            source_table,
            target_table,
            "m_pNpcIconData",
            source_index,
            preferred_target_index,
        )? {
            set_unity_usize_field(&mut npc_row, "m_iIcon1", target_index);
            copied.push("m_pNpcIconData".to_string());
        }
    }
    if let Some(source_index) =
        unity_usize_field(&npc_row, "m_iServiceNumber").filter(|index| *index > 0)
    {
        if let Some(target_index) = copy_npc_linked_row(
            source_table,
            target_table,
            "m_pNpcServiceData",
            source_index,
            source_index,
        )? {
            set_unity_usize_field(&mut npc_row, "m_iServiceNumber", target_index);
            copied.push("m_pNpcServiceData".to_string());
        }
    }

    let Some(target_rows) = npc_target_rows_mut(target_table, "m_pNpcData") else {
        return Err("target m_pNpcTable.m_pNpcData is not an array".to_string());
    };
    let empty = empty_npc_row_like(&npc_row);
    while target_rows.len() <= target_id {
        target_rows.push(empty.clone());
    }
    target_rows[target_id] = npc_row;
    copied.push("m_pNpcData".to_string());

    copied.sort();
    copied.dedup();
    if copied.is_empty() {
        return Err(format!(
            "No NPC table rows were copied from source NPC {source_id}"
        ));
    }
    Ok(copied)
}

pub(in super::super) fn save_npc_generated_icon(
    project_dir: String,
    relative_file: String,
    data_url: String,
    expected_size: u32,
) -> EditorResult<NpcGeneratedIconSaveResult> {
    use image::GenericImageView;

    let project = PathBuf::from(project_dir);
    if !project.exists() || !project.is_dir() {
        return Err(EditorError::MissingPath(project.to_string_lossy().to_string()).to_string());
    }
    let relative_path = safe_project_relative_path(&relative_file)?;
    let png = decode_png_data_url(&data_url)?;
    let image = image::load_from_memory(&png)
        .map_err(|err| format!("could not verify generated ICON PNG: {err}"))?;
    let (width, height) = image.dimensions();
    if width != expected_size || height != expected_size {
        return Err(format!(
            "Generated ICON must be {expected_size}x{expected_size}; got {width}x{height}"
        ));
    }
    let target = project.join(&relative_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    fs::write(&target, png).map_err(|err| EditorError::Io(err).to_string())?;
    Ok(NpcGeneratedIconSaveResult {
        path: target.to_string_lossy().to_string(),
        relative_file: relative_path.to_string_lossy().replace('\\', "/"),
        width,
        height,
    })
}

pub(in super::super) fn stage_npc_import_from_source(
    project_dir: String,
    target_bundle_path: String,
    target_asset: String,
    target_path_id: i64,
    source_bundle_path: String,
    source_project_dir: String,
    source_asset: String,
    source_path_id: i64,
    source_npc_id: usize,
    mut blueprint: NpcBlueprint,
) -> EditorResult<NpcImportStageResult> {
    run_table_data_task(move || {
        use fusionforge::object_name;

        let project = PathBuf::from(project_dir);
        if !project.exists() || !project.is_dir() {
            return Err(
                EditorError::MissingPath(project.to_string_lossy().to_string()).to_string(),
            );
        }

        let target_bundle = PathBuf::from(&target_bundle_path);
        let target_extract_dir = extract_bundle_cached(&project, &target_bundle)?;
        let target_asset_path = extracted_asset_path(&target_extract_dir, &target_asset)?;
        let target_asset_file = fusionforge::Asset::from_path(&target_asset_path)?;
        let target_info = target_asset_file
            .objects
            .get(&target_path_id)
            .ok_or_else(|| {
                format!(
                    "{}#{} was not found",
                    target_asset_file.name, target_path_id
                )
            })?;
        let mut target_body = if let Some(body) = table_data_patch_body_for_object(
            &project,
            &target_bundle_path,
            &target_asset,
            target_path_id,
        )? {
            body
        } else {
            target_asset_file.read_object(0, target_info)?
        };

        let source_project = PathBuf::from(&source_project_dir);
        let source_bundle = PathBuf::from(&source_bundle_path);
        let source_extract_dir = extract_bundle_cached(&source_project, &source_bundle)?;
        let source_asset_path = extracted_asset_path(&source_extract_dir, &source_asset)?;
        let source_asset_file = fusionforge::Asset::from_path(&source_asset_path)?;
        let source_info = source_asset_file
            .objects
            .get(&source_path_id)
            .ok_or_else(|| {
                format!(
                    "{}#{} was not found",
                    source_asset_file.name, source_path_id
                )
            })?;
        let source_body = source_asset_file.read_object(0, source_info)?;

        let source_table = npc_table_from_body(&source_body)
            .ok_or_else(|| "source TableData object has no m_pNpcTable".to_string())?;
        let target_table = npc_table_from_body_mut(&mut target_body)
            .ok_or_else(|| "target TableData object has no m_pNpcTable".to_string())?;
        let npc_id = requested_or_allocated_npc_id(&project, target_table, blueprint.npc_id)?;
        blueprint.npc_id = npc_id as i64;
        blueprint.template_npc_id = Some(source_npc_id as i64);
        let mut copied_sections =
            copy_npc_table_rows(source_table, target_table, source_npc_id, npc_id)?;
        if apply_blueprint_npc_profile(target_table, npc_id, &blueprint.profile)? {
            copied_sections.push("m_pNpcData".to_string());
        }
        if apply_blueprint_npc_text(target_table, npc_id, &blueprint)? {
            copied_sections.push("m_pNpcStringData".to_string());
            copied_sections.push("m_pNpcBarkerData".to_string());
        }
        if apply_blueprint_npc_assets(target_table, npc_id, &blueprint)? {
            copied_sections.push("m_pNpcMeshData".to_string());
            copied_sections.push("m_pNpcIconData".to_string());
        }
        copied_sections.sort();
        copied_sections.dedup();
        let warnings = vec![
            "Source bundle dependencies were materialized into this NPC's assets folder; Home build packs the standalone resourceFile from those project assets.".to_string(),
        ];

        let patch_name = format!("npc_import_{}__{}", npc_id, safe_segment(&blueprint.name));
        let document = json!({
            "format": "fftools.tabledata-object.v1",
            "sourceBundle": target_bundle.to_string_lossy(),
            "container": container_name_from_bundle(&target_bundle),
            "asset": target_asset_file.name,
            "pathId": target_path_id,
            "name": object_name(&target_body),
            "objectType": target_asset_file.object_type_name(target_info),
            "value": unity_value_to_json(&target_body),
        });
        let table_data_patch_path = write_table_data_patch_document(
            &project,
            &target_bundle_path,
            &target_asset,
            target_path_id,
            &patch_name,
            &document,
        )?;
        let import_manifest_path = write_npc_import_manifest(
            &project,
            &blueprint,
            &source_bundle_path,
            &source_asset,
            source_path_id,
            source_npc_id,
            &table_data_patch_path,
            &copied_sections,
            &warnings,
        )?;
        Ok(NpcImportStageResult {
            npc_id,
            table_data_patch_path: table_data_patch_path.to_string_lossy().to_string(),
            import_manifest_path: import_manifest_path.to_string_lossy().to_string(),
            bundle_name: npc_blueprint_bundle_name(&blueprint),
            copied_sections,
            warnings,
        })
    })
}

pub(in super::super) fn materialize_npc_import_source_assets(
    project: &Path,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
) -> Result<(NpcBlueprint, Vec<String>), String> {
    let asset_stage_root = npc_build_assets_root(project, blueprint);
    let (staged_blueprint, staged_paths) = stage_npc_manifest_source_assets_to_root(
        project,
        blueprint,
        context,
        &asset_stage_root,
        true,
    )?;
    write_npc_build_assets_manifest(project, &staged_blueprint, &staged_paths)?;
    Ok((staged_blueprint, staged_paths))
}

pub(in super::super) fn copy_file_if_different(source: &Path, target: &Path) -> Result<(), String> {
    let source_bytes = fs::read(source).map_err(|err| format!("{}: {err}", source.display()))?;
    if fs::read(target).ok().as_ref() == Some(&source_bytes) {
        return Ok(());
    }
    if target.is_file() {
        let metadata =
            fs::metadata(target).map_err(|err| format!("{}: {err}", target.display()))?;
        let mut permissions = metadata.permissions();
        if permissions.readonly() {
            permissions.set_readonly(false);
            fs::set_permissions(target, permissions)
                .map_err(|err| format!("{}: {err}", target.display()))?;
        }
    }
    fs::write(target, source_bytes).map_err(|err| format!("{}: {err}", target.display()))
}

#[derive(Debug, Clone)]
pub(in super::super) struct NpcGeneratedIconImport {
    pub(in super::super) template_path: String,
    pub(in super::super) target_path: String,
    pub(in super::super) target_name: String,
    pub(in super::super) texture: fusionforge::modding::ImportedTexture,
}

pub(in super::super) fn npc_generated_icon_import(
    project: &Path,
    blueprint: &NpcBlueprint,
) -> Result<Option<NpcGeneratedIconImport>, String> {
    let Some(icon) = blueprint.generated_icon.as_ref() else {
        return Ok(None);
    };
    let relative_path = safe_project_relative_path(&icon.file)?;
    let png_path = project.join(relative_path);
    if !png_path.is_file() {
        return Err(format!(
            "Generated NPC ICON PNG does not exist: {}",
            png_path.display()
        ));
    }
    let template_path = normalized_asset_path(&icon.template_asset_path);
    if template_path.is_empty() || template_path.eq_ignore_ascii_case("null") {
        return Err("Generated NPC ICON needs templateAssetPath.".to_string());
    }
    let target_path = normalized_asset_path(
        icon.asset_path
            .trim()
            .is_empty()
            .then_some(blueprint.icon_asset.as_deref().unwrap_or_default())
            .unwrap_or(icon.asset_path.as_str()),
    );
    if target_path.is_empty() || target_path.eq_ignore_ascii_case("null") {
        return Err("Generated NPC ICON needs assetPath.".to_string());
    }
    let target_name = asset_stem_from_container_path(&target_path)
        .unwrap_or_else(|| format!("npcicon_{}", blueprint.npc_id.max(0)));
    let texture = fusionforge::modding::ImportedTexture::from_png_path(
        &png_path,
        Some(target_name.clone()),
        true,
    )?;
    if icon.size > 0 && (texture.width != icon.size || texture.height != icon.size) {
        return Err(format!(
            "Generated NPC ICON {} is {}x{}, expected {}x{}",
            png_path.display(),
            texture.width,
            texture.height,
            icon.size,
            icon.size
        ));
    }
    Ok(Some(NpcGeneratedIconImport {
        template_path,
        target_path,
        target_name,
        texture,
    }))
}

pub(in super::super) fn write_build_source_snapshot(
    out_dir: &Path,
    snapshot: &BuildSourceSnapshot,
) -> Result<(), String> {
    let data =
        serde_json::to_string_pretty(snapshot).map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(out_dir.join(BUILD_SOURCE_INDEX_FILE), format!("{data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())
}

pub(in super::super) fn copy_source_relative_file(source: &Path, out_dir: &Path, relative: &str) -> Result<(), String> {
    let source_path = path_from_normalized_relative(source, relative)?;
    let target_path = path_from_normalized_relative(out_dir, relative)?;
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    fs::copy(&source_path, &target_path).map_err(|err| {
        format!(
            "{} -> {}: {err}",
            source_path.display(),
            target_path.display()
        )
    })?;
    Ok(())
}

pub(in super::super) fn apply_npc_import_to_table_body(
    project: &Path,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
    target_body: &mut fusionforge::UnityValue,
) -> Result<Vec<String>, String> {
    let target_npc_id = usize::try_from(blueprint.npc_id)
        .map_err(|_| format!("Invalid target NPC id {}", blueprint.npc_id))?;
    let target_body_snapshot = target_body.clone();
    let (source_body, source_copy_npc_id) = if let Some(patch_path) = context
        .table_data_patch_path
        .as_ref()
        .filter(|path| path.exists())
    {
        (table_data_patch_value(patch_path, 0)?, target_npc_id)
    } else if let (Some(source_bundle), Some(source_asset), Some(source_path_id)) = (
        context.source_table_data_bundle.as_ref(),
        context.source_asset.as_ref(),
        context.source_path_id,
    ) {
        let source_body =
            read_table_data_object_body(project, source_bundle, source_asset, source_path_id)?;
        let source_npc_id = context.source_npc_id.unwrap_or(target_npc_id);
        let source_has_row = npc_table_from_body(&source_body)
            .is_some_and(|table| npc_copy_source_row(table, "m_pNpcData", source_npc_id).is_some());
        if source_has_row {
            (source_body, source_npc_id)
        } else {
            let target_has_row = npc_table_from_body(&target_body_snapshot).is_some_and(|table| {
                npc_copy_source_row(table, "m_pNpcData", target_npc_id).is_some()
            });
            if target_has_row {
                (target_body_snapshot, target_npc_id)
            } else {
                return Err(format!(
                    "source NPC data row {source_npc_id} does not exist and consolidated row {target_npc_id} is unavailable"
                ));
            }
        }
    } else if npc_table_from_body(&target_body_snapshot)
        .is_some_and(|table| npc_copy_source_row(table, "m_pNpcData", target_npc_id).is_some())
    {
        (target_body_snapshot, target_npc_id)
    } else {
        return Ok(Vec::new());
    };
    let source_table = npc_table_from_body(&source_body)
        .ok_or_else(|| "source TableData object has no m_pNpcTable".to_string())?;
    let target_table = npc_table_from_body_mut(target_body)
        .ok_or_else(|| "target TableData patch has no m_pNpcTable".to_string())?;
    let mut copied = copy_npc_table_rows(
        source_table,
        target_table,
        source_copy_npc_id,
        target_npc_id,
    )?;
    if apply_blueprint_npc_profile(target_table, target_npc_id, &blueprint.profile)? {
        copied.push("m_pNpcData".to_string());
    }
    if apply_blueprint_npc_text(target_table, target_npc_id, blueprint)? {
        copied.push("m_pNpcStringData".to_string());
        copied.push("m_pNpcBarkerData".to_string());
    }
    if apply_blueprint_npc_assets(target_table, target_npc_id, blueprint)? {
        copied.push("m_pNpcMeshData".to_string());
        copied.push("m_pNpcIconData".to_string());
    }
    copied.sort();
    copied.dedup();
    Ok(copied)
}

#[derive(Debug, Clone)]
pub(in super::super) struct ExternalResourceImportSpec {
    pub(in super::super) source: PathBuf,
    pub(in super::super) bundle_name: String,
    pub(in super::super) routes: BTreeSet<String>,
    pub(in super::super) sections: Vec<String>,
    pub(in super::super) project_bundle: PathBuf,
}

pub(in super::super) fn external_import_value<'a>(value: &'a JsonValue, keys: &[&str]) -> Option<&'a JsonValue> {
    keys.iter().find_map(|key| value.get(*key))
}
