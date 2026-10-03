use super::super::*;

pub(in super::super) fn staged_npc_table_icon_asset_path(
    project: &Path,
    manifest: &JsonValue,
    blueprint: &NpcBlueprint,
) -> Result<Option<String>, String> {
    let context = npc_import_manifest_context(project, manifest);
    let Some(patch_path) = context
        .table_data_patch_path
        .as_ref()
        .filter(|path| path.is_file())
        .cloned()
    else {
        return Ok(None);
    };
    let npc_id = blueprint.npc_id;
    run_table_data_task(move || {
        let body = table_data_patch_value(&patch_path, 0)?;
        let Some(npc_table) = npc_table_from_body(&body) else {
            return Ok(None);
        };
        let npc_id = usize::try_from(npc_id)
            .map_err(|_| format!("Invalid NPC id for icon sync: {npc_id}"))?;
        let Some(npc_row) = npc_table_array(npc_table, "m_pNpcData").get(npc_id) else {
            return Ok(None);
        };
        let Some(icon_index) = unity_usize_field(npc_row, "m_iIcon1") else {
            return Ok(None);
        };
        let Some(icon_row) = npc_table_array(npc_table, "m_pNpcIconData").get(icon_index) else {
            return Ok(None);
        };
        let Some(icon_type) = icon_row
            .get("m_iIconType")
            .and_then(fusionforge::UnityValue::as_i64)
        else {
            return Ok(None);
        };
        let Some(icon_number) = icon_row
            .get("m_iIconNumber")
            .and_then(fusionforge::UnityValue::as_i64)
        else {
            return Ok(None);
        };
        Ok(npc_icon_asset_paths(icon_type, icon_number)
            .into_iter()
            .next())
    })
}

pub(in super::super) fn staged_icon_asset_files(path: &Path) -> Result<Vec<PathBuf>, String> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Err(format!(
            "Icon bundle path does not exist: {}",
            path.display()
        ));
    }
    let files = extracted_files_in_dir(path)
        .into_iter()
        .map(|file| PathBuf::from(file.path))
        .collect::<Vec<_>>();
    Ok(files)
}

pub(in super::super) fn sync_staged_npc_icon_asset_file(
    asset_path: &Path,
    old_paths: &BTreeSet<String>,
    target_path: &str,
) -> Result<usize, String> {
    let target_path = normalized_asset_path(target_path);
    let target_stem = asset_stem_from_container_path(&target_path)
        .ok_or_else(|| format!("Invalid target icon path: {target_path}"))?;
    let asset = fusionforge::Asset::from_path(asset_path)
        .map_err(|err| format!("{}: {err}", asset_path.display()))?;
    let env = fusionforge::UnityEnvironment::from_assets(vec![asset.clone()]);
    let mut replacements = BTreeMap::<i64, Vec<u8>>::new();
    let mut texture_keys = BTreeSet::<(usize, i64)>::new();
    let texture2d_count = asset
        .objects
        .values()
        .filter(|info| asset.object_type_name(info) == "Texture2D")
        .count();
    let mut has_target_container = false;
    let mut changed = 0usize;

    for info in asset.objects.values() {
        if asset.object_type_name(info) != "AssetBundle" {
            continue;
        }
        let mut value = asset.read_object(0, info)?;
        let mut object_changed = false;
        if let Some(object) = value.as_object_mut() {
            if let Some(container) = object
                .get_mut("m_Container")
                .and_then(fusionforge::UnityValue::as_array_mut)
            {
                for entry in container {
                    let Some((path, keys)) =
                        fusionforge::pair_name_value(entry).map(|(path, metadata)| {
                            let mut keys = metadata
                                .get("asset")
                                .and_then(fusionforge::UnityValue::as_pointer)
                                .map(|pointer| {
                                    let mut keys = Vec::new();
                                    if pointer.path_id != 0 {
                                        keys.push((0, pointer.path_id));
                                    }
                                    keys.extend(pointer_candidate_keys(&env, pointer));
                                    keys.sort();
                                    keys.dedup();
                                    keys
                                })
                                .unwrap_or_default();
                            keys.sort();
                            keys.dedup();
                            (path.to_string(), keys)
                        })
                    else {
                        continue;
                    };
                    let normalized = normalized_asset_path(&path);
                    let should_retarget =
                        staged_icon_container_should_retarget(&normalized, old_paths, &target_path);
                    if normalized == target_path || should_retarget {
                        has_target_container = true;
                        texture_keys.extend(keys);
                    }
                    if should_retarget {
                        rename_pair_key(entry, &target_path);
                        object_changed = true;
                        changed += 1;
                    }
                }
            }
        }
        if object_changed {
            replacements.insert(info.path_id, asset.serialize_object_value(0, info, &value)?);
        }
    }

    for info in asset.objects.values() {
        if asset.object_type_name(info) != "Texture2D" {
            continue;
        }
        let pointer_match = texture_keys.contains(&(0, info.path_id));
        let sole_target_texture = has_target_container && texture2d_count == 1;
        if !pointer_match && !sole_target_texture {
            continue;
        }
        let mut value = asset.read_object(0, info)?;
        if fusionforge::object_name(&value) == target_stem {
            continue;
        }
        set_unity_object_string(&mut value, "m_Name", &target_stem);
        replacements.insert(info.path_id, asset.serialize_object_value(0, info, &value)?);
        changed += 1;
    }

    if replacements.is_empty() {
        return Ok(0);
    }
    let rebuilt = asset.rebuild_with_object_data(&replacements)?;
    fs::write(asset_path, rebuilt).map_err(|err| format!("{}: {err}", asset_path.display()))?;
    Ok(changed)
}

pub(in super::super) fn sync_staged_npc_manifest_icon_paths(manifest: &mut JsonValue, target_path: &str) -> bool {
    let mut changed = false;
    changed |= set_json_nested_string(manifest, &["blueprint", "iconAsset"], target_path);
    changed |= set_json_nested_string(
        manifest,
        &["resources", "iconTexture", "asset"],
        target_path,
    );
    changed
}

pub(in super::super) fn asset_stem_from_container_path(path: &str) -> Option<String> {
    let file_name = path.replace('\\', "/").rsplit('/').next()?.to_string();
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(&file_name);
    (!stem.is_empty()).then(|| stem.to_string())
}

pub(in super::super) fn write_npc_import_manifest(
    project: &Path,
    blueprint: &NpcBlueprint,
    source_bundle_path: &str,
    source_asset: &str,
    source_path_id: i64,
    source_npc_id: usize,
    table_data_patch_path: &Path,
    copied_sections: &[String],
    warnings: &[String],
) -> Result<PathBuf, String> {
    let bundle_name = npc_blueprint_bundle_name(blueprint);
    let target = npc_import_manifest_path(project, blueprint);
    let import_root = target
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", target.display()))?;
    fs::create_dir_all(import_root).map_err(|err| EditorError::Io(err).to_string())?;
    let context = NpcImportManifestContext {
        source_table_data_bundle: Some(PathBuf::from(source_bundle_path)),
        source_asset: Some(source_asset.to_string()),
        source_path_id: Some(source_path_id),
        source_npc_id: Some(source_npc_id),
        table_data_patch_path: Some(table_data_patch_path.to_path_buf()),
        source_paths: Vec::new(),
    };
    let (staged_blueprint, source_bundles) =
        materialize_npc_import_source_assets(project, blueprint, &context)?;
    let mut document = json!({
        "format": "fftools.npc-import.v1",
        "bundleStrategy": "build-time-standalone-npc-resource-bundle",
        "sourceAssetStrategy": "import-time-build-source-assets",
        "bundleName": bundle_name,
        "blueprint": &staged_blueprint,
        "source": {
            "tableDataBundle": source_bundle_path,
            "asset": source_asset,
            "pathId": source_path_id,
            "npcId": source_npc_id,
        },
        "tableDataPatch": {
            "file": table_data_patch_path.strip_prefix(project).unwrap_or(table_data_patch_path).to_string_lossy().replace('\\', "/"),
            "copiedSections": copied_sections,
        },
        "sourceBundles": source_bundles,
        "resources": {
            "model": {
                "bundle": staged_blueprint.model_bundle.as_deref(),
                "asset": staged_blueprint.model_asset.as_deref(),
            },
            "texture": {
                "bundle": staged_blueprint.texture_bundle.as_deref(),
                "asset": staged_blueprint.texture_asset.as_deref(),
            },
            "iconTexture": {
                "bundle": staged_blueprint.icon_bundle.as_deref(),
                "asset": staged_blueprint.icon_asset.as_deref(),
                "generated": staged_blueprint.generated_icon.as_ref(),
            },
            "animation": {
                "bundle": staged_blueprint.animation_set.as_deref(),
            },
            "audioSource": staged_blueprint.audio_source.as_deref(),
            "audioPrefix": staged_blueprint.audio_prefix.as_deref(),
        },
        "warnings": warnings,
    });
    set_npc_manifest_build_source_assets(project, &mut document, &staged_blueprint);
    let data = serde_json::to_string_pretty(&document)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(&target, format!("{data}\n")).map_err(|err| EditorError::Io(err).to_string())?;
    cleanup_npc_build_work_dir(project, blueprint);

    let mut patch_config = ensure_patch_project_config(project)?;
    patch_config["NpcImportDir"] = json!("npcs");
    let patch_data = serde_json::to_string_pretty(&patch_config)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(project.join("ffpatch.json"), format!("{patch_data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    Ok(target)
}

pub(in super::super) fn npc_import_manifest_source_bundle_references(
    project: &Path,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
) -> Result<Vec<String>, String> {
    let mut hints = npc_asset_hints_from_patch(context, blueprint)?;
    if !npc_blueprint_uses_authoring_model_bundle(blueprint) {
        extend_npc_asset_hints(&mut hints, npc_asset_hints_from_source(project, context)?);
    }
    if let Some(index) = load_npc_source_index(project, context) {
        add_npc_texture_asset_hints_from_index(&mut hints, blueprint, &index);
        add_npc_audio_asset_hints_from_index(&mut hints, blueprint, &index);
    }
    let source_paths = npc_augmented_source_bundles(project, blueprint, context, &hints);
    let mut references = source_paths
        .into_iter()
        .map(|path| {
            path_relative_to_project(project, &path)
                .unwrap_or_else(|| path.to_string_lossy().replace('\\', "/"))
        })
        .collect::<Vec<_>>();
    references.sort();
    references.dedup();
    Ok(references)
}

pub(in super::super) fn snapshot_npc_import_manifest_assets(
    project: &Path,
    manifest: &mut JsonValue,
    blueprint: &NpcBlueprint,
    fallback_context: &NpcImportManifestContext,
) -> Result<(), String> {
    let context = npc_import_manifest_context(project, manifest);
    let source_bundle = direct_npc_character_bundle_source(project, manifest, blueprint, &context)
        .or_else(|| {
            direct_npc_character_bundle_source(project, manifest, blueprint, fallback_context)
        })
        .or_else(|| first_npc_character_source_bundle(project, blueprint, &context))
        .or_else(|| first_npc_character_source_bundle(project, blueprint, fallback_context));
    let Some(source_bundle) = source_bundle else {
        return Ok(());
    };
    let snapshot_dir = npc_snapshot_assets_root(project, blueprint);
    fusionforge::snapshot_npc_bundle_quiet(&source_bundle, &snapshot_dir)?;
    set_npc_manifest_raw_assets_snapshot(project, manifest, blueprint);
    Ok(())
}

pub(in super::super) fn safe_project_relative_path(relative_file: &str) -> Result<PathBuf, String> {
    let normalized = relative_file.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err("relative file path is empty".to_string());
    }
    let path = PathBuf::from(&normalized);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "{relative_file} is not a safe project-relative path"
        ));
    }
    Ok(path)
}

pub(in super::super) fn staged_npc_import_manifest_for_id(project: &Path, npc_id: usize) -> Result<PathBuf, String> {
    let mut manifests = Vec::new();
    collect_npc_import_manifest_paths(&project.join("npcs"), &mut manifests);
    manifests.sort();
    let mut matches = Vec::new();
    for path in manifests {
        let Ok(data) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(document) = serde_json::from_str::<JsonValue>(&data) else {
            continue;
        };
        if document.get("format").and_then(JsonValue::as_str) != Some("fftools.npc-import.v1") {
            continue;
        }
        if document
            .pointer("/blueprint/npcId")
            .and_then(JsonValue::as_u64)
            == Some(npc_id as u64)
        {
            matches.push(path);
        }
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(format!(
            "Staged NPC manifest for NPC {npc_id} was not found"
        )),
        count => Err(format!(
            "Found {count} staged NPC manifests for NPC {npc_id}; refusing an ambiguous icon update"
        )),
    }
}

pub(crate) fn ensure_staged_npc_icon_manifest(
    project_dir: String,
    target_bundle_path: String,
    target_asset: String,
    target_path_id: i64,
    npc_id: usize,
    mut blueprint: NpcBlueprint,
) -> EditorResult<String> {
    run_table_data_task(move || {
        use fusionforge::object_name;

        let project = PathBuf::from(project_dir);
        if let Ok(path) = staged_npc_import_manifest_for_id(&project, npc_id) {
            return Ok(path.to_string_lossy().to_string());
        }
        let mut existing = Vec::new();
        collect_npc_import_manifest_paths(&project.join("npcs"), &mut existing);
        let matching = existing
            .iter()
            .filter(|path| {
                fs::read_to_string(path)
                    .ok()
                    .and_then(|data| serde_json::from_str::<JsonValue>(&data).ok())
                    .and_then(|document| {
                        document
                            .pointer("/blueprint/npcId")
                            .and_then(JsonValue::as_u64)
                    })
                    == Some(npc_id as u64)
            })
            .count();
        if matching > 1 {
            return Err(format!(
                "Found {matching} staged NPC manifests for NPC {npc_id}; refusing an ambiguous icon manifest"
            ));
        }

        let icon_bundle = blueprint
            .icon_bundle
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("NPC {npc_id} has no source icon bundle"))?;
        let icon_asset = blueprint
            .icon_asset
            .clone()
            .filter(|value| parse_npc_icon_asset_path(value).is_some())
            .ok_or_else(|| format!("NPC {npc_id} has no valid icon Texture2D template path"))?;
        let patch_path = match table_data_patch_path_for_object(
            &project,
            &target_bundle_path,
            &target_asset,
            target_path_id,
        )? {
            Some(path) if path.is_file() => path,
            _ => {
                let target_bundle = PathBuf::from(&target_bundle_path);
                let target_extract_dir = extract_bundle_cached(&project, &target_bundle)?;
                let target_asset_path = extracted_asset_path(&target_extract_dir, &target_asset)?;
                let target_asset_file = fusionforge::Asset::from_path(&target_asset_path)?;
                let target_info =
                    target_asset_file
                        .objects
                        .get(&target_path_id)
                        .ok_or_else(|| {
                            format!(
                                "{}#{} was not found",
                                target_asset_file.name, target_path_id
                            )
                        })?;
                let target_body = target_asset_file.read_object(0, target_info)?;
                let target_table = npc_table_from_body(&target_body)
                    .ok_or_else(|| "target TableData object has no m_pNpcTable".to_string())?;
                if npc_table_array(target_table, "m_pNpcData")
                    .get(npc_id)
                    .is_none()
                {
                    return Err(format!("m_pNpcTable.m_pNpcData[{npc_id}] does not exist"));
                }
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
                write_table_data_patch_document(
                    &project,
                    &target_bundle_path,
                    &target_asset,
                    target_path_id,
                    &format!("npc_icon_{npc_id}__{}", safe_segment(&blueprint.name)),
                    &document,
                )?
            }
        };

        blueprint.npc_id = npc_id as i64;
        // This bundle intentionally contains only the icon Texture2D. The
        // original character model and textures remain owned by their source
        // bundles and are not duplicated merely to change a portrait.
        blueprint.authoring_model_path = None;
        blueprint.model_bundle = None;
        blueprint.model_asset = None;
        blueprint.texture_bundle = None;
        blueprint.texture_asset = None;
        blueprint.animation_set = None;
        blueprint.audio_source = None;
        blueprint.audio_prefix = None;
        blueprint.generated_icon = None;
        blueprint.icon_bundle = Some(icon_bundle.clone());
        blueprint.icon_asset = Some(icon_asset.clone());

        let manifest_path = npc_import_manifest_path(&project, &blueprint);
        let manifest_root = manifest_path
            .parent()
            .ok_or_else(|| format!("{} has no parent", manifest_path.display()))?;
        fs::create_dir_all(manifest_root).map_err(|err| EditorError::Io(err).to_string())?;
        let patch_file = patch_path
            .strip_prefix(&project)
            .unwrap_or(&patch_path)
            .to_string_lossy()
            .replace('\\', "/");
        let bundle_name = format!("Character_Icon_{npc_id}.resourceFile");
        let document = json!({
            "format": "fftools.npc-import.v1",
            "bundleStrategy": "build-time-icon-resource-bundle",
            "sourceAssetStrategy": "existing-icon-template-bundle",
            "bundleName": bundle_name,
            "blueprint": &blueprint,
            "source": {
                "tableDataBundle": target_bundle_path,
                "asset": target_asset,
                "pathId": target_path_id,
                "npcId": npc_id,
            },
            "tableDataPatch": {
                "file": patch_file,
                "copiedSections": ["m_pNpcData", "m_pNpcIconData"],
            },
            "sourceBundles": [icon_bundle],
            "resources": {
                "model": { "bundle": null, "asset": null },
                "texture": { "bundle": null, "asset": null },
                "iconTexture": {
                    "bundle": blueprint.icon_bundle.as_deref(),
                    "asset": icon_asset,
                    "generated": null,
                },
                "animation": { "bundle": null },
                "audioSource": null,
                "audioPrefix": null,
            },
            "warnings": ["Icon-only staged bundle; the original NPC model remains unchanged."],
        });
        let data = serde_json::to_string_pretty(&document)
            .map_err(|err| EditorError::Json(err).to_string())?;
        fs::write(&manifest_path, format!("{data}\n"))
            .map_err(|err| EditorError::Io(err).to_string())?;

        let mut patch_config = ensure_patch_project_config(&project)?;
        patch_config["NpcImportDir"] = json!("npcs");
        let patch_data = serde_json::to_string_pretty(&patch_config)
            .map_err(|err| EditorError::Json(err).to_string())?;
        fs::write(project.join("ffpatch.json"), format!("{patch_data}\n"))
            .map_err(|err| EditorError::Io(err).to_string())?;
        Ok(manifest_path.to_string_lossy().to_string())
    })
}

pub(in super::super) fn set_staged_generated_icon_manifest_fields(
    manifest: &mut JsonValue,
    icon_bundle: &str,
    generated_icon: &NpcGeneratedIcon,
) -> Result<(), String> {
    let generated_value = serde_json::to_value(generated_icon).map_err(|err| err.to_string())?;
    let blueprint = manifest
        .get_mut("blueprint")
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| "Staged NPC manifest has no blueprint object".to_string())?;
    blueprint.insert("iconBundle".to_string(), json!(icon_bundle));
    blueprint.insert("iconAsset".to_string(), json!(generated_icon.asset_path));
    blueprint.insert("generatedIcon".to_string(), generated_value.clone());

    let root = manifest
        .as_object_mut()
        .ok_or_else(|| "Staged NPC manifest is not an object".to_string())?;
    let resources = root
        .entry("resources".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "Staged NPC manifest resources is not an object".to_string())?;
    let icon_texture = resources
        .entry("iconTexture".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "Staged NPC manifest iconTexture is not an object".to_string())?;
    icon_texture.insert("bundle".to_string(), json!(icon_bundle));
    icon_texture.insert("asset".to_string(), json!(generated_icon.asset_path));
    icon_texture.insert("generated".to_string(), generated_value);

    let source_bundles = root
        .entry("sourceBundles".to_string())
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| "Staged NPC manifest sourceBundles is not an array".to_string())?;
    if !source_bundles
        .iter()
        .any(|value| value.as_str() == Some(icon_bundle))
    {
        source_bundles.push(json!(icon_bundle));
    }
    if let Some(copied_sections) = root
        .get_mut("tableDataPatch")
        .and_then(|value| value.get_mut("copiedSections"))
        .and_then(JsonValue::as_array_mut)
    {
        if !copied_sections
            .iter()
            .any(|value| value.as_str() == Some("m_pNpcIconData"))
        {
            copied_sections.push(json!("m_pNpcIconData"));
        }
    }
    Ok(())
}

pub(in super::super) fn stage_npc_custom_asset_import(
    project_dir: String,
    target_bundle_path: String,
    target_asset: String,
    target_path_id: i64,
    template_npc_id: usize,
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
        if blueprint
            .model_bundle
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            return Err(
                "Custom NPC needs a source Model Bundle/resourceFile or GLB/GLTF/OBJ model."
                    .to_string(),
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

        let source_table_snapshot = npc_table_from_body(&target_body)
            .ok_or_else(|| "target TableData object has no m_pNpcTable".to_string())?
            .clone();
        let mut warnings = vec![
            "Custom NPC uses the selected target NPC only as a TableData row schema; source dependencies are materialized into this NPC's assets folder before build.".to_string(),
        ];
        prepare_npc_authoring_model_bundle(
            &project,
            &source_table_snapshot,
            template_npc_id,
            &mut blueprint,
            &mut warnings,
        )?;
        let target_table = npc_table_from_body_mut(&mut target_body)
            .ok_or_else(|| "target TableData object has no m_pNpcTable".to_string())?;
        let npc_id = requested_or_existing_staged_npc_id(
            &project,
            target_table,
            blueprint.npc_id,
            &blueprint.name,
        )?;
        if npc_id == template_npc_id {
            return Err("Custom NPC ID must be different from the template NPC ID.".to_string());
        }
        blueprint.npc_id = npc_id as i64;
        blueprint.template_npc_id = Some(template_npc_id as i64);
        let mut copied_sections = copy_npc_table_rows(
            &source_table_snapshot,
            target_table,
            template_npc_id,
            npc_id,
        )?;
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

        let patch_name = format!("npc_custom_{}__{}", npc_id, safe_segment(&blueprint.name));
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
            &target_bundle_path,
            &target_asset,
            target_path_id,
            template_npc_id,
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

pub(in super::super) fn is_unity_bundle_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| {
            ext.eq_ignore_ascii_case("resourceFile") || ext.eq_ignore_ascii_case("unity3d")
        })
}

pub(in super::super) fn npc_manifest_file_name(blueprint: &NpcBlueprint) -> String {
    format!(
        "{}__{}.npc-import.json",
        blueprint.npc_id,
        safe_segment(&blueprint.name)
    )
}

pub(in super::super) fn npc_import_manifest_path(project: &Path, blueprint: &NpcBlueprint) -> PathBuf {
    npc_stage_root(project, blueprint).join(npc_manifest_file_name(blueprint))
}

pub(in super::super) fn npc_snapshot_manifest_relative(project: &Path, blueprint: &NpcBlueprint) -> String {
    let manifest_path = npc_snapshot_assets_root(project, blueprint).join("_manifest.json");
    path_relative_to_project(project, &manifest_path)
        .unwrap_or_else(|| manifest_path.to_string_lossy().replace('\\', "/"))
}

pub(in super::super) fn set_npc_manifest_raw_assets_snapshot(
    project: &Path,
    manifest: &mut JsonValue,
    blueprint: &NpcBlueprint,
) {
    manifest["rawAssets"] = json!({
        "format": "fftools.npc-assets.v3",
        "layout": "final-bundle-snapshot",
        "manifest": npc_snapshot_manifest_relative(project, blueprint),
    });
}
