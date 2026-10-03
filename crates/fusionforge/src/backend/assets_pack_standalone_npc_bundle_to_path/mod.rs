use super::super::*;

pub(in super::super) fn pack_standalone_npc_bundle_to_path(
    project: &Path,
    source_blueprint: &NpcBlueprint,
    blueprint: &NpcBlueprint,
    context: &NpcImportManifestContext,
    bundle_path: &Path,
    target_serialized_format: Option<u32>,
    target_external_index: &NpcTargetExternalIndex,
    target_type_trees: &NpcTargetTypeTreeIndex,
) -> Result<(Vec<String>, Vec<String>, Vec<String>), String> {
    let mut hints = npc_asset_hints_from_patch(context, blueprint)?;
    if !npc_blueprint_uses_authoring_model_bundle(blueprint) {
        let source_hints = npc_asset_hints_from_source(project, context)?;
        extend_npc_asset_hints(&mut hints, source_hints);
    }
    let source_index = load_npc_source_index(project, context);
    if let Some(index) = source_index.as_ref() {
        add_npc_texture_asset_hints_from_index(&mut hints, blueprint, index);
        add_npc_audio_asset_hints_from_index(&mut hints, blueprint, index);
    }
    let source_bundles = npc_augmented_source_bundles(project, blueprint, context, &hints);
    if source_bundles.is_empty() {
        return Err(
            "NPC blueprint needs at least Model Bundle to create a standalone bundle.".to_string(),
        );
    }
    add_npc_audio_asset_hints_from_source_bundles(project, &source_bundles, &mut hints)?;
    if let Some(parent) = bundle_path.parent() {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    let output_temp = native_build_temp_dir("npc_bundle")?;
    let mut warnings = Vec::new();
    let source_bundle_container_paths =
        npc_container_paths_from_source_bundles(project, &source_bundles)?;
    let source_index_paths = if source_bundle_container_paths.is_empty() {
        source_index
            .as_ref()
            .map(client_index_container_paths)
            .or_else(|| npc_source_index_container_paths(project, context))
    } else {
        Some(source_bundle_container_paths)
    };
    let mut required_hints = hints.clone();
    required_hints.model_paths.clear();
    required_hints.texture_paths.clear();
    required_hints.icon_paths.clear();
    let required_container_paths = npc_standalone_required_container_paths(
        source_blueprint,
        &required_hints,
        source_index_paths.as_ref(),
    );
    let output_path_overrides =
        npc_standalone_output_path_overrides(source_blueprint, blueprint, &hints);
    let generated_icon_import = npc_generated_icon_import(project, blueprint)?;
    if required_container_paths.is_empty() {
        return Err(format!(
            "NPC '{}' has no exact model/texture/icon container paths to build a standalone bundle.",
            blueprint.name
        ));
    }
    if npc_bundle_should_use_safe_imported_fallback(blueprint) {
        pack_npc_source_assets_fallback_bundle(
            project,
            &source_bundles,
            &output_path_overrides,
            &hints,
            bundle_path,
        )?;
        warnings.push(
            "Imported NPC bundle packaged from staged source assets to preserve the original dependency graph."
                .to_string(),
        );
        return Ok((Vec::new(), Vec::new(), warnings));
    }

    let mut assets = Vec::<fusionforge::Asset>::new();
    for source_path in &source_bundles {
        if !(source_path.is_file() || source_path.is_dir()) {
            return Err(
                EditorError::MissingPath(source_path.to_string_lossy().to_string()).to_string(),
            );
        }
        assets.extend(npc_source_assets_from_path(project, source_path)?);
    }
    if assets.is_empty() {
        return Err("Source NPC bundles did not contain serialized asset files.".to_string());
    }

    let env = fusionforge::UnityEnvironment::from_assets(assets);
    let mut selected = BTreeSet::<(usize, i64)>::new();
    let mut queue = VecDeque::<(usize, i64)>::new();
    let mut assetbundle_replacements = BTreeMap::<(usize, i64), fusionforge::UnityValue>::new();
    let mut assetbundle_quality = BTreeMap::<(usize, i64), i32>::new();
    let mut matched_paths = Vec::<String>::new();
    let mut object_name_overrides = BTreeMap::<(usize, i64), String>::new();
    let mut generated_icon_texture_keys = BTreeSet::<(usize, i64)>::new();
    let mut matched_generated_icon_template = generated_icon_import.is_none();
    let mut missing_required_roots = Vec::<String>::new();

    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let body = asset.read_object(asset_index, info)?;
            let preload_table = fusionforge::value_array(body.get("m_PreloadTable")).to_vec();
            let mut kept_paths = BTreeSet::<String>::new();
            let mut asset_pointer_overrides = BTreeMap::<String, fusionforge::UnityValue>::new();
            let mut kept_quality = 0;

            for entry in fusionforge::value_array(body.get("m_Container")) {
                let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                    continue;
                };
                if !npc_standalone_container_path_matches(path, &required_container_paths) {
                    continue;
                }
                let Some(container_pointer) = metadata
                    .get("asset")
                    .and_then(fusionforge::UnityValue::as_pointer)
                else {
                    warnings.push(format!(
                        "{}: matched container path {path} has no asset pointer",
                        asset.name
                    ));
                    continue;
                };
                let mut pointer = container_pointer.clone();
                let mut pointer_quality =
                    npc_container_pointer_quality(&env, asset_index, path, &pointer);
                let (start, end) = metadata_preload_range(metadata, preload_table.len());
                if pointer_quality == 0 {
                    if let Some((preload_pointer, preload_quality)) =
                        npc_container_preload_root_pointer(
                            &env,
                            asset_index,
                            path,
                            &preload_table,
                            metadata,
                        )
                    {
                        pointer = preload_pointer;
                        pointer_quality = preload_quality;
                        warnings.push(format!(
                            "{}: recovered stale container path {path} from preload root",
                            asset.name
                        ));
                    } else if let Some((named_pointer, named_quality)) =
                        npc_container_named_object_pointer(&env, path)
                    {
                        pointer = named_pointer;
                        pointer_quality = named_quality;
                        warnings.push(format!(
                            "{}: recovered stale container path {path} from named object",
                            asset.name
                        ));
                    }
                }
                if pointer_quality == 0 {
                    missing_required_roots.push(path.to_string());
                    warnings.push(format!(
                        "{}: skipped stale container path {path}; asset pointer did not resolve to a readable {}",
                        asset.name,
                        npc_container_expected_object_type(path).unwrap_or("object")
                    ));
                    continue;
                }

                kept_paths.insert(path.to_string());
                asset_pointer_overrides.insert(
                    path.to_string(),
                    fusionforge::UnityValue::Pointer(pointer.clone()),
                );
                kept_quality = kept_quality.max(pointer_quality);
                matched_paths.push(format!("{}: {path}", asset.name));
                if let Some(output_path) = output_path_overrides.get(&normalized_asset_path(path)) {
                    if let Some(output_name) = asset_stem_from_container_path(output_path) {
                        for key in pointer_candidate_keys(&env, &pointer) {
                            object_name_overrides.insert((key.0, key.1), output_name.clone());
                        }
                    }
                }
                if let Some(generated_icon) = generated_icon_import.as_ref() {
                    let lower_path = normalized_asset_path(path);
                    if lower_path == generated_icon.template_path
                        || lower_path.ends_with(&format!("/{}", generated_icon.template_path))
                    {
                        matched_generated_icon_template = true;
                        for key in pointer_candidate_keys(&env, &pointer) {
                            generated_icon_texture_keys.insert((key.0, key.1));
                            object_name_overrides
                                .insert((key.0, key.1), generated_icon.target_name.clone());
                        }
                    }
                }
                add_resolved_pointer_root(&env, &pointer, &mut selected, &mut queue);
                if pointer_quality >= 2 && npc_container_should_follow_preload(path) {
                    if start == end {
                        warnings.push(format!(
                            "{}: matched container path {path} has no preload dependencies",
                            asset.name
                        ));
                    }
                    for preload in &preload_table[start..end] {
                        if let Some(pointer) = preload.as_pointer() {
                            add_resolved_pointer_root(&env, pointer, &mut selected, &mut queue);
                        }
                    }
                }
                if pointer_quality == 1 && start != end {
                    warnings.push(format!(
                        "{}: ignored stale preload list for {path}; using resolved object dependency closure instead",
                        asset.name
                    ));
                }
            }

            if !kept_paths.is_empty() {
                selected.insert((asset_index, info.path_id));
                assetbundle_quality.insert((asset_index, info.path_id), kept_quality);
                assetbundle_replacements.insert(
                    (asset_index, info.path_id),
                    filtered_assetbundle_value(
                        &body,
                        &kept_paths,
                        &output_path_overrides,
                        &hints,
                        &asset_pointer_overrides,
                    ),
                );
            }
        }
    }

    if matched_paths.is_empty() {
        return Err(format!(
            "No container paths matched NPC '{}' in selected source bundles; refusing to pack whole bundles.",
            blueprint.name
        ));
    }
    if !matched_generated_icon_template {
        let template_path = generated_icon_import
            .as_ref()
            .map(|icon| icon.template_path.as_str())
            .unwrap_or("");
        return Err(format!(
            "Generated ICON template texture '{template_path}' was not found in selected icon/template bundles."
        ));
    }
    if let Some(generated_icon) = generated_icon_import.as_ref() {
        warnings.push(format!(
            "Generated NPC ICON: {} -> {}",
            generated_icon.template_path, generated_icon.target_path
        ));
    }
    if !missing_required_roots.is_empty() {
        return Err(format!(
            "Refusing to pack whole source bundles for NPC '{}': standalone root resolution failed for: {}",
            blueprint.name,
            missing_required_roots.join(", ")
        ));
    }

    while let Some((asset_index, path_id)) = queue.pop_front() {
        let Some(asset) = env.assets.get(asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(&path_id) else {
            continue;
        };
        if asset.object_type_name(info) == "AssetBundle" {
            continue;
        }
        let Ok(body) = asset.read_object(asset_index, info) else {
            warnings.push(format!(
                "Could not read dependency {}#{}",
                asset.name, path_id
            ));
            continue;
        };
        let mut pointers = Vec::new();
        collect_value_pointers(&body, &mut pointers);
        for pointer in pointers {
            add_resolved_pointer_root(&env, &pointer, &mut selected, &mut queue);
        }
    }

    let source_container_paths = npc_source_container_paths_by_key(&env);
    let externalized = if npc_bundle_should_externalize_dependencies(blueprint) {
        npc_externalized_pointer_map(
            &env,
            &selected,
            &source_container_paths,
            &required_container_paths,
            target_external_index,
        )
    } else {
        BTreeMap::new()
    };
    let referenced_external_asset_names =
        npc_referenced_external_asset_names(&env, &selected, &externalized)?;
    let (mut external_asset_refs, mut external_ref_indices) =
        npc_external_asset_refs(&externalized, &referenced_external_asset_names);
    let explicit_external_asset_refs =
        npc_collect_explicit_external_asset_refs(&env, &selected, &externalized)?;
    let mut explicit_external_ref_indices = BTreeMap::<(String, [u8; 16], i32, String), i32>::new();
    for asset_ref in explicit_external_asset_refs {
        let key = explicit_asset_ref_key(&asset_ref);
        if explicit_external_ref_indices.contains_key(&key) {
            continue;
        }
        let file_id = external_asset_refs.len() as i32 + 1;
        explicit_external_ref_indices.insert(key, file_id);
        external_asset_refs.push(asset_ref);
    }
    let shared_shader_ref = if !npc_blueprint_uses_authoring_model_bundle(blueprint) {
        default_npc_shared_shader_ref(target_external_index).map(|reference| {
            let file_id = ensure_external_asset_ref(
                &mut external_asset_refs,
                &mut external_ref_indices,
                &reference.asset_name,
            );
            (reference, file_id)
        })
    } else {
        None
    };
    if !externalized.is_empty() {
        let mut summaries = externalized
            .iter()
            .take(8)
            .map(|((asset_index, path_id), reference)| {
                format!(
                    "{}#{} -> {}#{} {} {}",
                    env.asset_name(*asset_index),
                    path_id,
                    reference.asset_name,
                    reference.path_id,
                    reference.object_type,
                    reference.object_name
                )
            })
            .collect::<Vec<_>>();
        if externalized.len() > summaries.len() {
            summaries.push(format!("+{} more", externalized.len() - summaries.len()));
        }
        warnings.push(format!(
            "Externalized {} target shared object(s): {}",
            externalized.len(),
            summaries.join("; ")
        ));
    }

    let primary_asset_index = assetbundle_replacements
        .iter()
        .filter_map(|(key, value)| {
            assetbundle_value_has_container_prefix(value, "mob/").then_some((
                assetbundle_quality.get(key).copied().unwrap_or_default(),
                key.0,
            ))
        })
        .max_by_key(|(quality, asset_index)| (*quality, std::cmp::Reverse(*asset_index)))
        .map(|(_, asset_index)| asset_index);
    let output_path_ids =
        npc_compact_output_path_ids(&env, &selected, &externalized, primary_asset_index);
    let referenced_external_pointers = npc_referenced_external_output_pointers(
        &env,
        &selected,
        &externalized,
        &external_ref_indices,
    )?;
    // Shader pointers that are valid in the output bundle: externalized target
    // shaders and shader objects kept locally. Materials whose shader pointer
    // is not in this set fall back to the default shared NPC shader.
    let mut valid_shader_pointers = BTreeSet::<(i32, i64)>::new();
    for reference in externalized.values() {
        if reference.object_type == "Shader" {
            if let Some(file_id) = external_ref_indices.get(&reference.asset_name) {
                valid_shader_pointers.insert((*file_id, reference.path_id));
            }
        }
    }
    if let Some((reference, file_id)) = shared_shader_ref.as_ref() {
        valid_shader_pointers.insert((*file_id, reference.path_id));
    }
    for key in &selected {
        if externalized.contains_key(key) {
            continue;
        }
        let Some(asset) = env.assets.get(key.0) else {
            continue;
        };
        let Some(info) = asset.objects.get(&key.1) else {
            continue;
        };
        if asset.object_type_name(info) == "Shader" {
            let output_id = output_path_ids.get(key).copied().unwrap_or(key.1);
            valid_shader_pointers.insert((0, output_id));
        }
    }
    let merged_primary_assetbundle = primary_asset_index.and_then(|primary_asset_index| {
        merge_assetbundle_replacements_to_primary(
            &assetbundle_replacements,
            &env,
            primary_asset_index,
            &selected,
            &externalized,
            &external_ref_indices,
            &explicit_external_ref_indices,
            &output_path_ids,
            &hints,
        )
    });
    let standalone_asset_name = standalone_npc_asset_name(bundle_path, blueprint);
    let local_output_path_ids = output_path_ids.values().copied().collect::<BTreeSet<_>>();
    let valid_external_file_ids = external_ref_indices
        .values()
        .copied()
        .collect::<BTreeSet<_>>();
    let local_texture_pointers = npc_local_texture_output_pointers(
        &env,
        &selected,
        &externalized,
        &output_path_ids,
        &object_name_overrides,
    );
    let npc_main_texture_pointer =
        npc_material_main_texture_pointer(blueprint, &local_texture_pointers);
    let mut copied_files = Vec::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        let keep_ids = selected
            .iter()
            .filter_map(|key| {
                (key.0 == asset_index && !externalized.contains_key(key)).then_some(key.1)
            })
            .collect::<BTreeSet<_>>();
        if keep_ids.is_empty() {
            continue;
        }
        if primary_asset_index.is_some_and(|primary| primary != asset_index) {
            continue;
        }
        let mut data_replacements = BTreeMap::<i64, Vec<u8>>::new();
        let keep_infos = keep_ids
            .iter()
            .filter_map(|path_id| asset.objects.get(path_id))
            .collect::<Vec<_>>();
        let current_asset_output_path_ids = output_path_ids
            .iter()
            .filter_map(|((object_asset_index, source_path_id), output_path_id)| {
                (*object_asset_index == asset_index).then_some((*source_path_id, *output_path_id))
            })
            .collect::<BTreeMap<_, _>>();
        let mut written_output_path_ids = current_asset_output_path_ids
            .values()
            .copied()
            .collect::<BTreeSet<_>>();
        let output_asset = npc_output_asset_with_target_type_trees(
            asset,
            target_serialized_format,
            target_type_trees,
            keep_infos.iter().copied(),
        );
        if primary_asset_index == Some(asset_index) {
            for path_id in &keep_ids {
                let info = asset
                    .objects
                    .get(path_id)
                    .ok_or_else(|| format!("{}#{} not found", asset.name, path_id))?;
                let mut value = if let Some((key, value)) = &merged_primary_assetbundle {
                    if *key == (asset_index, *path_id) {
                        value.clone()
                    } else if let Some(value) =
                        assetbundle_replacements.get(&(asset_index, *path_id))
                    {
                        value.clone()
                    } else {
                        asset.read_object(asset_index, info)?
                    }
                } else if let Some(value) = assetbundle_replacements.get(&(asset_index, *path_id)) {
                    value.clone()
                } else {
                    asset.read_object(asset_index, info)?
                };
                rewrite_npc_output_pointers(
                    &mut value,
                    &env,
                    asset_index,
                    &selected,
                    &externalized,
                    &external_ref_indices,
                    &explicit_external_ref_indices,
                    &output_path_ids,
                );
                if generated_icon_texture_keys.contains(&(asset_index, *path_id)) {
                    if asset.object_type_name(info) != "Texture2D" {
                        return Err(format!(
                            "Generated ICON template {}#{} is {}, expected Texture2D",
                            asset.name,
                            path_id,
                            asset.object_type_name(info)
                        ));
                    }
                    if let Some(generated_icon) = generated_icon_import.as_ref() {
                        fusionforge::modding::apply_texture_import(
                            &mut value,
                            generated_icon.texture.clone(),
                        )?;
                    }
                }
                if let Some(name) = object_name_overrides.get(&(asset_index, *path_id)) {
                    set_unity_object_string(&mut value, "m_Name", name);
                }
                if !npc_blueprint_uses_authoring_model_bundle(blueprint) {
                    normalize_imported_npc_output_object(&asset.object_type_name(info), &mut value);
                    if asset.object_type_name(info) == "Material" {
                        let shader_override =
                            material_shader_needs_default(&value, &valid_shader_pointers)
                                .then(|| {
                                    shared_shader_ref
                                        .as_ref()
                                        .map(|(reference, file_id)| (reference, *file_id))
                                })
                                .flatten();
                        normalize_imported_material_output(
                            &mut value,
                            shader_override,
                            &local_output_path_ids,
                            &valid_external_file_ids,
                            npc_main_texture_pointer.clone(),
                        );
                    }
                }
                if asset.object_type_name(info) == "AssetBundle" {
                    set_unity_object_string(&mut value, "m_Name", &standalone_asset_name);
                    if !normalize_clean_npc_assetbundle_preloads(
                        &mut value,
                        &local_output_path_ids,
                        &referenced_external_pointers,
                    ) {
                        filter_assetbundle_preloads_to_output_paths(
                            &mut value,
                            &local_output_path_ids,
                            &referenced_external_pointers,
                        );
                    }
                }
                data_replacements.insert(
                    *path_id,
                    serialize_npc_output_object_value(&output_asset, asset_index, info, &value)?,
                );
            }
        } else {
            for ((replacement_asset, path_id), value) in &assetbundle_replacements {
                if *replacement_asset != asset_index {
                    continue;
                }
                let info = asset
                    .objects
                    .get(path_id)
                    .ok_or_else(|| format!("{}#{} not found", asset.name, path_id))?;
                let mut value = value.clone();
                rewrite_npc_output_pointers(
                    &mut value,
                    &env,
                    asset_index,
                    &selected,
                    &externalized,
                    &external_ref_indices,
                    &explicit_external_ref_indices,
                    &output_path_ids,
                );
                if !npc_blueprint_uses_authoring_model_bundle(blueprint) {
                    normalize_imported_npc_output_object(&asset.object_type_name(info), &mut value);
                    if asset.object_type_name(info) == "Material" {
                        let shader_override =
                            material_shader_needs_default(&value, &valid_shader_pointers)
                                .then(|| {
                                    shared_shader_ref
                                        .as_ref()
                                        .map(|(reference, file_id)| (reference, *file_id))
                                })
                                .flatten();
                        normalize_imported_material_output(
                            &mut value,
                            shader_override,
                            &local_output_path_ids,
                            &valid_external_file_ids,
                            npc_main_texture_pointer.clone(),
                        );
                    }
                }
                if asset.object_type_name(info) == "AssetBundle" {
                    set_unity_object_string(&mut value, "m_Name", &standalone_asset_name);
                    if !normalize_clean_npc_assetbundle_preloads(
                        &mut value,
                        &local_output_path_ids,
                        &referenced_external_pointers,
                    ) {
                        filter_assetbundle_preloads_to_output_paths(
                            &mut value,
                            &local_output_path_ids,
                            &referenced_external_pointers,
                        );
                    }
                }
                data_replacements.insert(
                    *path_id,
                    serialize_npc_output_object_value(&output_asset, asset_index, info, &value)?,
                );
            }
        }
        let mut extra_objects = Vec::<(fusionforge::ObjectInfo, Vec<u8>)>::new();
        let mut extra_type_trees = BTreeMap::<i32, fusionforge::TypeTree>::new();
        if primary_asset_index == Some(asset_index) {
            for (extra_asset_index, extra_asset) in env.assets.iter().enumerate() {
                if extra_asset_index == asset_index {
                    continue;
                }
                for (selected_asset_index, path_id) in &selected {
                    if *selected_asset_index != extra_asset_index {
                        continue;
                    }
                    if externalized.contains_key(&(*selected_asset_index, *path_id)) {
                        continue;
                    }
                    let target_output_path_id = output_path_ids
                        .get(&(*selected_asset_index, *path_id))
                        .copied()
                        .unwrap_or(*path_id);
                    let Some(info) = extra_asset.objects.get(path_id) else {
                        continue;
                    };
                    if extra_asset.object_type_name(info) == "AssetBundle" {
                        continue;
                    }
                    if !written_output_path_ids.insert(target_output_path_id) {
                        continue;
                    }
                    let mut value = extra_asset.read_object(extra_asset_index, info)?;
                    rewrite_npc_output_pointers(
                        &mut value,
                        &env,
                        asset_index,
                        &selected,
                        &externalized,
                        &external_ref_indices,
                        &explicit_external_ref_indices,
                        &output_path_ids,
                    );
                    if generated_icon_texture_keys.contains(&(*selected_asset_index, *path_id)) {
                        if extra_asset.object_type_name(info) != "Texture2D" {
                            return Err(format!(
                                "Generated ICON template {}#{} is {}, expected Texture2D",
                                extra_asset.name,
                                path_id,
                                extra_asset.object_type_name(info)
                            ));
                        }
                        if let Some(generated_icon) = generated_icon_import.as_ref() {
                            fusionforge::modding::apply_texture_import(
                                &mut value,
                                generated_icon.texture.clone(),
                            )?;
                        }
                    }
                    if let Some(name) =
                        object_name_overrides.get(&(*selected_asset_index, *path_id))
                    {
                        set_unity_object_string(&mut value, "m_Name", name);
                    }
                    if !npc_blueprint_uses_authoring_model_bundle(blueprint) {
                        normalize_imported_npc_output_object(
                            &extra_asset.object_type_name(info),
                            &mut value,
                        );
                        if extra_asset.object_type_name(info) == "Material" {
                            let shader_override =
                                material_shader_needs_default(&value, &valid_shader_pointers)
                                    .then(|| {
                                        shared_shader_ref
                                            .as_ref()
                                            .map(|(reference, file_id)| (reference, *file_id))
                                    })
                                    .flatten();
                            normalize_imported_material_output(
                                &mut value,
                                shader_override,
                                &local_output_path_ids,
                                &valid_external_file_ids,
                                npc_main_texture_pointer.clone(),
                            );
                        }
                    }
                    let extra_output_asset = npc_output_asset_with_target_type_trees(
                        extra_asset,
                        target_serialized_format,
                        target_type_trees,
                        std::iter::once(info),
                    );
                    if let Ok(tree) = extra_output_asset.object_type_tree(info) {
                        extra_type_trees
                            .entry(unity_object_type_tree_key(info))
                            .or_insert_with(|| tree.clone());
                    }
                    let mut output_info = info.clone();
                    output_info.path_id = target_output_path_id;
                    extra_objects.push((
                        output_info,
                        serialize_npc_output_object_value(
                            &extra_output_asset,
                            extra_asset_index,
                            info,
                            &value,
                        )?,
                    ));
                }
            }
        }
        let mut rebuild_asset =
            npc_asset_with_target_type_trees(asset, target_type_trees, keep_infos.iter().copied());
        if primary_asset_index == Some(asset_index) && !external_asset_refs.is_empty() {
            let mut asset_refs = vec![fusionforge::AssetRef {
                asset_path: String::new(),
                guid: [0; 16],
                type_id: 0,
                file_path: standalone_asset_name.clone(),
            }];
            asset_refs.extend(external_asset_refs.iter().cloned());
            rebuild_asset.asset_refs = asset_refs;
        }
        if primary_asset_index == Some(asset_index) && !extra_type_trees.is_empty() {
            let primary_type_keys = keep_ids
                .iter()
                .filter_map(|path_id| asset.objects.get(path_id))
                .map(unity_object_type_tree_key)
                .collect::<BTreeSet<_>>();
            for (type_key, tree) in extra_type_trees {
                if !primary_type_keys.contains(&type_key) {
                    rebuild_asset.tree.type_trees.insert(type_key, tree);
                }
            }
        }
        let data = if primary_asset_index == Some(asset_index) {
            rebuild_asset.rebuild_with_object_data_filtered_and_extra_remapped_as_format(
                Some(&keep_ids),
                &data_replacements,
                &extra_objects,
                &current_asset_output_path_ids,
                target_serialized_format,
            )?
        } else {
            rebuild_asset.rebuild_with_object_data_filtered_and_extra_as_format(
                Some(&keep_ids),
                &data_replacements,
                &[],
                target_serialized_format,
            )?
        };
        let output_asset_name = if primary_asset_index == Some(asset_index) {
            standalone_asset_name.as_str()
        } else {
            asset.name.as_str()
        };
        let target = output_temp.path().join(output_asset_name);
        fs::write(&target, data).map_err(|err| format!("{}: {err}", target.display()))?;
        copied_files.push(format!(
            "{} ({} objects{})",
            output_asset_name,
            keep_ids.len() + extra_objects.len(),
            if extra_objects.is_empty() {
                String::new()
            } else {
                format!(", merged {} external", extra_objects.len())
            }
        ));
    }

    if copied_files.is_empty() {
        return Err("NPC dependency pruning selected no asset files.".to_string());
    }
    copied_files.sort();
    copied_files.dedup();
    matched_paths.sort();
    matched_paths.dedup();
    let matched_container_paths = matched_paths
        .iter()
        .filter_map(|value| {
            value
                .split_once(": ")
                .map(|(_, path)| normalized_asset_path(path))
        })
        .collect::<BTreeSet<_>>();
    let mut missing_model_paths = Vec::new();
    for base in &hints.model_names {
        let kfm = format!("mob/{base}.kfm");
        let nif = format!("mob/{base}.nif");
        if matched_container_paths.contains(&kfm) || matched_container_paths.contains(&nif) {
            continue;
        }
        let exists_in_source_index = source_index_paths
            .as_ref()
            .is_some_and(|paths| paths.contains(&kfm) || paths.contains(&nif));
        if !exists_in_source_index {
            continue;
        }
        let label = format!("mob/{base}.kfm|.nif");
        if !matched_container_paths.contains(&kfm) && !matched_container_paths.contains(&nif) {
            missing_model_paths.push(label);
        }
    }
    if !missing_model_paths.is_empty() {
        return Err(format!(
            "TableData model paths exist in source index but were not copied for NPC '{}': {}",
            blueprint.name,
            missing_model_paths.join(", ")
        ));
    }
    pack_bundle_native_from_dir(output_temp.path(), bundle_path)?;
    warnings.push(format!(
        "Pruned NPC bundle: matched {} container paths and kept {} asset files.",
        matched_paths.len(),
        copied_files.len()
    ));
    let source_bundle_strings = source_bundles
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    Ok((source_bundle_strings, copied_files, warnings))
}
