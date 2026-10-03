use super::super::*;

pub(in super::super) fn default_authoring_model_container_path(
    authoring_model: &Path,
    blueprint: &NpcBlueprint,
) -> String {
    let base = if !blueprint.internal_name.trim().is_empty() {
        blueprint.internal_name.trim().to_string()
    } else {
        authoring_model
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("imported_npc")
            .to_string()
    };
    format!("mob/{}.kfm", safe_segment(&base))
}

pub(in super::super) fn legacy_manifest_authoring_model_path(manifest: &JsonValue) -> Option<PathBuf> {
    let marker = "Converted authoring model ";
    let separator = " into a clean Unity resourceFile ";
    manifest
        .get("warnings")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .find_map(|warning| {
            warning
                .strip_prefix(marker)
                .and_then(|value| value.split_once(separator))
                .map(|(path, _)| PathBuf::from(path.trim()))
        })
        .filter(|path| is_authoring_model_path(&path.to_string_lossy()))
}

pub(in super::super) fn authoring_model_path_from_manifest(
    project: &Path,
    blueprint: &NpcBlueprint,
    manifest: &JsonValue,
) -> Option<PathBuf> {
    blueprint
        .authoring_model_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && is_authoring_model_path(value))
        .map(|value| resolve_project_path(project, value))
        .or_else(|| legacy_manifest_authoring_model_path(manifest))
}

pub(in super::super) fn stage_npc_authoring_model_source(
    project: &Path,
    blueprint: &mut NpcBlueprint,
    authoring_model: &Path,
) -> Result<(PathBuf, String, String), String> {
    let (project_authoring_model, project_authoring_model_relative) =
        materialize_npc_authoring_source_assets(project, blueprint, authoring_model)?;
    let target_model_path = blueprint
        .model_asset
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && !is_authoring_model_path(value))
        .map(|value| value.replace('\\', "/"))
        .unwrap_or_else(|| {
            default_authoring_model_container_path(&project_authoring_model, blueprint)
        });
    let internal_name = asset_stem_from_container_path(&target_model_path)
        .unwrap_or_else(|| safe_segment(&blueprint.name).to_ascii_lowercase());
    let target_texture_path = format!("texture/{internal_name}.dds");
    blueprint.model_bundle = Some(project_authoring_model_relative.clone());
    blueprint.model_asset = Some(target_model_path.clone());
    blueprint.authoring_model_path = Some(project_authoring_model_relative);
    blueprint.texture_asset = Some(target_texture_path.clone());
    Ok((
        project_authoring_model,
        target_model_path,
        target_texture_path,
    ))
}

pub(in super::super) fn materialize_npc_authoring_model_bundle(
    project: &Path,
    blueprint: &mut NpcBlueprint,
    authoring_model: &Path,
) -> Result<(PathBuf, String, String), String> {
    let (project_authoring_model, project_authoring_model_relative) =
        materialize_npc_authoring_source_assets(project, blueprint, authoring_model)?;
    let target_model_path = blueprint
        .model_asset
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && !is_authoring_model_path(value))
        .map(|value| value.replace('\\', "/"))
        .unwrap_or_else(|| {
            default_authoring_model_container_path(&project_authoring_model, blueprint)
        });
    blueprint.model_asset = Some(target_model_path.clone());
    blueprint.authoring_model_path = Some(project_authoring_model_relative);

    let output_bundle = npc_converted_authoring_model_path(project, blueprint);
    let internal_name = asset_stem_from_container_path(&target_model_path)
        .unwrap_or_else(|| safe_segment(&blueprint.name).to_ascii_lowercase());
    let target_texture_path = format!("texture/{internal_name}.dds");
    write_clean_gltf_npc_resource_file(
        &project_authoring_model,
        &output_bundle,
        output_bundle
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Character_Custom"),
        &internal_name,
    )?;
    blueprint.model_bundle = Some(output_bundle.to_string_lossy().to_string());
    blueprint.texture_bundle = Some(output_bundle.to_string_lossy().to_string());
    blueprint.texture_asset = Some(target_texture_path.clone());
    Ok((output_bundle, target_model_path, target_texture_path))
}

pub(in super::super) fn prepare_npc_authoring_model_bundle(
    project: &Path,
    source_table: &fusionforge::UnityValue,
    template_npc_id: usize,
    blueprint: &mut NpcBlueprint,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Some(model_bundle) = blueprint
        .model_bundle
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return Ok(());
    };
    if !is_authoring_model_path(&model_bundle) {
        return Ok(());
    }
    let authoring_model = resolve_project_path(project, &model_bundle);
    if !authoring_model.is_file() {
        return Err(EditorError::MissingPath(model_bundle).to_string());
    }
    let (project_authoring_model, target_model_path, target_texture_path) =
        stage_npc_authoring_model_source(project, blueprint, &authoring_model)?;
    warnings.push(format!(
        "Staged authoring model {} as project NPC source {} with target AssetBundle container entry {}; clean Unity resourceFile will be generated during build.",
        authoring_model.display(),
        project_authoring_model.display(),
        target_model_path
    ));
    warnings.push(format!(
        "Staged NPC texture target {}.",
        target_texture_path
    ));
    infer_authoring_npc_animation_set_from_template(
        project,
        source_table,
        template_npc_id,
        blueprint,
        warnings,
    );
    infer_authoring_npc_icon_from_template(
        project,
        source_table,
        template_npc_id,
        blueprint,
        warnings,
    );
    Ok(())
}

pub(in super::super) fn refresh_npc_authoring_model_bundle_for_build(
    project: &Path,
    blueprint: &mut NpcBlueprint,
    manifest: &JsonValue,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Some(authoring_model) = authoring_model_path_from_manifest(project, blueprint, manifest)
    else {
        return Ok(());
    };
    if !authoring_model.is_file() {
        return Err(
            EditorError::MissingPath(authoring_model.to_string_lossy().to_string()).to_string(),
        );
    }
    materialize_npc_authoring_model_bundle(project, blueprint, &authoring_model)?;
    if let Some(template_npc_id) = blueprint
        .template_npc_id
        .and_then(|value| usize::try_from(value).ok())
    {
        let context = npc_import_manifest_context(project, manifest);
        let source_body = if let Some(patch_path) = context
            .table_data_patch_path
            .as_ref()
            .filter(|path| path.exists())
        {
            table_data_patch_value(patch_path, 0).ok()
        } else if let (Some(source_bundle), Some(source_asset), Some(source_path_id)) = (
            context.source_table_data_bundle.as_ref(),
            context.source_asset.as_deref(),
            context.source_path_id,
        ) {
            read_table_data_object_body(project, source_bundle, source_asset, source_path_id).ok()
        } else {
            None
        };
        if let Some(source_body) = source_body {
            if let Some(source_table) = npc_table_from_body(&source_body) {
                infer_authoring_npc_animation_set_from_template(
                    project,
                    source_table,
                    template_npc_id,
                    blueprint,
                    warnings,
                );
                infer_authoring_npc_icon_from_template(
                    project,
                    source_table,
                    template_npc_id,
                    blueprint,
                    warnings,
                );
            }
        }
    }
    Ok(())
}

pub(in super::super) fn npc_imported_model_target_path(npc_id: i64, source_path: &str) -> String {
    format!(
        "mob/{}.kfm",
        npc_imported_asset_unique_stem(npc_id, source_path)
    )
}

pub(in super::super) fn npc_model_sibling_path(model_path: &str, extension: &str) -> Option<String> {
    let model_path = normalized_asset_path(model_path);
    let (base, _) = model_path.rsplit_once('.')?;
    Some(format!(
        "{}.{}",
        base,
        extension.trim_start_matches('.').to_ascii_lowercase()
    ))
}

pub(in super::super) fn npc_expected_model_name_variants(expected_name: &str) -> Vec<String> {
    let mut variants = BTreeSet::new();
    let normalized = expected_name.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Vec::new();
    }
    variants.insert(normalized.clone());
    for prefix in ["npc_", "mob_"] {
        if let Some(stripped) = normalized.strip_prefix(prefix) {
            if !stripped.is_empty() {
                variants.insert(stripped.to_string());
            }
        }
    }
    if let Some((_, tail)) = normalized.rsplit_once('_') {
        if !tail.is_empty() {
            variants.insert(tail.to_string());
        }
    }
    variants.into_iter().collect()
}
