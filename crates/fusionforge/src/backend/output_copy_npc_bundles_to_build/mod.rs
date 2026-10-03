use super::super::*;

pub(in super::super) fn copy_npc_bundles_to_build(
    project: &Path,
    patch_config: &JsonValue,
    out_dir: &Path,
) -> Result<(usize, Vec<String>), String> {
    let mut copied = 0usize;
    let mut all_warnings = Vec::new();

    let npc_import_dir = npc_import_dir_from_patch_config(project, patch_config);
    if npc_import_dir.is_dir() {
        let target_serialized_format = Some(6);
        let target_external_index_project = project.to_path_buf();
        let target_external_index = run_table_data_task(move || {
            build_npc_target_external_index(&target_external_index_project)
        })?;
        let target_type_tree_project = project.to_path_buf();
        let target_type_trees = run_table_data_task(move || {
            build_npc_target_type_tree_index(&target_type_tree_project, target_serialized_format)
        })?;
        let mut manifests = Vec::new();
        collect_npc_import_manifest_paths(&npc_import_dir, &mut manifests);
        manifests.sort();
        manifests.dedup();
        let manifest_total = manifests.len();
        let mut consolidated_table_patch = seed_active_table_data_patch(project)?;
        for (manifest_index, manifest_path) in manifests.into_iter().enumerate() {
            let manifest_label = manifest_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("npc-import")
                .to_string();
            eprintln!(
                "[ffclient:npc] {}/{} {}",
                manifest_index + 1,
                manifest_total,
                manifest_label
            );
            let mut manifest = serde_json::from_str::<JsonValue>(
                &fs::read_to_string(&manifest_path)
                    .map_err(|err| format!("{}: {err}", manifest_path.display()))?,
            )
            .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
            migrate_npc_import_manifest_source_table_bundle(&mut manifest);
            hydrate_npc_import_manifest_source_table_bundle(project, &mut manifest);
            if manifest.get("format").and_then(JsonValue::as_str) != Some("fftools.npc-import.v1") {
                return Err(format!(
                    "Unsupported NPC import manifest format in {}",
                    manifest_path.display()
                ));
            }
            let blueprint_value = manifest
                .get("blueprint")
                .cloned()
                .ok_or_else(|| format!("{} has no blueprint", manifest_path.display()))?;
            let mut blueprint = serde_json::from_value::<NpcBlueprint>(blueprint_value)
                .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
            let tabledata_only_link = manifest.get("bundleStrategy").and_then(JsonValue::as_str)
                == Some("tabledata-only-npc-link");
            let mut manifest_warnings = Vec::new();
            let bundle_name = manifest
                .get("bundleName")
                .and_then(JsonValue::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| npc_blueprint_bundle_name(&blueprint));
            let mut pre_stage_context = npc_import_manifest_context(project, &manifest);
            if !tabledata_only_link
                && !npc_blueprint_uses_authoring_model_bundle(&blueprint)
                && !npc_manifest_uses_build_source_assets(
                    project,
                    &blueprint,
                    &manifest,
                    &pre_stage_context,
                )
            {
                let (materialized_blueprint, materialized_source_paths) =
                    materialize_npc_import_source_assets(project, &blueprint, &pre_stage_context)
                        .map_err(|err| {
                        format!("{manifest_label}: materialize_npc_import_source_assets: {err}")
                    })?;
                blueprint = materialized_blueprint;
                update_npc_manifest_build_source_assets(
                    project,
                    &mut manifest,
                    &blueprint,
                    &materialized_source_paths,
                )
                .map_err(|err| {
                    format!("{manifest_label}: update_npc_manifest_build_source_assets: {err}")
                })?;
                let manifest_data = serde_json::to_string_pretty(&manifest)
                    .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
                fs::write(&manifest_path, format!("{manifest_data}\n"))
                    .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
                pre_stage_context = npc_import_manifest_context(project, &manifest);
            }
            let target = out_dir.join(&bundle_name);
            if !tabledata_only_link
                && npc_standalone_bundle_is_fresh(
                    project,
                    &manifest_path,
                    &manifest,
                    &blueprint,
                    &target,
                )?
            {
                eprintln!("[ffclient:npc] reusing fresh {bundle_name}");
                let mut build_hints = npc_asset_hints_from_patch(&pre_stage_context, &blueprint)?;
                extend_npc_asset_hints(
                    &mut build_hints,
                    npc_asset_hints_from_source(project, &pre_stage_context)?,
                );
                let build_blueprint =
                    retarget_imported_npc_blueprint_paths_for_build(&blueprint, &build_hints);
                let mut build_manifest = manifest.clone();
                build_manifest["blueprint"] = serde_json::to_value(&build_blueprint)
                    .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
                migrate_npc_import_manifest_source_table_bundle(&mut build_manifest);
                hydrate_npc_import_manifest_source_table_bundle(project, &mut build_manifest);
                let context = npc_import_manifest_context(project, &build_manifest);
                apply_npc_build_table_patch_step(
                    project,
                    &manifest_label,
                    &mut consolidated_table_patch,
                    &build_blueprint,
                    &context,
                )?;
                copied += 1;
                cleanup_npc_build_work_dir(project, &blueprint);
                continue;
            }
            if !tabledata_only_link {
                if let Err(err) = refresh_npc_authoring_model_bundle_for_build(
                    project,
                    &mut blueprint,
                    &manifest,
                    &mut manifest_warnings,
                ) {
                    if npc_blueprint_uses_authoring_model_bundle(&blueprint) {
                        all_warnings.push(format!(
                            "{manifest_label}: standalone bundle skipped because authoring NPC refresh failed, but TableData patch will still be applied: {err}"
                        ));
                    } else {
                        return Err(format!(
                            "{manifest_label}: refresh_npc_authoring_model_bundle_for_build: {err}"
                        ));
                    }
                }
            }
            let direct_reference_bundle = None::<PathBuf>;
            let mut skip_bundle_pack = tabledata_only_link;
            let (staged_blueprint, staged_source_paths) = if tabledata_only_link {
                (blueprint.clone(), Vec::new())
            } else if direct_reference_bundle.is_some() {
                cleanup_generated_authoring_bundles(project, &blueprint);
                (blueprint.clone(), Vec::new())
            } else {
                match stage_npc_manifest_source_assets(project, &blueprint, &pre_stage_context) {
                    Ok(result) => {
                        cleanup_generated_authoring_bundles(project, &blueprint);
                        result
                    }
                    Err(err) if npc_blueprint_uses_authoring_model_bundle(&blueprint) => {
                        skip_bundle_pack = true;
                        all_warnings.push(format!(
                            "{manifest_label}: standalone bundle skipped because authoring NPC staging failed, but TableData patch will still be applied: {err}"
                        ));
                        (blueprint.clone(), Vec::new())
                    }
                    Err(err) => {
                        return Err(format!(
                            "{manifest_label}: stage_npc_manifest_source_assets: {err}"
                        ));
                    }
                }
            };
            let mut build_hints =
                npc_asset_hints_from_patch(&pre_stage_context, &staged_blueprint)?;
            extend_npc_asset_hints(
                &mut build_hints,
                npc_asset_hints_from_source(project, &pre_stage_context)?,
            );
            let mut source_blueprint = staged_blueprint.clone();
            let staged_source_paths_absolute = staged_source_paths
                .iter()
                .map(|path| resolve_project_path(project, path))
                .collect::<Vec<_>>();
            rebind_imported_npc_blueprint_assets_from_sources(
                project,
                &mut source_blueprint,
                &staged_source_paths_absolute,
                &build_hints,
            )
            .map_err(|err| {
                format!(
                    "{manifest_label}: rebind_imported_npc_blueprint_assets_from_sources: {err}"
                )
            })?;
            let build_blueprint = if tabledata_only_link {
                source_blueprint.clone()
            } else {
                retarget_imported_npc_blueprint_paths_for_build(&source_blueprint, &build_hints)
            };
            let mut build_manifest = manifest.clone();
            build_manifest["blueprint"] = serde_json::to_value(&build_blueprint)
                .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
            if !staged_source_paths.is_empty() {
                build_manifest["sourceBundles"] = json!(staged_source_paths);
            }
            migrate_npc_import_manifest_source_table_bundle(&mut build_manifest);
            hydrate_npc_import_manifest_source_table_bundle(project, &mut build_manifest);
            if let Some(resources) = build_manifest
                .get_mut("resources")
                .and_then(JsonValue::as_object_mut)
            {
                if let Some(model) = resources
                    .get_mut("model")
                    .and_then(JsonValue::as_object_mut)
                {
                    model.insert(
                        "bundle".to_string(),
                        json!(build_blueprint.model_bundle.as_deref()),
                    );
                    model.insert(
                        "asset".to_string(),
                        json!(build_blueprint.model_asset.as_deref()),
                    );
                }
                if let Some(texture) = resources
                    .get_mut("texture")
                    .and_then(JsonValue::as_object_mut)
                {
                    texture.insert(
                        "bundle".to_string(),
                        json!(build_blueprint.texture_bundle.as_deref()),
                    );
                    texture.insert(
                        "asset".to_string(),
                        json!(build_blueprint.texture_asset.as_deref()),
                    );
                }
                if let Some(icon) = resources
                    .get_mut("iconTexture")
                    .and_then(JsonValue::as_object_mut)
                {
                    icon.insert(
                        "bundle".to_string(),
                        json!(build_blueprint.icon_bundle.as_deref()),
                    );
                    icon.insert(
                        "asset".to_string(),
                        json!(build_blueprint.icon_asset.as_deref()),
                    );
                    icon.insert(
                        "generated".to_string(),
                        json!(build_blueprint.generated_icon.as_ref()),
                    );
                }
                if let Some(animation) = resources
                    .get_mut("animation")
                    .and_then(JsonValue::as_object_mut)
                {
                    animation.insert(
                        "bundle".to_string(),
                        json!(build_blueprint.animation_set.as_deref()),
                    );
                }
            }
            let context = npc_import_manifest_context(project, &build_manifest);
            apply_npc_build_table_patch_step(
                project,
                &manifest_label,
                &mut consolidated_table_patch,
                &build_blueprint,
                &context,
            )?;
            all_warnings.extend(
                manifest_warnings
                    .into_iter()
                    .map(|warning| format!("{bundle_name}: {warning}")),
            );
            if !skip_bundle_pack {
                if let Some(source_bundle) = direct_reference_bundle.as_ref() {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)
                            .map_err(|err| format!("{}: {err}", parent.display()))?;
                    }
                    copy_file_if_different(source_bundle, &target).map_err(|err| {
                        format!(
                            "{manifest_label}: copy reference NPC bundle {} -> {}: {err}",
                            source_bundle.display(),
                            target.display()
                        )
                    })?;
                } else {
                    eprintln!("[ffclient:npc] packing {bundle_name}");
                    let pack_project = project.to_path_buf();
                    let context = context.clone();
                    let target_serialized_format = target_serialized_format;
                    let target_external_index = target_external_index.clone();
                    let target_type_trees = target_type_trees.clone();
                    let source_blueprint_for_pack = source_blueprint.clone();
                    let build_blueprint_for_pack = build_blueprint.clone();
                    let target_for_pack = target.clone();
                    let (_source_bundles, _copied_files, warnings) =
                        run_table_data_task(move || {
                            pack_standalone_npc_bundle_to_path(
                                &pack_project,
                                &source_blueprint_for_pack,
                                &build_blueprint_for_pack,
                                &context,
                                &target_for_pack,
                                target_serialized_format,
                                &target_external_index,
                                &target_type_trees,
                            )
                        })
                        .map_err(|err| {
                            format!("{manifest_label}: pack_standalone_npc_bundle_to_path: {err}")
                        })?;
                    all_warnings.extend(
                        warnings
                            .into_iter()
                            .filter(|warning| {
                                !warning.starts_with("Pruned NPC bundle:")
                                    && !warning.contains("has no preload dependencies")
                            })
                            .map(|warning| format!("{bundle_name}: {warning}")),
                    );
                }
                copied += 1;
            }
            cleanup_npc_build_work_dir(project, &blueprint);
            eprintln!("[ffclient:npc] finished {bundle_name}");
        }
        if let Some((asset, path_id, mut document, mut body)) = consolidated_table_patch {
            let icon_fallbacks = apply_configured_npc_icon_fallbacks(&mut body, patch_config)?;
            if icon_fallbacks > 0 {
                all_warnings.push(format!(
                    "Applied {icon_fallbacks} configured NPC icon fallback(s)."
                ));
            }
            let write_project = project.to_path_buf();
            run_table_data_task(move || {
                document["name"] = json!("npc_imports_consolidated");
                document["value"] = unity_value_to_json(&body);
                write_table_data_patch_document(
                    &write_project,
                    "TableData.resourceFile",
                    &asset,
                    path_id,
                    "npc_imports_consolidated",
                    &document,
                )?;
                Ok(())
            })
            .map_err(|err| {
                format!("npc_imports_consolidated: write_table_data_patch_document: {err}")
            })?;
        }
    }

    let npc_bundle_dir = json_string(patch_config, "NpcBundleDir")
        .map(|value| resolve_project_path(project, &value))
        .unwrap_or_else(|| project.join("npcs").join("bundles"));
    if !npc_bundle_dir.is_dir() {
        write_npc_build_warning_log(project, &all_warnings);
        return Ok((copied, all_warnings));
    }
    for entry in fs::read_dir(&npc_bundle_dir)
        .map_err(|err| format!("{}: {err}", npc_bundle_dir.display()))?
    {
        let entry = entry.map_err(|err| err.to_string())?;
        let source = entry.path();
        if !source.is_file() {
            continue;
        }
        let name = source
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with(".resourcefile") || lower.ends_with(".unity3d")) {
            continue;
        }
        fs::copy(&source, out_dir.join(name))
            .map_err(|err| format!("{}: {err}", source.display()))?;
        copied += 1;
    }
    write_npc_build_warning_log(project, &all_warnings);
    Ok((copied, all_warnings))
}

/// Persists NPC build warnings so silently skipped bundles can always be
/// diagnosed after the fact (the UI only shows a truncated list).
pub(in super::super) fn write_npc_build_warning_log(project: &Path, warnings: &[String]) {
    let log_path = project.join("npcs").join("last-build-warnings.log");
    let body = if warnings.is_empty() {
        "no NPC warnings\n".to_string()
    } else {
        format!("{}\n", warnings.join("\n"))
    };
    if let Err(err) = fs::write(&log_path, body) {
        eprintln!("could not write {}: {err}", log_path.display());
    }
}

pub(in super::super) fn save_text_document(path: String, data: String) -> EditorResult<String> {
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    fs::write(&path, data).map_err(|err| EditorError::Io(err).to_string())?;
    Ok(path.to_string_lossy().into_owned())
}
