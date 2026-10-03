use super::super::*;

pub(crate) fn attach_staged_npc_generated_icon(
    project_dir: String,
    npc_id: usize,
    requested_icon: NpcGeneratedIcon,
    icon_bundle: Option<String>,
) -> EditorResult<NpcGeneratedIconAttachResult> {
    run_table_data_task(move || {
        use image::GenericImageView;

        let project = PathBuf::from(project_dir);
        let (manifest_path, mut manifest, patch_path, mut patch_document, mut body) =
            load_staged_npc_icon_patch(&project, npc_id)?;
        let table = npc_table_from_body_mut(&mut body)
            .ok_or_else(|| "Staged TableData patch has no m_pNpcTable".to_string())?;
        let preferred_icon_type = parse_npc_icon_asset_path(&requested_icon.asset_path)
            .and_then(|icon| npc_icon_type_for_prefix(&icon.prefix));
        let plan = plan_staged_npc_generated_icon_from_loaded(
            &project,
            &manifest_path,
            &manifest,
            table,
            npc_id,
            preferred_icon_type,
        )?;
        let binding =
            bind_unique_generated_npc_icon_row(table, npc_id, plan.icon_type, plan.icon_number)?;
        let asset_path = npc_icon_runtime_asset_path(plan.icon_type, binding.icon_number)
            .ok_or_else(|| "Could not format generated NPC icon asset path".to_string())?;
        let relative_file = Some(requested_icon.file)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(plan.relative_file);
        let relative_path = safe_project_relative_path(&relative_file)?;
        let png_path = project.join(&relative_path);
        let png = fs::read(&png_path).map_err(|err| format!("{}: {err}", png_path.display()))?;
        let image = image::load_from_memory(&png)
            .map_err(|err| format!("could not verify generated ICON PNG: {err}"))?;
        let (width, height) = image.dimensions();
        if width != 64 || height != 64 {
            return Err(format!(
                "Generated NPC ICON must be 64x64; got {width}x{height}"
            ));
        }
        let template_asset_path = Some(requested_icon.template_asset_path)
            .map(|value| normalized_asset_path(value.trim()))
            .filter(|value| !value.is_empty())
            .unwrap_or(plan.template_asset_path);
        if parse_npc_icon_asset_path(&template_asset_path).is_none() {
            return Err(format!(
                "Generated NPC ICON template path is not a runtime icon path: {template_asset_path}"
            ));
        }
        let icon_bundle = icon_bundle
            .map(|value| value.trim().replace('\\', "/"))
            .filter(|value| !value.is_empty())
            .unwrap_or(plan.icon_bundle);
        let generated_icon = NpcGeneratedIcon {
            file: relative_path.to_string_lossy().replace('\\', "/"),
            asset_path,
            template_asset_path,
            size: 64,
            camera: requested_icon.camera,
        };
        set_staged_generated_icon_manifest_fields(&mut manifest, &icon_bundle, &generated_icon)?;
        if patch_document.get("value").is_some() {
            patch_document["value"] = unity_value_to_json(&body);
        } else {
            patch_document = unity_value_to_json(&body);
        }

        let original_patch =
            fs::read(&patch_path).map_err(|err| format!("{}: {err}", patch_path.display()))?;
        let patch_data =
            serde_json::to_string_pretty(&patch_document).map_err(|err| err.to_string())?;
        let manifest_data =
            serde_json::to_string_pretty(&manifest).map_err(|err| err.to_string())?;
        fs::write(&patch_path, format!("{patch_data}\n"))
            .map_err(|err| format!("{}: {err}", patch_path.display()))?;
        if let Err(err) = fs::write(&manifest_path, format!("{manifest_data}\n")) {
            let rollback = fs::write(&patch_path, original_patch);
            return Err(match rollback {
                Ok(()) => format!("{}: {err}", manifest_path.display()),
                Err(rollback_err) => format!(
                    "{}: {err}; additionally failed to roll back {}: {rollback_err}",
                    manifest_path.display(),
                    patch_path.display()
                ),
            });
        }

        Ok(NpcGeneratedIconAttachResult {
            manifest_path: manifest_path.to_string_lossy().to_string(),
            table_data_patch_path: patch_path.to_string_lossy().to_string(),
            generated_icon,
            icon_type: plan.icon_type,
            icon_number: binding.icon_number,
            icon_row_index: binding.row_index,
            cloned_shared_row: binding.cloned_shared_row,
        })
    })
}
