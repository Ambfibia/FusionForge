use super::super::*;

pub(in super::super) fn npc_material_main_texture_pointer(
    blueprint: &NpcBlueprint,
    pointers: &BTreeMap<String, fusionforge::UnityValue>,
) -> Option<fusionforge::UnityValue> {
    let preferred_name = blueprint
        .texture_asset
        .as_deref()
        .and_then(asset_stem_from_container_path)
        .map(|name| normalized_asset_path(&name));
    if let Some(preferred_name) = preferred_name {
        if let Some(pointer) = pointers.get(&preferred_name) {
            return Some(pointer.clone());
        }
    }
    pointers
        .iter()
        .filter(|(name, _)| {
            !name.contains("icon")
                && !name.contains("toonramp")
                && !name.contains("fusionlight")
                && !name.contains("bubble")
        })
        .map(|(_, pointer)| pointer.clone())
        .next()
}

pub(in super::super) fn stage_texture_replacement(
    project_dir: String,
    bundle_path: String,
    asset: String,
    path_id: i64,
    name: String,
    width: i64,
    height: i64,
    texture_format: i64,
    replacement_path: String,
) -> EditorResult<String> {
    let project = PathBuf::from(project_dir);
    let mut patch_config = ensure_patch_project_config(&project)?;
    let source = PathBuf::from(replacement_path);
    if !source.is_file() {
        return Err(EditorError::MissingPath(source.to_string_lossy().to_string()).to_string());
    }
    let container = Path::new(&bundle_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("bundle");
    let texture_root = project.join("textures");
    let relative_file = PathBuf::from(safe_segment(container))
        .join(safe_segment(&asset))
        .join(format!("{path_id}__{}.png", safe_segment(&name)));
    let target = texture_root.join(&relative_file);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    fs::copy(&source, &target).map_err(|err| EditorError::Io(err).to_string())?;

    let manifest_path = texture_root.join("manifest.json");
    let mut manifest = if manifest_path.exists() {
        serde_json::from_str::<serde_json::Value>(
            &fs::read_to_string(&manifest_path).map_err(|err| EditorError::Io(err).to_string())?,
        )
        .unwrap_or_else(|_| json!({ "format": "fftools.texture-patch.v1", "entries": [] }))
    } else {
        json!({ "format": "fftools.texture-patch.v1", "entries": [] })
    };
    let entry = json!({
        "container": container,
        "asset": asset,
        "path_id": path_id,
        "name": name,
        "width": width,
        "height": height,
        "texture_format": texture_format.to_string(),
        "file": relative_file.to_string_lossy().replace('\\', "/"),
    });
    let entries = manifest["entries"]
        .as_array_mut()
        .ok_or_else(|| "Texture manifest entries is not an array".to_string())?;
    entries.retain(|item| {
        item.get("container") != Some(&json!(container))
            || item.get("asset") != Some(&json!(asset))
            || item.get("path_id") != Some(&json!(path_id))
    });
    entries.push(entry);
    let data = serde_json::to_string_pretty(&manifest)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(&manifest_path, format!("{data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    patch_config["AllowTextureResize"] = json!(true);
    let patch_data = serde_json::to_string_pretty(&patch_config)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(project.join("ffpatch.json"), format!("{patch_data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    Ok(format!(
        "Staged texture replacement: {}",
        target.to_string_lossy()
    ))
}

pub(in super::super) fn patch_texture_assets_native(
    extracted_dir: &Path,
    texture_dir: &Path,
    entries: &[serde_json::Value],
    container: &str,
    allow_resize: bool,
) -> Result<bool, String> {
    let assets = unique_manifest_values(entries, "asset", Some(container));
    let mut patched_any = false;
    for asset in assets {
        let status_path = build_status_path(extracted_dir, "texture", &asset);
        let mut args = vec![
            extracted_dir.to_string_lossy().to_string(),
            texture_dir.to_string_lossy().to_string(),
            "--asset".to_string(),
            asset,
            "--container".to_string(),
            container.to_string(),
            "--status-file".to_string(),
            status_path.to_string_lossy().to_string(),
        ];
        if allow_resize {
            args.push("--allow-resize".to_string());
        }
        fusionforge::build_patch::patch_texture_pngs(&args)?;
        if status_patched_count(&status_path, "patched") > 0 {
            patched_any = true;
        }
    }
    Ok(patched_any)
}
