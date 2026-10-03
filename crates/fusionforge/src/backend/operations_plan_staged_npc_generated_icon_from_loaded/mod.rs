use super::super::*;

pub(in super::super) fn append_npc_section_row(
    target_table: &mut fusionforge::UnityValue,
    section: &str,
    row: fusionforge::UnityValue,
) -> Result<usize, String> {
    let rows = npc_target_rows_mut(target_table, section)
        .ok_or_else(|| format!("m_pNpcTable.{section} is not an array"))?;
    let index = rows.len();
    rows.push(row);
    Ok(index)
}

pub(in super::super) fn create_blank_npc_table_rows(
    target_table: &mut fusionforge::UnityValue,
    target_id: usize,
    blueprint: &NpcBlueprint,
) -> Result<Vec<String>, String> {
    let mut copied = Vec::new();
    let mesh_index = npc_table_array(target_table, "m_pNpcMeshData").len();
    let icon_index = npc_table_array(target_table, "m_pNpcIconData").len();
    let has_greeting = blueprint_has_greeting(blueprint);

    let mut npc_row = blank_npc_row_from_section(target_table, "m_pNpcData");
    set_existing_unity_i64_field(&mut npc_row, "m_iNpcNumber", target_id as i64);
    set_existing_unity_i64_field(&mut npc_row, "m_iNpcName", target_id as i64);
    set_existing_unity_i64_field(&mut npc_row, "m_iComment", target_id as i64);
    set_existing_unity_i64_field(&mut npc_row, "m_iMesh", mesh_index as i64);
    set_existing_unity_i64_field(&mut npc_row, "m_iIcon1", icon_index as i64);
    set_existing_unity_i64_field(
        &mut npc_row,
        "m_iBarkerNumber",
        if has_greeting { target_id as i64 } else { 0 },
    );
    patch_existing_unity_object_fields(&mut npc_row, &blueprint.profile)?;

    let mut string_row = blank_npc_row_from_section(target_table, "m_pNpcStringData");
    set_existing_unity_string_field(&mut string_row, "m_strName", &blueprint.name);
    set_existing_unity_string_field(
        &mut string_row,
        "m_strComment",
        blueprint.comment.as_deref().unwrap_or_default(),
    );
    set_existing_unity_string_field(
        &mut string_row,
        "m_strComment1",
        blueprint.comment1.as_deref().unwrap_or_default(),
    );
    set_existing_unity_string_field(&mut string_row, "m_strComment2", &blueprint.internal_name);

    let mut mesh_row = blank_npc_row_from_section(target_table, "m_pNpcMeshData");
    let model = blueprint_asset_value(blueprint.model_asset.as_ref(), "null");
    let texture = blueprint_asset_value(blueprint.texture_asset.as_ref(), "null");
    set_existing_unity_string_field(&mut mesh_row, "m_pstrMMeshModelString", &model);
    set_existing_unity_string_field(&mut mesh_row, "m_pstrMTextureString", &texture);
    set_existing_unity_string_field(&mut mesh_row, "m_pstrMTextureString2", "null");
    set_existing_unity_string_field(&mut mesh_row, "m_pstrFMeshModelString", "null");
    set_existing_unity_string_field(&mut mesh_row, "m_pstrFTextureString", "null");
    set_existing_unity_string_field(&mut mesh_row, "m_pstrFTextureString2", "null");

    let mut icon_row = blank_npc_row_from_section(target_table, "m_pNpcIconData");
    let team = blueprint
        .profile
        .get("m_iTeam")
        .and_then(JsonValue::as_i64)
        .unwrap_or_default();
    let icon_type = if team == 2 { 8 } else { 4 };
    set_existing_unity_i64_field(&mut icon_row, "m_iIconType", icon_type);
    set_existing_unity_i64_field(
        &mut icon_row,
        "m_iIconNumber",
        blueprint_icon_number(blueprint, target_id),
    );

    write_npc_row_at_index(target_table, "m_pNpcStringData", target_id, string_row)?;
    copied.push("m_pNpcStringData".to_string());
    if has_greeting {
        let mut barker_row = blank_npc_row_from_section(target_table, "m_pNpcBarkerData");
        set_existing_unity_string_field(
            &mut barker_row,
            "m_strName",
            blueprint
                .greeting_name
                .as_deref()
                .unwrap_or(&blueprint.name),
        );
        set_existing_unity_string_field(
            &mut barker_row,
            "m_strComment",
            blueprint.greeting_comment.as_deref().unwrap_or_default(),
        );
        set_existing_unity_string_field(
            &mut barker_row,
            "m_strComment1",
            blueprint.greeting_comment1.as_deref().unwrap_or_default(),
        );
        set_existing_unity_string_field(
            &mut barker_row,
            "m_strComment2",
            blueprint.greeting_key.as_deref().unwrap_or_default(),
        );
        write_npc_row_at_index(target_table, "m_pNpcBarkerData", target_id, barker_row)?;
        copied.push("m_pNpcBarkerData".to_string());
    }
    append_npc_section_row(target_table, "m_pNpcMeshData", mesh_row)?;
    copied.push("m_pNpcMeshData".to_string());
    append_npc_section_row(target_table, "m_pNpcIconData", icon_row)?;
    copied.push("m_pNpcIconData".to_string());
    write_npc_row_at_index(target_table, "m_pNpcData", target_id, npc_row)?;
    copied.push("m_pNpcData".to_string());
    copied.sort();
    copied.dedup();
    Ok(copied)
}

pub(in super::super) fn npc_icon_type_for_prefix(prefix: &str) -> Option<i64> {
    match prefix {
        "wpnicon" => Some(0),
        "nanoicon" => Some(1),
        "skillicon" => Some(2),
        "cosicon" => Some(3),
        "npcicon" => Some(4),
        "nanoready" => Some(5),
        "questitemicon" => Some(6),
        "generalitemicon" => Some(7),
        "mobicon" => Some(8),
        "fusionicon" => Some(9),
        "hnpcicon" => Some(10),
        "transport" => Some(11),
        "vehicle" => Some(12),
        _ => None,
    }
}

pub(in super::super) fn npc_icon_prefix_for_type(icon_type: i64) -> Option<&'static str> {
    match icon_type {
        0 => Some("wpnicon"),
        1 => Some("nanoicon"),
        2 => Some("skillicon"),
        3 => Some("cosicon"),
        4 => Some("npcicon"),
        5 => Some("nanoready"),
        6 => Some("questitemicon"),
        7 => Some("generalitemicon"),
        8 => Some("mobicon"),
        9 => Some("fusionicon"),
        10 => Some("hnpcicon"),
        11 => Some("transport"),
        12 => Some("vehicle"),
        _ => None,
    }
}

pub(in super::super) fn json_nested_string(value: &JsonValue, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(in super::super) fn set_json_nested_string(value: &mut JsonValue, path: &[&str], text: &str) -> bool {
    let Some((last, parents)) = path.split_last() else {
        return false;
    };
    let mut current = value;
    for key in parents {
        let Some(next) = current.get_mut(*key) else {
            return false;
        };
        current = next;
    }
    let Some(object) = current.as_object_mut() else {
        return false;
    };
    if object
        .get(*last)
        .and_then(JsonValue::as_str)
        .is_some_and(|value| value == text)
    {
        return false;
    }
    object.insert((*last).to_string(), json!(text));
    true
}

pub(in super::super) fn option_has_text(value: &Option<String>) -> bool {
    value.as_deref().is_some_and(|text| !text.trim().is_empty())
}

pub(in super::super) fn blueprint_has_greeting(blueprint: &NpcBlueprint) -> bool {
    option_has_text(&blueprint.greeting_name)
        || option_has_text(&blueprint.greeting_comment)
        || option_has_text(&blueprint.greeting_comment1)
        || option_has_text(&blueprint.greeting_key)
}

pub(in super::super) fn ensure_npc_barker_row(
    target_table: &mut fusionforge::UnityValue,
    target_id: usize,
) -> Result<usize, String> {
    if let Some(index) = npc_table_array(target_table, "m_pNpcData")
        .get(target_id)
        .and_then(|row| unity_usize_field(row, "m_iBarkerNumber"))
        .filter(|index| *index > 0)
    {
        return Ok(index);
    }

    {
        let barker_rows = npc_target_rows_mut(target_table, "m_pNpcBarkerData")
            .ok_or_else(|| "m_pNpcTable.m_pNpcBarkerData is not an array".to_string())?;
        let empty = barker_rows
            .first()
            .map(empty_npc_row_like)
            .unwrap_or_else(|| fusionforge::UnityValue::Object(BTreeMap::new()));
        while barker_rows.len() <= target_id {
            barker_rows.push(empty.clone());
        }
    }

    let npc_rows = npc_target_rows_mut(target_table, "m_pNpcData")
        .ok_or_else(|| "m_pNpcTable.m_pNpcData is not an array".to_string())?;
    let npc_row = npc_rows
        .get_mut(target_id)
        .ok_or_else(|| format!("m_pNpcTable.m_pNpcData[{target_id}] does not exist"))?;
    set_unity_usize_field(npc_row, "m_iBarkerNumber", target_id);
    Ok(target_id)
}

pub(in super::super) fn npc_icon_row_type_and_number(row: &fusionforge::UnityValue) -> Option<(i64, i64)> {
    Some((
        row.get("m_iIconType")
            .and_then(fusionforge::UnityValue::as_i64)?,
        row.get("m_iIconNumber")
            .and_then(fusionforge::UnityValue::as_i64)?,
    ))
}

pub(in super::super) fn npc_icon_row_reference_count(
    target_table: &fusionforge::UnityValue,
    icon_index: usize,
) -> usize {
    npc_table_array(target_table, "m_pNpcData")
        .iter()
        .filter(|row| unity_usize_field(row, "m_iIcon1") == Some(icon_index))
        .count()
}

pub(in super::super) fn allocate_unique_npc_icon_number(
    target_table: &fusionforge::UnityValue,
    icon_type: i64,
    preferred_number: i64,
    reusable_row_index: Option<usize>,
) -> Result<i64, String> {
    allocate_unique_npc_icon_number_with_reserved(
        target_table,
        icon_type,
        preferred_number,
        reusable_row_index,
        &BTreeSet::new(),
    )
}

pub(in super::super) fn allocate_unique_npc_icon_number_with_reserved(
    target_table: &fusionforge::UnityValue,
    icon_type: i64,
    preferred_number: i64,
    reusable_row_index: Option<usize>,
    reserved_numbers: &BTreeSet<i64>,
) -> Result<i64, String> {
    let mut icon_number = preferred_number.max(0);
    loop {
        let occupied = reserved_numbers.contains(&icon_number)
            || npc_table_array(target_table, "m_pNpcIconData")
                .iter()
                .enumerate()
                .any(|(index, row)| {
                    Some(index) != reusable_row_index
                        && npc_icon_row_type_and_number(row) == Some((icon_type, icon_number))
                });
        if !occupied {
            return Ok(icon_number);
        }
        icon_number = icon_number
            .checked_add(1)
            .ok_or_else(|| format!("No free icon number remains for icon type {icon_type}"))?;
    }
}

pub(in super::super) fn bind_unique_generated_npc_icon_row(
    target_table: &mut fusionforge::UnityValue,
    target_id: usize,
    icon_type: i64,
    preferred_number: i64,
) -> Result<NpcGeneratedIconBinding, String> {
    npc_icon_prefix_for_type(icon_type)
        .ok_or_else(|| format!("Unsupported NPC icon type {icon_type}"))?;
    let current_icon_index = npc_table_array(target_table, "m_pNpcData")
        .get(target_id)
        .and_then(|row| unity_usize_field(row, "m_iIcon1"));
    let current_row_exists = current_icon_index
        .is_some_and(|index| index < npc_table_array(target_table, "m_pNpcIconData").len());
    let current_row_is_shared = current_icon_index
        .filter(|_| current_row_exists)
        .is_some_and(|index| npc_icon_row_reference_count(target_table, index) > 1);
    let reusable_row_index =
        current_icon_index.filter(|_| current_row_exists && !current_row_is_shared);
    let icon_number = allocate_unique_npc_icon_number(
        target_table,
        icon_type,
        preferred_number,
        reusable_row_index,
    )?;

    let row_index = if let Some(index) = reusable_row_index {
        let row = npc_target_rows_mut(target_table, "m_pNpcIconData")
            .and_then(|rows| rows.get_mut(index))
            .ok_or_else(|| format!("m_pNpcTable.m_pNpcIconData[{index}] does not exist"))?;
        set_unity_i64_field(row, "m_iIconType", icon_type);
        set_unity_i64_field(row, "m_iIconNumber", icon_number);
        index
    } else {
        let mut row = current_icon_index
            .and_then(|index| npc_table_array(target_table, "m_pNpcIconData").get(index))
            .cloned()
            .unwrap_or_else(|| blank_npc_row_from_section(target_table, "m_pNpcIconData"));
        set_unity_i64_field(&mut row, "m_iIconType", icon_type);
        set_unity_i64_field(&mut row, "m_iIconNumber", icon_number);
        append_npc_section_row(target_table, "m_pNpcIconData", row)?
    };

    let npc_row = npc_target_rows_mut(target_table, "m_pNpcData")
        .and_then(|rows| rows.get_mut(target_id))
        .ok_or_else(|| format!("m_pNpcTable.m_pNpcData[{target_id}] does not exist"))?;
    set_unity_usize_field(npc_row, "m_iIcon1", row_index);

    Ok(NpcGeneratedIconBinding {
        row_index,
        icon_number,
        cloned_shared_row: current_row_is_shared,
    })
}

pub(in super::super) fn staged_icon_numbers_for_other_manifests(
    project: &Path,
    current_manifest: &Path,
    icon_type: i64,
) -> BTreeSet<i64> {
    let mut manifests = Vec::new();
    collect_npc_import_manifest_paths(&project.join("npcs"), &mut manifests);
    let current_manifest =
        fs::canonicalize(current_manifest).unwrap_or_else(|_| current_manifest.to_path_buf());
    let mut numbers = BTreeSet::new();
    for path in manifests {
        if fs::canonicalize(&path).unwrap_or_else(|_| path.clone()) == current_manifest {
            continue;
        }
        let Ok(data) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(document) = serde_json::from_str::<JsonValue>(&data) else {
            continue;
        };
        for pointer in [
            "/blueprint/iconAsset",
            "/resources/iconTexture/asset",
            "/blueprint/generatedIcon/assetPath",
            "/resources/iconTexture/generated/assetPath",
        ] {
            let Some(path) = document.pointer(pointer).and_then(JsonValue::as_str) else {
                continue;
            };
            let Some(icon) = parse_npc_icon_asset_path(path) else {
                continue;
            };
            if npc_icon_type_for_prefix(&icon.prefix) == Some(icon_type) {
                numbers.insert(icon.number);
            }
        }
    }
    numbers
}

pub(in super::super) fn plan_staged_npc_generated_icon_from_loaded(
    project: &Path,
    manifest_path: &Path,
    manifest: &JsonValue,
    target_table: &fusionforge::UnityValue,
    npc_id: usize,
    preferred_icon_type: Option<i64>,
) -> Result<NpcGeneratedIconPlan, String> {
    let blueprint = manifest
        .get("blueprint")
        .cloned()
        .ok_or_else(|| format!("{} has no blueprint", manifest_path.display()))
        .and_then(|value| {
            serde_json::from_value::<NpcBlueprint>(value).map_err(|err| err.to_string())
        })?;
    if blueprint.npc_id != npc_id as i64 {
        return Err(format!(
            "{} belongs to NPC {}, expected NPC {npc_id}",
            manifest_path.display(),
            blueprint.npc_id
        ));
    }

    let npc_row = npc_table_array(target_table, "m_pNpcData")
        .get(npc_id)
        .ok_or_else(|| format!("m_pNpcTable.m_pNpcData[{npc_id}] does not exist"))?;
    let current_icon_index = unity_usize_field(npc_row, "m_iIcon1");
    let current_icon_row = current_icon_index
        .and_then(|index| npc_table_array(target_table, "m_pNpcIconData").get(index));
    let current_icon_type = current_icon_row
        .and_then(|row| row.get("m_iIconType"))
        .and_then(fusionforge::UnityValue::as_i64)
        .filter(|icon_type| npc_icon_prefix_for_type(*icon_type).is_some());
    let team = npc_row
        .get("m_iTeam")
        .and_then(fusionforge::UnityValue::as_i64)
        .unwrap_or(1);
    let manifest_icon_type = blueprint
        .generated_icon
        .as_ref()
        .map(|icon| icon.asset_path.as_str())
        .or(blueprint.icon_asset.as_deref())
        .and_then(parse_npc_icon_asset_path)
        .and_then(|icon| npc_icon_type_for_prefix(&icon.prefix));
    let icon_type = preferred_icon_type
        .or(manifest_icon_type)
        .or(current_icon_type)
        .unwrap_or(if team == 2 { 8 } else { 4 });
    npc_icon_prefix_for_type(icon_type)
        .ok_or_else(|| format!("Unsupported NPC icon type {icon_type}"))?;

    let row_exists = current_icon_index
        .is_some_and(|index| index < npc_table_array(target_table, "m_pNpcIconData").len());
    let row_is_shared = current_icon_index
        .filter(|_| row_exists)
        .is_some_and(|index| npc_icon_row_reference_count(target_table, index) > 1);
    let reusable_row_index = current_icon_index.filter(|_| row_exists && !row_is_shared);
    let existing_generated_icon = blueprint.generated_icon.as_ref().filter(|icon| {
        parse_npc_icon_asset_path(&icon.asset_path)
            .and_then(|path| npc_icon_type_for_prefix(&path.prefix))
            == Some(icon_type)
    });
    let existing_generated_number = existing_generated_icon
        .and_then(|icon| parse_npc_icon_asset_path(&icon.asset_path))
        .map(|icon| icon.number);
    let reserved_numbers =
        staged_icon_numbers_for_other_manifests(project, manifest_path, icon_type);
    let icon_number = allocate_unique_npc_icon_number_with_reserved(
        target_table,
        icon_type,
        existing_generated_number.unwrap_or(npc_id as i64),
        reusable_row_index,
        &reserved_numbers,
    )?;
    let asset_path = npc_icon_runtime_asset_path(icon_type, icon_number)
        .ok_or_else(|| format!("Could not format NPC icon type {icon_type}/{icon_number}"))?;
    let file_name = asset_path.rsplit('/').next().unwrap_or("npcicon.png");
    let relative_file = existing_generated_icon
        .map(|icon| icon.file.trim())
        .filter(|file| !file.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            let path = manifest_path.parent()?.join("generated").join(file_name);
            path_relative_to_project(project, &path)
        })
        .ok_or_else(|| format!("Could not derive generated icon path for NPC {npc_id}"))?;
    let current_table_icon_path = current_icon_row
        .and_then(npc_icon_row_type_and_number)
        .and_then(|(icon_type, icon_number)| npc_icon_runtime_asset_path(icon_type, icon_number));
    let template_asset_path = blueprint
        .generated_icon
        .as_ref()
        .map(|icon| icon.template_asset_path.trim())
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            blueprint
                .icon_asset
                .clone()
                .filter(|path| !path.trim().is_empty())
        })
        .or(current_table_icon_path)
        .ok_or_else(|| format!("NPC {npc_id} has no icon Texture2D template path"))?;
    let icon_bundle = blueprint
        .icon_bundle
        .clone()
        .filter(|path| !path.trim().is_empty())
        .or_else(|| json_nested_string(manifest, &["resources", "iconTexture", "bundle"]))
        .ok_or_else(|| format!("NPC {npc_id} has no icon Texture2D template bundle"))?;

    Ok(NpcGeneratedIconPlan {
        npc_id,
        icon_type,
        icon_number,
        icon_row_index: reusable_row_index
            .unwrap_or_else(|| npc_table_array(target_table, "m_pNpcIconData").len()),
        asset_path,
        relative_file,
        template_asset_path: normalized_asset_path(&template_asset_path),
        icon_bundle,
        size: 64,
        cloned_shared_row: row_is_shared,
    })
}

pub(crate) fn plan_staged_npc_generated_icon(
    project_dir: String,
    npc_id: usize,
    preferred_icon_type: Option<i64>,
) -> EditorResult<NpcGeneratedIconPlan> {
    run_table_data_task(move || {
        let project = PathBuf::from(project_dir);
        let (manifest_path, manifest, _, _, body) = load_staged_npc_icon_patch(&project, npc_id)?;
        let table = npc_table_from_body(&body)
            .ok_or_else(|| "Staged TableData patch has no m_pNpcTable".to_string())?;
        plan_staged_npc_generated_icon_from_loaded(
            &project,
            &manifest_path,
            &manifest,
            table,
            npc_id,
            preferred_icon_type,
        )
    })
}

pub(in super::super) fn stage_npc_blank_table_data(
    project_dir: String,
    target_bundle_path: String,
    target_asset: String,
    target_path_id: i64,
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
        if blueprint.name.trim().is_empty() {
            return Err("NPC name is required for m_pNpcStringData.m_strName.".to_string());
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

        let target_table = npc_table_from_body_mut(&mut target_body)
            .ok_or_else(|| "target TableData object has no m_pNpcTable".to_string())?;
        let npc_id = requested_or_allocated_npc_id(&project, target_table, blueprint.npc_id)?;
        blueprint.npc_id = npc_id as i64;
        blueprint.template_npc_id = None;
        let copied_sections = create_blank_npc_table_rows(target_table, npc_id, &blueprint)?;

        let patch_name = format!("npc_blank_{}__{}", npc_id, safe_segment(&blueprint.name));
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
        Ok(NpcImportStageResult {
            npc_id,
            table_data_patch_path: table_data_patch_path.to_string_lossy().to_string(),
            import_manifest_path: String::new(),
            bundle_name: String::new(),
            copied_sections,
            warnings: vec![
                "TableData row only. Server NPCs.json/mobs.json/vendor/warp JSON and standalone NPC bundle manifests are not written by this action.".to_string(),
            ],
        })
    })
}

pub(in super::super) fn stage_npc_table_data_edit(
    project_dir: String,
    target_bundle_path: String,
    target_asset: String,
    target_path_id: i64,
    npc_id: usize,
    blueprint: NpcBlueprint,
) -> EditorResult<String> {
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

        let target_table = npc_table_from_body_mut(&mut target_body)
            .ok_or_else(|| "target TableData object has no m_pNpcTable".to_string())?;
        if npc_table_array(target_table, "m_pNpcData")
            .get(npc_id)
            .is_none()
        {
            return Err(format!("m_pNpcTable.m_pNpcData[{npc_id}] does not exist"));
        }
        let mut copied_sections = Vec::new();
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
        if copied_sections.is_empty() {
            return Err("No NPC TableData fields were changed.".to_string());
        }

        let patch_name = format!("npc_edit_{}__{}", npc_id, safe_segment(&blueprint.name));
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
        let target = write_table_data_patch_document(
            &project,
            &target_bundle_path,
            &target_asset,
            target_path_id,
            &patch_name,
            &document,
        )?;
        retarget_staged_npc_import_table_patch(&project, npc_id, &target, &copied_sections)?;
        Ok(format!(
            "Staged NPC TableData edit ({}): {}",
            copied_sections.join(", "),
            target.to_string_lossy()
        ))
    })
}
