use super::super::*;

pub(in super::super) fn npc_build_assets_manifest_relative(project: &Path, blueprint: &NpcBlueprint) -> String {
    let manifest_path = npc_build_assets_root(project, blueprint).join("_manifest.json");
    path_relative_to_project(project, &manifest_path)
        .unwrap_or_else(|| manifest_path.to_string_lossy().replace('\\', "/"))
}

pub(in super::super) fn set_npc_manifest_build_source_assets(
    project: &Path,
    manifest: &mut JsonValue,
    blueprint: &NpcBlueprint,
) {
    manifest["rawAssets"] = json!({
        "format": "fftools.npc-assets.v3",
        "layout": "build-source-assets",
        "manifest": npc_build_assets_manifest_relative(project, blueprint),
    });
}

pub(in super::super) fn write_npc_build_assets_manifest(
    project: &Path,
    blueprint: &NpcBlueprint,
    source_paths: &[String],
) -> Result<(), String> {
    let root = npc_build_assets_root(project, blueprint);
    fs::create_dir_all(&root).map_err(|err| format!("{}: {err}", root.display()))?;
    let asset_dirs = source_paths
        .iter()
        .map(|path| {
            let resolved = resolve_project_path(project, path);
            let files = if resolved.is_dir() {
                extracted_files_in_dir(&resolved)
                    .into_iter()
                    .filter(|file| !file.name.ends_with(".json"))
                    .map(|file| {
                        path_relative_to_project(project, Path::new(&file.path))
                            .unwrap_or(file.path)
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            json!({
                "path": path,
                "fileCount": files.len(),
                "files": files,
            })
        })
        .collect::<Vec<_>>();
    let manifest = json!({
        "format": "fftools.npc-build-assets.v1",
        "layout": "build-ready-source-assets",
        "npcId": blueprint.npc_id,
        "npcName": blueprint.name,
        "sourceBundles": source_paths,
        "assetDirs": asset_dirs,
    });
    let data = serde_json::to_string_pretty(&manifest).map_err(|err| err.to_string())?;
    fs::write(root.join("_manifest.json"), format!("{data}\n"))
        .map_err(|err| format!("{}: {err}", root.join("_manifest.json").display()))
}

pub(in super::super) fn npc_manifest_uses_build_source_assets(
    project: &Path,
    blueprint: &NpcBlueprint,
    manifest: &JsonValue,
    context: &NpcImportManifestContext,
) -> bool {
    manifest
        .get("sourceAssetStrategy")
        .and_then(JsonValue::as_str)
        == Some("import-time-build-source-assets")
        && manifest
            .get("rawAssets")
            .and_then(|value| value.get("layout"))
            .and_then(JsonValue::as_str)
            == Some("build-source-assets")
        && !context.source_paths.is_empty()
        && context
            .source_paths
            .iter()
            .all(|path| is_npc_build_asset_source_path(project, blueprint, path))
}

pub(in super::super) fn update_npc_manifest_build_source_assets(
    project: &Path,
    manifest: &mut JsonValue,
    blueprint: &NpcBlueprint,
    source_paths: &[String],
) -> Result<(), String> {
    manifest["sourceAssetStrategy"] = json!("import-time-build-source-assets");
    manifest["blueprint"] =
        serde_json::to_value(blueprint).map_err(|err| format!("serialize blueprint: {err}"))?;
    manifest["sourceBundles"] = json!(source_paths);
    set_npc_manifest_build_source_assets(project, manifest, blueprint);
    manifest["resources"] = json!({
        "model": {
            "bundle": blueprint.model_bundle.as_deref(),
            "asset": blueprint.model_asset.as_deref(),
        },
        "texture": {
            "bundle": blueprint.texture_bundle.as_deref(),
            "asset": blueprint.texture_asset.as_deref(),
        },
        "iconTexture": {
            "bundle": blueprint.icon_bundle.as_deref(),
            "asset": blueprint.icon_asset.as_deref(),
            "generated": blueprint.generated_icon.as_ref(),
        },
        "animation": {
            "bundle": blueprint.animation_set.as_deref(),
        },
        "audioSource": blueprint.audio_source.as_deref(),
        "audioPrefix": blueprint.audio_prefix.as_deref(),
    });
    Ok(())
}

pub(in super::super) fn is_npc_build_asset_source_path(project: &Path, blueprint: &NpcBlueprint, path: &Path) -> bool {
    let root = npc_build_assets_root(project, blueprint);
    path.starts_with(&root) && path.is_dir()
}

pub(in super::super) fn source_bundle_path_from_label(source_dir: &Path, label: &str) -> Option<PathBuf> {
    if label.trim().is_empty() {
        return None;
    }
    for extension in ["resourceFile", "unity3d"] {
        let candidate = source_dir.join(format!("{label}.{extension}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let direct_dir = source_dir.join(label);
    if direct_dir.is_dir() {
        return Some(direct_dir);
    }
    fs::read_dir(source_dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .is_some_and(|stem| stem.eq_ignore_ascii_case(label))
        })
}

pub(in super::super) fn recover_source_bundle_from_staged_path(
    project: &Path,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
    path: &Path,
) -> Option<PathBuf> {
    if !path.starts_with(npc_stage_root(project, blueprint)) {
        return None;
    }
    let label = npc_staged_source_label(path)?;
    let source_dir = context.source_table_data_bundle.as_ref()?.parent()?;
    source_bundle_path_from_label(source_dir, &label)
}

pub(in super::super) fn recovered_or_original_npc_source_path(
    project: &Path,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
    path: PathBuf,
) -> PathBuf {
    recover_source_bundle_from_staged_path(project, blueprint, context, &path).unwrap_or(path)
}

pub(in super::super) fn stage_npc_manifest_source_assets_to_root(
    project: &Path,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
    asset_stage_root: &Path,
    clear_existing_assets: bool,
) -> Result<(NpcBlueprint, Vec<String>), String> {
    let mut staged_blueprint = blueprint.clone();
    let staged_aliases = collect_npc_staged_source_aliases(project, blueprint, context);
    let preserve_stage_assets = npc_blueprint_uses_authoring_model_bundle(blueprint);
    if asset_stage_root.exists() && clear_existing_assets && !preserve_stage_assets {
        fs::remove_dir_all(asset_stage_root)
            .map_err(|err| format!("{}: {err}", asset_stage_root.display()))?;
    }
    fs::create_dir_all(asset_stage_root)
        .map_err(|err| format!("{}: {err}", asset_stage_root.display()))?;
    let mut hints = npc_asset_hints_from_patch(context, blueprint)?;
    if !npc_blueprint_uses_authoring_model_bundle(blueprint) {
        let source_hints = npc_asset_hints_from_source(project, context)?;
        extend_npc_asset_hints(&mut hints, source_hints);
    }
    if let Some(index) = load_npc_source_index(project, context) {
        add_npc_texture_asset_hints_from_index(&mut hints, blueprint, &index);
        add_npc_audio_asset_hints_from_index(&mut hints, blueprint, &index);
    }
    let source_paths = npc_augmented_source_bundles(project, blueprint, context, &hints);
    add_npc_audio_asset_hints_from_source_bundles(project, &source_paths, &mut hints)?;
    let mut staged_paths = Vec::new();
    let mut replacements = BTreeMap::<String, String>::new();
    for source_path in source_paths {
        if source_path.is_dir() {
            let relative = path_relative_to_project(project, &source_path)
                .unwrap_or_else(|| source_path.to_string_lossy().replace('\\', "/"));
            replacements.insert(
                source_path.to_string_lossy().to_ascii_lowercase(),
                relative.clone(),
            );
            staged_paths.push(relative);
            continue;
        }
        if !is_unity_bundle_path(&source_path) {
            continue;
        }
        let is_authoring = npc_blueprint_uses_authoring_model_bundle(blueprint);
        let is_model_source = blueprint
            .model_bundle
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| resolve_project_path(project, value))
            .is_some_and(|model_path| {
                model_path.to_string_lossy().to_ascii_lowercase()
                    == source_path.to_string_lossy().to_ascii_lowercase()
            });
        let staged_dir = match stage_npc_source_extract_dir(
            project,
            blueprint,
            &source_path,
            &hints,
            asset_stage_root,
        ) {
            Ok(staged_dir) => staged_dir,
            Err(_)
                if !is_authoring
                    && source_path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(is_npc_shared_dependency_bundle) =>
            {
                continue;
            }
            Err(err) if is_authoring && !is_model_source => {
                // Authoring NPCs only strictly need the generated model
                // bundle; a failed optional source (animation set, icon or
                // audio donor bundle) must not silently cancel the whole
                // standalone NPC bundle.
                eprintln!(
                    "NPC '{}': skipped optional source bundle {}: {err}",
                    blueprint.name,
                    source_path.display()
                );
                continue;
            }
            Err(err) => return Err(err),
        };
        let relative = path_relative_to_project(project, &staged_dir).ok_or_else(|| {
            format!(
                "{} is outside project {}",
                staged_dir.display(),
                project.display()
            )
        })?;
        replacements.insert(
            source_path.to_string_lossy().to_ascii_lowercase(),
            relative.clone(),
        );
        if let Some(label) = source_path.file_stem().and_then(|value| value.to_str()) {
            if let Some(old_paths) = staged_aliases.get(label) {
                for old_path in old_paths {
                    replacements.insert(
                        old_path.to_string_lossy().to_ascii_lowercase(),
                        relative.clone(),
                    );
                }
            }
        }
        staged_paths.push(relative);
    }
    staged_paths.sort();
    staged_paths.dedup();

    for field in [
        &mut staged_blueprint.model_bundle,
        &mut staged_blueprint.texture_bundle,
        &mut staged_blueprint.icon_bundle,
        &mut staged_blueprint.animation_set,
    ] {
        let Some(current) = field
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let resolved = resolve_project_path(project, current);
        let key = resolved.to_string_lossy().to_ascii_lowercase();
        if let Some(replacement) = replacements.get(&key) {
            *field = Some(replacement.clone());
        }
    }

    Ok((staged_blueprint, staged_paths))
}

pub(in super::super) fn stage_npc_manifest_source_assets(
    project: &Path,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
) -> Result<(NpcBlueprint, Vec<String>), String> {
    let asset_stage_root = npc_build_work_root(project, blueprint).join("assets");
    stage_npc_manifest_source_assets_to_root(project, blueprint, context, &asset_stage_root, true)
}

pub(in super::super) fn component_path_id_from_gameobject(
    value: &fusionforge::UnityValue,
    class_id: i64,
) -> Option<i64> {
    for component in fusionforge::value_array(value.get("m_Component")) {
        let fusionforge::UnityValue::Array(items) = component else {
            continue;
        };
        if items.len() < 2 || items[0].as_i64() != Some(class_id) {
            continue;
        }
        let pointer = items[1].as_pointer()?;
        if pointer.file_id == 0 {
            return Some(pointer.path_id);
        }
    }
    None
}

pub(in super::super) fn transform_path_id_from_gameobject(value: &fusionforge::UnityValue) -> Option<i64> {
    component_path_id_from_gameobject(value, 4)
}

pub(in super::super) fn transform_path_id_from_gameobject_key(
    env: &fusionforge::UnityEnvironment,
    gameobject_key: (usize, i64),
) -> Option<i64> {
    let asset = env.assets.get(gameobject_key.0)?;
    let info = asset.objects.get(&gameobject_key.1)?;
    let body = asset.read_object(gameobject_key.0, info).ok()?;
    transform_path_id_from_gameobject(&body)
}

pub(in super::super) fn clean_asset_bundle(
    asset: &fusionforge::Asset,
    bundle_name: &str,
    model_path: &str,
    root_path_id: i64,
    texture_path: &str,
    texture_path_id: i64,
    external_preload_pointers: &[fusionforge::Pointer],
    preload_path_ids: &[i64],
) -> Result<fusionforge::UnityValue, String> {
    let mut value = asset.empty_object_value_for_class(142)?;
    let model_preload_path_ids = preload_path_ids.to_vec();
    let external_preloads = external_preload_pointers.to_vec();
    let shared_material_preload_count = external_preloads.len() as i64;
    let texture_preload_index = shared_material_preload_count + model_preload_path_ids.len() as i64;
    let mut preload_table = Vec::with_capacity(
        shared_material_preload_count as usize + model_preload_path_ids.len() + 1,
    );
    preload_table.extend(
        external_preloads
            .iter()
            .map(|pointer| unity_external_pointer(pointer.file_id, pointer.path_id)),
    );
    preload_table.extend(
        model_preload_path_ids
            .iter()
            .map(|path_id| unity_local_pointer(0, *path_id)),
    );
    preload_table.push(unity_local_pointer(0, texture_path_id));
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "m_Name".to_string(),
            fusionforge::UnityValue::String(bundle_name.to_string()),
        );
        object.insert(
            "m_MainAsset".to_string(),
            fusionforge::UnityValue::Object(BTreeMap::from([
                ("asset".to_string(), unity_local_pointer(0, root_path_id)),
                ("preloadIndex".to_string(), fusionforge::UnityValue::Int(0)),
                ("preloadSize".to_string(), fusionforge::UnityValue::Int(0)),
            ])),
        );
        object.insert(
            "m_PreloadTable".to_string(),
            fusionforge::UnityValue::Array(preload_table),
        );
        let model_preload_size =
            shared_material_preload_count + model_preload_path_ids.len() as i64;
        let model_entry_metadata = || {
            fusionforge::UnityValue::Object(BTreeMap::from([
                ("asset".to_string(), unity_local_pointer(0, root_path_id)),
                ("preloadIndex".to_string(), fusionforge::UnityValue::Int(0)),
                (
                    "preloadSize".to_string(),
                    fusionforge::UnityValue::Int(model_preload_size),
                ),
            ]))
        };
        let mut container = vec![unity_pair(
            fusionforge::UnityValue::String(model_path.to_string()),
            model_entry_metadata(),
        )];
        // Original NPC bundles expose the model under BOTH mob/<name>.kfm and
        // mob/<name>.nif (the client may request either one; the NIF path is
        // what the mesh loader asks for). Without the .nif alias the NPC
        // spawns invisible.
        let lower_model_path = model_path.to_ascii_lowercase();
        if let Some(stem) = lower_model_path.strip_suffix(".kfm") {
            container.push(unity_pair(
                fusionforge::UnityValue::String(format!("{stem}.nif")),
                model_entry_metadata(),
            ));
        }
        container.push(unity_pair(
            fusionforge::UnityValue::String(texture_path.to_string()),
            fusionforge::UnityValue::Object(BTreeMap::from([
                ("asset".to_string(), unity_local_pointer(0, texture_path_id)),
                (
                    "preloadIndex".to_string(),
                    fusionforge::UnityValue::Int(texture_preload_index),
                ),
                ("preloadSize".to_string(), fusionforge::UnityValue::Int(1)),
            ])),
        ));
        object.insert(
            "m_Container".to_string(),
            fusionforge::UnityValue::Array(container),
        );
    }
    Ok(value)
}

pub(in super::super) fn asset_refs_file_id_for(asset_refs: &mut Vec<fusionforge::AssetRef>, file_path: &str) -> i32 {
    if let Some(index) = asset_refs
        .iter()
        .skip(1)
        .position(|reference| reference.file_path.eq_ignore_ascii_case(file_path))
    {
        return index as i32 + 1;
    }
    asset_refs.push(fusionforge::AssetRef {
        asset_path: String::new(),
        guid: [0; 16],
        type_id: 0,
        file_path: file_path.to_string(),
    });
    asset_refs.len() as i32 - 1
}

pub(in super::super) fn is_launcher_manifest_document(document: &JsonValue) -> bool {
    document
        .get("bundles")
        .and_then(JsonValue::as_object)
        .is_some()
        && document
            .get("main_file_url")
            .and_then(JsonValue::as_str)
            .is_some()
}

pub(in super::super) fn build_manifest_paths(build_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(build_dir) else {
        return Vec::new();
    };
    let mut paths = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        })
        .filter(|path| {
            fs::read_to_string(path)
                .ok()
                .and_then(|data| serde_json::from_str::<JsonValue>(&data).ok())
                .is_some_and(|document| is_launcher_manifest_document(&document))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub(in super::super) fn build_output_manifest_file_name(project: &Path, build_name: &str) -> String {
    let project_name = project
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .trim();
    if let Some(stem) = project_name.strip_suffix(".ffclient") {
        let stem = stem.trim();
        if !stem.is_empty() {
            return format!("{stem}.json");
        }
    }
    format!("{}.json", safe_segment(build_name))
}

pub(in super::super) fn launcher_manifest_uuid(path: &Path) -> Result<Option<String>, String> {
    let data = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let document = serde_json::from_str::<JsonValue>(&data)
        .map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(document
        .get("uuid")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string))
}

pub(in super::super) fn rewrite_local_launcher_manifest_urls(
    manifest_path: &Path,
    build_dir: &Path,
) -> Result<(), String> {
    let absolute_build_dir = if build_dir.is_absolute() {
        build_dir.to_path_buf()
    } else if let Some(root) = workspace_root_dir() {
        root.join(build_dir)
    } else {
        build_dir.to_path_buf()
    };
    let mut manifest = serde_json::from_str::<JsonValue>(
        &fs::read_to_string(manifest_path)
            .map_err(|err| format!("{}: {err}", manifest_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    if !is_launcher_manifest_document(&manifest) {
        return Ok(());
    }
    let asset_url = ffbuildtool::util::file_path_to_uri(&absolute_build_dir.to_string_lossy());
    let main_file_url = ffbuildtool::util::file_path_to_uri(
        &absolute_build_dir.join("main.unity3d").to_string_lossy(),
    );
    manifest["asset_url"] = json!(asset_url);
    manifest["main_file_url"] = json!(main_file_url);
    let data = serde_json::to_string_pretty(&manifest)
        .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    fs::write(manifest_path, format!("{data}\n"))
        .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    Ok(())
}

pub(in super::super) fn prune_build_manifest_files(build_dir: &Path, keep_path: &Path) -> Result<(), String> {
    for path in build_manifest_paths(build_dir) {
        if path != keep_path && path.is_file() {
            fs::remove_file(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        }
    }
    Ok(())
}

pub(in super::super) fn asset_max_path_id(asset: &fusionforge::Asset) -> i64 {
    asset
        .objects
        .keys()
        .copied()
        .map(|path_id| path_id.max(0))
        .max()
        .unwrap_or(0)
}

pub(in super::super) fn load_npc_source_index(
    project: &Path,
    context: &NpcImportManifestContext,
) -> Option<ClientFileIndex> {
    let source_dir = context.source_table_data_bundle.as_ref()?.parent()?;
    let target_project_dir = project.to_string_lossy().to_string();
    for import_project in
        npc_source_import_project_candidates(source_dir, Some(target_project_dir.clone()))
    {
        let index_path = import_project.join("cache").join("bundle-index.json");
        let index_data = fs::read_to_string(&index_path).ok().or_else(|| {
            index_client_project(
                source_dir.to_string_lossy().to_string(),
                import_project.to_string_lossy().to_string(),
            )
            .ok()?;
            fs::read_to_string(index_path).ok()
        });
        let Some(data) = index_data else {
            continue;
        };
        if let Ok(index) = serde_json::from_str::<ClientFileIndex>(&data) {
            return Some(index);
        }
    }
    None
}

pub(in super::super) fn client_index_container_paths(index: &ClientFileIndex) -> BTreeSet<String> {
    index
        .bundles
        .iter()
        .flat_map(|bundle| bundle.assets.iter())
        .flat_map(|asset| asset.container_paths.iter())
        .map(|path| normalized_asset_path(path))
        .collect()
}

pub(in super::super) fn npc_source_index_container_paths(
    project: &Path,
    context: &NpcImportManifestContext,
) -> Option<BTreeSet<String>> {
    let index = load_npc_source_index(project, context)?;
    Some(client_index_container_paths(&index))
}

pub(in super::super) fn npc_clean_asset_name(value: &str) -> Option<String> {
    let normalized = value.trim().replace('\\', "/");
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("null") {
        return None;
    }
    Some(normalized)
}

pub(in super::super) fn npc_asset_basename(value: &str) -> String {
    let normalized = normalized_asset_path(value);
    let without_extension = normalized
        .rsplit_once('.')
        .filter(|(_, extension)| extension.chars().all(|ch| ch.is_ascii_alphanumeric()))
        .map(|(base, _)| base)
        .unwrap_or(&normalized);
    without_extension
        .rsplit('/')
        .next()
        .unwrap_or(without_extension)
        .to_string()
}

pub(in super::super) fn add_npc_asset_hint(hints: &mut NpcAssetHints, value: &str, model: bool, texture: bool) {
    let Some(value) = npc_clean_asset_name(value) else {
        return;
    };
    add_npc_token_hint(hints, &value);

    let lower = normalized_asset_path(&value);
    if lower.contains('/') {
        hints.exact_paths.insert(lower.clone());
    }
    let base = npc_asset_basename(&value);
    if base.is_empty() {
        return;
    }
    if model {
        let kfm = format!("mob/{base}.kfm");
        let nif = format!("mob/{base}.nif");
        hints.exact_paths.insert(kfm.clone());
        hints.exact_paths.insert(nif.clone());
        hints.model_paths.insert(kfm);
        hints.model_paths.insert(nif);
        hints.model_names.insert(base.clone());
    }
    if texture {
        let dds = format!("texture/{base}.dds");
        hints.exact_paths.insert(dds.clone());
        hints.texture_paths.insert(dds);
    }
}

pub(in super::super) fn npc_icon_asset_paths(icon_type: i64, icon_number: i64) -> Vec<String> {
    npc_icon_runtime_asset_path(icon_type, icon_number)
        .map(|path| vec![path])
        .unwrap_or_default()
}

pub(in super::super) fn add_npc_icon_asset_hints(hints: &mut NpcAssetHints, icon_row: &fusionforge::UnityValue) {
    let Some(icon_number) = icon_row
        .get("m_iIconNumber")
        .and_then(fusionforge::UnityValue::as_i64)
    else {
        return;
    };
    let Some(icon_type) = icon_row
        .get("m_iIconType")
        .and_then(fusionforge::UnityValue::as_i64)
    else {
        return;
    };
    for path in npc_icon_asset_paths(icon_type, icon_number) {
        hints.exact_paths.insert(path.clone());
        hints.icon_paths.insert(path);
    }
}

pub(in super::super) fn collect_npc_asset_hints_from_table(
    npc_table: &fusionforge::UnityValue,
    npc_id: usize,
    hints: &mut NpcAssetHints,
) {
    let Some(npc_data) = npc_table_array(npc_table, "m_pNpcData").get(npc_id) else {
        return;
    };
    let string_index = unity_usize_field(npc_data, "m_iNpcName").unwrap_or(npc_id);
    if let Some(string_row) = npc_table_array(npc_table, "m_pNpcStringData").get(string_index) {
        if let Some(name) = unity_string_field(Some(string_row), "m_strName") {
            add_npc_token_hint(hints, &name);
        }
    }
    let mesh_index = unity_usize_field(npc_data, "m_iMesh");
    if let Some(mesh_index) = mesh_index {
        if let Some(mesh_row) = npc_table_array(npc_table, "m_pNpcMeshData").get(mesh_index) {
            for key in ["m_pstrMMeshModelString", "m_pstrFMeshModelString"] {
                if let Some(value) = unity_string_field(Some(mesh_row), key) {
                    add_npc_asset_hint(hints, &value, true, false);
                }
            }
            for key in [
                "m_pstrMTextureString",
                "m_pstrMTextureString2",
                "m_pstrFTextureString",
                "m_pstrFTextureString2",
            ] {
                if let Some(value) = unity_string_field(Some(mesh_row), key) {
                    add_npc_asset_hint(hints, &value, false, true);
                }
            }
        }
    }
    if let Some(icon_index) = unity_usize_field(npc_data, "m_iIcon1") {
        if let Some(icon_row) = npc_table_array(npc_table, "m_pNpcIconData").get(icon_index) {
            add_npc_icon_asset_hints(hints, icon_row);
        }
    }
}

pub(in super::super) fn npc_asset_hints_from_patch(
    context: &NpcImportManifestContext,
    blueprint: &NpcBlueprint,
) -> Result<NpcAssetHints, String> {
    let mut hints = NpcAssetHints::default();
    let Some(path) = context
        .table_data_patch_path
        .as_ref()
        .filter(|path| path.exists())
    else {
        return Ok(hints);
    };
    let body = table_data_patch_value(path, 0)?;
    if let Some(npc_table) = npc_table_from_body(&body) {
        if let Ok(npc_id) = usize::try_from(blueprint.npc_id) {
            collect_npc_asset_hints_from_table(npc_table, npc_id, &mut hints);
        }
    }
    Ok(hints)
}
