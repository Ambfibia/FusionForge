use super::super::*;

pub(in super::super) fn apply_known_decompiler_patches(data: &str, path: &Path) -> String {
    let mut fixed = data.replace("(ref ", "(");
    fixed = fixed.replace(
        "DongLoader.GetPositionColor(base.transform.position, ref fog, ref sky, ref filterColor, ref fogDepth);",
        "DongLoader.GetPositionColor(base.transform.position, out fog, out sky, out filterColor, out fogDepth);",
    );
    fixed = rewrite_known_out_argument_calls(&fixed);
    fixed = fixed.replace(
        "localized.attachStr[localized.local]",
        "localized.attachStr[(int)localized.local]",
    );
    fixed = fixed.replace(
        "localized.attachFolderNameStr[localized.local]",
        "localized.attachFolderNameStr[(int)localized.local]",
    );
    fixed = fixed.replace(
        "using WindowFunction = GUI.WindowFunction;",
        "using WindowFunction = UnityEngine.GUI.WindowFunction;",
    );
    fixed = fixed.replace("(object)(Type)0", "(object)0");
    fixed = fixed.replace("(object)(Type)1", "(object)1");
    fixed = fixed.replace(
        "platformMoveType = currentPlatform.moveType;",
        "platformMoveType = (cnAvatarStatus.MoveType)currentPlatform.moveType;",
    );
    fixed = fixed.replace(
        "new delPlayCharacterEffectPriority(ReceivePlayCharacterEffectPriority)",
        "ReceivePlayCharacterEffectPriority",
    );
    fixed = fixed.replace("_003F ", "Material ");
    fixed = repair_empty_gui_color_temporaries(&fixed);
    fixed = remove_standalone_csharp_expression_statement(&fixed, "GUI.color");
    fixed = remove_standalone_csharp_expression_statement(&fixed, "GUI.enabled");
    fixed = remove_standalone_csharp_expression_statement(&fixed, "GUI.changed");
    fixed = remove_standalone_csharp_expression_statement(&fixed, "GUI.skin");
    fixed = remove_known_csharp_noop_statements(&fixed);
    fixed = rewrite_csharp_accessor_calls(&fixed);
    fixed = replace_csharp_call_with_argument(&fixed, "Object.op_Implicit");
    fixed = replace_csharp_call_with_argument(&fixed, "TrackedReference.op_Implicit");
    fixed = replace_csharp_call_with_argument(&fixed, "GUIStyle.op_Implicit");
    fixed = replace_csharp_call_with_argument(&fixed, "Vector2.op_Implicit");
    fixed = rewrite_collision_flags_masks(&fixed);
    fixed = rewrite_value_type_constructor_calls(&fixed);
    fixed = strip_redundant_value_type_casts(&fixed);
    fixed = rewrite_unknown_isinst_placeholders(&fixed);
    fixed = dedupe_csharp_using_aliases(&fixed);

    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if matches!(
        file_name,
        "EquipPopup.cs" | "GameFrame.cs" | "InventoryManagerScript.cs"
    ) {
        fixed = fixed.replace("(object)(Type)0", "(object)0");
        fixed = fixed.replace("(object)(Type)1", "(object)1");
    }
    fixed
}

pub(in super::super) fn apply_compiled_script_patches(
    project: &Path,
    manifest_path: &Path,
    extracted_main: &Path,
) -> EditorResult<usize> {
    let manifest = load_script_patch_manifest_from_path(manifest_path)?;
    let mut applied = 0usize;
    for entry in manifest.entries {
        let Some(compiled_dll) = entry.compiled_dll.as_deref() else {
            continue;
        };
        let Some(target_file) = entry.target_file.as_deref() else {
            continue;
        };
        let source = resolve_project_path(project, compiled_dll);
        if !source.is_file() {
            return Err(format!(
                "Compiled script DLL was not found: {}",
                source.display()
            ));
        }
        let target = extracted_main.join(target_file);
        if !target.is_file() {
            return Err(format!(
                "Target script DLL was not found in main.unity3d extraction: {}",
                target.display()
            ));
        }
        fs::copy(&source, &target)
            .map_err(|err| format!("{} -> {}: {err}", source.display(), target.display()))?;
        applied += 1;
    }
    Ok(applied)
}

pub(in super::super) fn apply_blueprint_npc_profile(
    target_table: &mut fusionforge::UnityValue,
    target_id: usize,
    profile: &BTreeMap<String, JsonValue>,
) -> Result<bool, String> {
    if profile.is_empty() {
        return Ok(false);
    }
    let mut fields = serde_json::Map::new();
    for (field, value) in profile {
        if value.is_null() || blueprint_profile_protected_field(field) {
            continue;
        }
        fields.insert(field.clone(), value.clone());
    }
    if fields.is_empty() {
        return Ok(false);
    }
    let rows = npc_target_rows_mut(target_table, "m_pNpcData")
        .ok_or_else(|| "m_pNpcTable.m_pNpcData is not an array".to_string())?;
    let row = rows
        .get_mut(target_id)
        .ok_or_else(|| format!("m_pNpcTable.m_pNpcData[{target_id}] does not exist"))?;
    patch_unity_object_fields(row, Some(&fields))
}

pub(in super::super) fn sync_staged_npc_icons(project_dir: String) -> EditorResult<JsonValue> {
    let project = PathBuf::from(project_dir);
    let project_root = fs::canonicalize(&project).unwrap_or_else(|_| project.clone());
    let staged_npc_root = project_root.join("npcs");
    let npc_root = project.join("npcs");
    if !npc_root.is_dir() {
        return Err(format!(
            "NPC import directory does not exist: {}",
            npc_root.display()
        ));
    }

    let mut manifest_paths = Vec::new();
    collect_npc_import_manifest_paths(&npc_root, &mut manifest_paths);
    manifest_paths.sort();
    manifest_paths.dedup();

    let mut items = Vec::new();
    let mut checked = 0usize;
    let mut manifests_updated = 0usize;
    let mut asset_files_updated = 0usize;
    let mut rename_operations = 0usize;
    let mut external_icon_bundles_skipped = 0usize;

    for manifest_path in manifest_paths {
        let manifest_label = manifest_path
            .strip_prefix(&project)
            .unwrap_or(&manifest_path)
            .to_string_lossy()
            .replace('\\', "/");
        let mut warnings = Vec::<String>::new();
        let mut manifest = match fs::read_to_string(&manifest_path)
            .map_err(|err| format!("{}: {err}", manifest_path.display()))
            .and_then(|data| {
                serde_json::from_str::<JsonValue>(&data)
                    .map_err(|err| format!("{}: {err}", manifest_path.display()))
            }) {
            Ok(manifest) => manifest,
            Err(err) => {
                warnings.push(err);
                items.push(json!({
                    "manifest": manifest_label,
                    "warnings": warnings,
                }));
                continue;
            }
        };
        if manifest.get("format").and_then(JsonValue::as_str) != Some("fftools.npc-import.v1") {
            continue;
        }
        checked += 1;

        let blueprint = match manifest
            .get("blueprint")
            .cloned()
            .ok_or_else(|| format!("{} has no blueprint", manifest_path.display()))
            .and_then(|value| {
                serde_json::from_value::<NpcBlueprint>(value)
                    .map_err(|err| format!("{}: {err}", manifest_path.display()))
            }) {
            Ok(blueprint) => blueprint,
            Err(err) => {
                warnings.push(err);
                items.push(json!({
                    "manifest": manifest_label,
                    "warnings": warnings,
                }));
                continue;
            }
        };

        let previous_icon_asset = blueprint.icon_asset.clone();
        let old_paths = staged_manifest_icon_paths(&manifest, &blueprint);
        let target_icon_asset =
            match staged_npc_table_icon_asset_path(&project, &manifest, &blueprint) {
                Ok(Some(path)) => path,
                Ok(None) => {
                    warnings.push("TableData icon path was not found for this NPC.".to_string());
                    items.push(json!({
                        "npcId": blueprint.npc_id,
                        "name": blueprint.name,
                        "manifest": manifest_label,
                        "previousIconAsset": previous_icon_asset,
                        "targetIconAsset": JsonValue::Null,
                        "manifestUpdated": false,
                        "assetFilesUpdated": 0,
                        "renameOperations": 0,
                        "warnings": warnings,
                    }));
                    continue;
                }
                Err(err) => {
                    warnings.push(err);
                    items.push(json!({
                        "npcId": blueprint.npc_id,
                        "name": blueprint.name,
                        "manifest": manifest_label,
                        "previousIconAsset": previous_icon_asset,
                        "targetIconAsset": JsonValue::Null,
                        "manifestUpdated": false,
                        "assetFilesUpdated": 0,
                        "renameOperations": 0,
                        "warnings": warnings,
                    }));
                    continue;
                }
            };

        let mut item_asset_files_updated = 0usize;
        let mut item_rename_operations = 0usize;
        let mut item_external_icon_bundle_skipped = false;
        if let Some(bundle_path) = staged_manifest_icon_bundle_path(&manifest, &blueprint) {
            let icon_source = resolve_project_path(&project, &bundle_path);
            let icon_source_root = fs::canonicalize(&icon_source).unwrap_or(icon_source.clone());
            if !icon_source_root.starts_with(&staged_npc_root) {
                item_external_icon_bundle_skipped = true;
                external_icon_bundles_skipped += 1;
            } else {
                match staged_icon_asset_files(&icon_source) {
                    Ok(files) if files.is_empty() => warnings.push(format!(
                        "Icon bundle source has no asset files: {}",
                        icon_source.display()
                    )),
                    Ok(files) => {
                        for asset_file in files {
                            match sync_staged_npc_icon_asset_file(
                                &asset_file,
                                &old_paths,
                                &target_icon_asset,
                            ) {
                                Ok(0) => {}
                                Ok(count) => {
                                    item_asset_files_updated += 1;
                                    item_rename_operations += count;
                                }
                                Err(err) => warnings.push(err),
                            }
                        }
                    }
                    Err(err) => warnings.push(err),
                }
            }
        } else {
            warnings.push("NPC manifest has no icon bundle path.".to_string());
        }

        let manifest_changed =
            sync_staged_npc_manifest_icon_paths(&mut manifest, &target_icon_asset);
        if manifest_changed {
            let data = serde_json::to_string_pretty(&manifest)
                .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
            fs::write(&manifest_path, format!("{data}\n"))
                .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
            manifests_updated += 1;
        }
        asset_files_updated += item_asset_files_updated;
        rename_operations += item_rename_operations;

        items.push(json!({
            "npcId": blueprint.npc_id,
            "name": blueprint.name,
            "manifest": manifest_label,
            "previousIconAsset": previous_icon_asset,
            "targetIconAsset": target_icon_asset,
            "manifestUpdated": manifest_changed,
            "assetFilesUpdated": item_asset_files_updated,
            "renameOperations": item_rename_operations,
            "externalIconBundleSkipped": item_external_icon_bundle_skipped,
            "warnings": warnings,
        }));
    }

    Ok(json!({
        "project": project.to_string_lossy().replace('\\', "/"),
        "checked": checked,
        "manifestsUpdated": manifests_updated,
        "assetFilesUpdated": asset_files_updated,
        "renameOperations": rename_operations,
        "externalIconBundlesSkipped": external_icon_bundles_skipped,
        "items": items,
    }))
}

pub(in super::super) fn apply_blueprint_npc_text(
    target_table: &mut fusionforge::UnityValue,
    target_id: usize,
    blueprint: &NpcBlueprint,
) -> Result<bool, String> {
    let mut changed = false;
    let (string_index, mut barker_index) = {
        let target_npc_row = npc_table_array(target_table, "m_pNpcData").get(target_id);
        (
            target_npc_row
                .and_then(|row| unity_usize_field(row, "m_iNpcName"))
                .unwrap_or(target_id),
            target_npc_row
                .and_then(|row| unity_usize_field(row, "m_iBarkerNumber"))
                .filter(|index| *index > 0),
        )
    };

    if barker_index.is_none() && blueprint_has_greeting(blueprint) {
        barker_index = Some(ensure_npc_barker_row(target_table, target_id)?);
        changed = true;
    }

    if let Some(string_row) = npc_target_rows_mut(target_table, "m_pNpcStringData")
        .and_then(|rows| rows.get_mut(string_index))
    {
        changed |=
            set_optional_unity_string(string_row, "m_strName", Some(blueprint.name.clone()))?;
        changed |=
            set_optional_unity_string(string_row, "m_strComment", blueprint.comment.clone())?;
        changed |=
            set_optional_unity_string(string_row, "m_strComment1", blueprint.comment1.clone())?;
        changed |= set_optional_unity_string(
            string_row,
            "m_strComment2",
            (!blueprint.internal_name.trim().is_empty())
                .then(|| blueprint.internal_name.trim().to_string()),
        )?;
    }

    if let Some(barker_row) = barker_index.and_then(|index| {
        npc_target_rows_mut(target_table, "m_pNpcBarkerData").and_then(|rows| rows.get_mut(index))
    }) {
        changed |=
            set_optional_unity_string(barker_row, "m_strName", blueprint.greeting_name.clone())?;
        changed |= set_optional_unity_string(
            barker_row,
            "m_strComment",
            blueprint.greeting_comment.clone(),
        )?;
        changed |= set_optional_unity_string(
            barker_row,
            "m_strComment1",
            blueprint.greeting_comment1.clone(),
        )?;
        changed |=
            set_optional_unity_string(barker_row, "m_strComment2", blueprint.greeting_key.clone())?;
    }
    Ok(changed)
}

pub(in super::super) fn apply_blueprint_npc_assets(
    target_table: &mut fusionforge::UnityValue,
    target_id: usize,
    blueprint: &NpcBlueprint,
) -> Result<bool, String> {
    let mut changed = false;
    let target_npc_row = npc_table_array(target_table, "m_pNpcData").get(target_id);
    let mesh_index = target_npc_row.and_then(|row| unity_usize_field(row, "m_iMesh"));
    let icon_index = target_npc_row.and_then(|row| unity_usize_field(row, "m_iIcon1"));
    let team = target_npc_row
        .and_then(|row| row.get("m_iTeam"))
        .and_then(fusionforge::UnityValue::as_i64)
        .unwrap_or(1);

    if let Some(mesh_row) = mesh_index.and_then(|index| {
        npc_target_rows_mut(target_table, "m_pNpcMeshData").and_then(|rows| rows.get_mut(index))
    }) {
        if let Some(model) = blueprint
            .model_asset
            .as_deref()
            .and_then(blueprint_asset_stem)
        {
            changed |= set_optional_unity_string(mesh_row, "m_pstrMMeshModelString", Some(model))?;
        }
        if let Some(texture) = blueprint
            .texture_asset
            .as_deref()
            .and_then(blueprint_asset_stem)
        {
            // The import wizard sometimes binds the *secondary* texture
            // (m_pstrMTextureString2, e.g. the fusion spawn texture
            // "spawn11_green") as the blueprint texture. Never overwrite a
            // valid copied main texture string with the row's own secondary
            // texture - that makes fusion mobs render with the spawn texture.
            let current_main =
                unity_string_field(Some(&*mesh_row), "m_pstrMTextureString").unwrap_or_default();
            let current_sub =
                unity_string_field(Some(&*mesh_row), "m_pstrMTextureString2").unwrap_or_default();
            let current_main_valid = !current_main.trim().is_empty()
                && !current_main.trim().eq_ignore_ascii_case("null");
            let texture_is_secondary = texture.eq_ignore_ascii_case(current_sub.trim())
                && !texture.eq_ignore_ascii_case(current_main.trim());
            if !(current_main_valid && texture_is_secondary) {
                changed |=
                    set_optional_unity_string(mesh_row, "m_pstrMTextureString", Some(texture))?;
            }
        }
        // Authoring NPCs copy their mesh row from an arbitrary template NPC;
        // neutralize leftover template strings (female variants, secondary
        // texture) that point at assets which do not exist in the new bundle.
        if npc_blueprint_uses_authoring_model_bundle(blueprint) {
            for field in [
                "m_pstrMTextureString2",
                "m_pstrFMeshModelString",
                "m_pstrFTextureString",
                "m_pstrFTextureString2",
            ] {
                let current = unity_string_field(Some(&*mesh_row), field).unwrap_or_default();
                if !current.trim().is_empty() && !current.trim().eq_ignore_ascii_case("null") {
                    changed |=
                        set_optional_unity_string(mesh_row, field, Some("null".to_string()))?;
                }
            }
        }
    }

    if blueprint
        .icon_asset
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || blueprint.generated_icon.is_some()
    {
        let icon_path = blueprint
            .generated_icon
            .as_ref()
            .map(|icon| icon.asset_path.as_str())
            .or(blueprint.icon_asset.as_deref());
        let current_icon_type = icon_index
            .and_then(|index| npc_table_array(target_table, "m_pNpcIconData").get(index))
            .and_then(|row| row.get("m_iIconType"))
            .and_then(fusionforge::UnityValue::as_i64);
        let parsed_icon = icon_path.and_then(parse_npc_icon_asset_path);
        let icon_type = parsed_icon
            .as_ref()
            .and_then(|path| npc_icon_type_for_prefix(&path.prefix))
            .or(current_icon_type
                .filter(|icon_type| npc_icon_prefix_for_type(*icon_type).is_some()))
            .unwrap_or(if team == 2 { 8 } else { 4 });

        if blueprint.generated_icon.is_some() {
            let preferred_number = parsed_icon
                .filter(|path| npc_icon_type_for_prefix(&path.prefix) == Some(icon_type))
                .map(|path| path.number)
                .unwrap_or_else(|| blueprint_icon_number(blueprint, target_id));
            bind_unique_generated_npc_icon_row(
                target_table,
                target_id,
                icon_type,
                preferred_number,
            )?;
        } else if let Some(icon_row) = icon_index.and_then(|index| {
            npc_target_rows_mut(target_table, "m_pNpcIconData").and_then(|rows| rows.get_mut(index))
        }) {
            set_unity_i64_field(icon_row, "m_iIconType", icon_type);
            set_unity_i64_field(
                icon_row,
                "m_iIconNumber",
                blueprint_icon_number(blueprint, target_id),
            );
        }
        changed = true;
    }

    Ok(changed)
}

pub(in super::super) fn sync_build_output_from_source_preserving(
    source: &Path,
    out_dir: &Path,
    preserved_output_files: &BTreeSet<String>,
    force_copy_source_files: bool,
) -> Result<BuildSourceSnapshot, String> {
    fs::create_dir_all(out_dir).map_err(|err| format!("{}: {err}", out_dir.display()))?;
    let snapshot = build_source_snapshot(source)?;
    let previous = read_build_source_snapshot(out_dir);

    let mut output_files = BTreeSet::new();
    collect_file_keys(out_dir, out_dir, &mut output_files)?;
    for relative in output_files {
        if snapshot.files.contains_key(&relative) {
            continue;
        }
        if preserved_output_files.contains(&relative) {
            continue;
        }
        let path = path_from_normalized_relative(out_dir, &relative)?;
        if path.is_file() {
            fs::remove_file(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        }
    }

    for (relative, source_file) in &snapshot.files {
        let target_path = path_from_normalized_relative(out_dir, relative)?;
        let unchanged_source = previous
            .as_ref()
            .and_then(|old| old.files.get(relative))
            .is_some_and(|old_file| old_file == source_file);
        let was_patched = previous
            .as_ref()
            .is_some_and(|old| old.patched_files.contains(relative));
        if force_copy_source_files || !unchanged_source || was_patched || !target_path.is_file() {
            copy_source_relative_file(source, out_dir, relative)?;
        }
    }

    remove_empty_dirs(out_dir, out_dir)?;
    Ok(snapshot)
}

pub(in super::super) fn apply_configured_npc_icon_fallbacks(
    body: &mut fusionforge::UnityValue,
    patch_config: &JsonValue,
) -> Result<usize, String> {
    let fallbacks = configured_npc_icon_fallbacks(patch_config)?;
    if fallbacks.is_empty() {
        return Ok(0);
    }
    let npc_table = npc_table_from_body_mut(body)
        .ok_or_else(|| "TableData patch has no m_pNpcTable for NpcIconFallbacks".to_string())?;
    let icon_rows = npc_table_array(npc_table, "m_pNpcIconData").len();
    let fallback_indices = fallbacks
        .iter()
        .map(|(npc_id, fallback_npc_id)| {
            let fallback_row = npc_table_array(npc_table, "m_pNpcData")
                .get(*fallback_npc_id)
                .ok_or_else(|| {
                    format!("NpcIconFallbacks donor NPC {fallback_npc_id} does not exist")
                })?;
            let icon_index = unity_usize_field(fallback_row, "m_iIcon1").ok_or_else(|| {
                format!("NpcIconFallbacks donor NPC {fallback_npc_id} has no m_iIcon1")
            })?;
            if icon_index >= icon_rows {
                return Err(format!(
                    "NpcIconFallbacks donor NPC {fallback_npc_id} points outside m_pNpcIconData ({icon_index} >= {icon_rows})"
                ));
            }
            Ok((*npc_id, *fallback_npc_id, icon_index))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let rows = npc_table
        .get_mut("m_pNpcData")
        .and_then(fusionforge::UnityValue::as_array_mut)
        .ok_or_else(|| "TableData patch has no m_pNpcData array".to_string())?;
    let mut changed = 0usize;
    for (npc_id, fallback_npc_id, icon_index) in fallback_indices {
        let row = rows
            .get_mut(npc_id)
            .ok_or_else(|| format!("NpcIconFallbacks target NPC {npc_id} does not exist"))?;
        if unity_usize_field(row, "m_iIcon1") != Some(icon_index) {
            set_unity_usize_field(row, "m_iIcon1", icon_index);
            changed += 1;
        }
        eprintln!(
            "[ffclient:npc] icon fallback NPC {npc_id} -> NPC {fallback_npc_id} (row {icon_index})"
        );
    }
    Ok(changed)
}

pub(in super::super) fn update_layout_input_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}
