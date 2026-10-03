use super::super::*;

pub(in super::super) fn pair_value_mut(value: &mut fusionforge::UnityValue) -> Option<&mut fusionforge::UnityValue> {
    match value {
        fusionforge::UnityValue::Pair(_, right) => Some(right.as_mut()),
        fusionforge::UnityValue::Array(items) if items.len() >= 2 => items.get_mut(1),
        _ => None,
    }
}

pub(in super::super) fn npc_target_external_ref_from_key(
    env: &fusionforge::UnityEnvironment,
    asset_index: usize,
    path_id: i64,
) -> Option<NpcTargetExternalRef> {
    let asset = env.assets.get(asset_index)?;
    let info = asset.objects.get(&path_id)?;
    let body = asset.read_object(asset_index, info).ok()?;
    Some(NpcTargetExternalRef {
        asset_name: asset.name.clone(),
        path_id,
        object_type: asset.object_type_name(info),
        object_name: fusionforge::object_name(&body),
    })
}

pub(in super::super) fn rename_pair_key(value: &mut fusionforge::UnityValue, key: &str) {
    match value {
        fusionforge::UnityValue::Pair(left, _) => {
            **left = fusionforge::UnityValue::String(key.to_string());
        }
        fusionforge::UnityValue::Array(items) if !items.is_empty() => {
            items[0] = fusionforge::UnityValue::String(key.to_string());
        }
        _ => {}
    }
}

pub(in super::super) fn replace_pair_value(value: &mut fusionforge::UnityValue, replacement: fusionforge::UnityValue) {
    match value {
        fusionforge::UnityValue::Pair(_, right) => {
            **right = replacement;
        }
        fusionforge::UnityValue::Array(items) if items.len() >= 2 => {
            items[1] = replacement;
        }
        _ => {}
    }
}

pub(in super::super) fn filtered_assetbundle_value(
    body: &fusionforge::UnityValue,
    kept_paths: &BTreeSet<String>,
    output_path_overrides: &BTreeMap<String, String>,
    hints: &NpcAssetHints,
    asset_pointer_overrides: &BTreeMap<String, fusionforge::UnityValue>,
) -> fusionforge::UnityValue {
    let old_preloads = fusionforge::value_array(body.get("m_PreloadTable")).to_vec();
    let mut new_preloads = Vec::new();
    let mut new_container = Vec::new();
    let mut main_asset = None::<(String, fusionforge::UnityValue)>;
    // Real container paths that exist in this source AssetBundle. An alias
    // must never shadow a real entry, otherwise e.g.
    // "texture/npc_princessbubblegum.dds" ends up pointing at the
    // "texture/npc_princess.dds" texture object.
    let real_paths = fusionforge::value_array(body.get("m_Container"))
        .iter()
        .filter_map(|entry| fusionforge::pair_name_value(entry))
        .map(|(path, _)| normalized_asset_path(path))
        .collect::<BTreeSet<_>>();

    for entry in fusionforge::value_array(body.get("m_Container")) {
        let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
            continue;
        };
        if !kept_paths.contains(path) {
            continue;
        }
        let entry = entry.clone();
        let output_path = output_path_overrides
            .get(&normalized_asset_path(path))
            .map(String::as_str)
            .unwrap_or(path);
        let preload_start = new_preloads.len();
        let (old_start, old_end) = metadata_preload_range(metadata, old_preloads.len());
        new_preloads.extend(old_preloads[old_start..old_end].iter().cloned());
        let preload_size = new_preloads.len().saturating_sub(preload_start);
        let mut output_paths = vec![output_path.to_string()];
        output_paths.extend(
            npc_container_alias_paths(output_path, hints)
                .into_iter()
                .filter(|alias| !real_paths.contains(&normalized_asset_path(alias))),
        );
        for alias_path in output_paths {
            let mut alias_entry = entry.clone();
            if alias_path != path {
                rename_pair_key(&mut alias_entry, &alias_path);
            }
            if let Some(metadata) = pair_value_mut(&mut alias_entry) {
                set_unity_object_i64(metadata, "preloadIndex", preload_start as i64);
                set_unity_object_i64(metadata, "preloadSize", preload_size as i64);
                if let Some(pointer) = asset_pointer_overrides.get(path) {
                    if let Some(object) = metadata.as_object_mut() {
                        object.insert("asset".to_string(), pointer.clone());
                    }
                }
                if main_asset.is_none() {
                    if let Some(asset_pointer) = metadata.get("asset").cloned() {
                        main_asset = Some((alias_path.clone(), asset_pointer));
                    }
                }
            }
            new_container.push(alias_entry);
        }
    }

    let mut filtered = body.clone();
    if let Some(object) = filtered.as_object_mut() {
        object.insert(
            "m_Container".to_string(),
            fusionforge::UnityValue::Array(new_container),
        );
        object.insert(
            "m_PreloadTable".to_string(),
            fusionforge::UnityValue::Array(new_preloads),
        );
        if let Some((path, asset_pointer)) = main_asset {
            if let Some(main_asset) = object
                .get_mut("m_MainAsset")
                .and_then(fusionforge::UnityValue::as_object_mut)
            {
                main_asset.insert("name".to_string(), fusionforge::UnityValue::String(path));
                main_asset.insert("asset".to_string(), asset_pointer);
            }
        }
    }
    filtered
}

pub(in super::super) fn filter_assetbundle_preloads_to_output_paths(
    value: &mut fusionforge::UnityValue,
    local_path_ids: &BTreeSet<i64>,
    external_pointers: &BTreeSet<(i32, i64)>,
) -> usize {
    let _ = external_pointers;
    let old_preloads = fusionforge::value_array(value.get("m_PreloadTable")).to_vec();
    if old_preloads.is_empty() {
        return 0;
    }
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let Some(container) = object
        .get_mut("m_Container")
        .and_then(fusionforge::UnityValue::as_array_mut)
    else {
        return 0;
    };

    let old_total = old_preloads.len();
    let mut new_preloads = Vec::<fusionforge::UnityValue>::new();
    for entry in container {
        let Some(metadata) = pair_value_mut(entry) else {
            continue;
        };
        let preload_start = new_preloads.len();
        let (old_start, old_end) = metadata_preload_range(metadata, old_preloads.len());
        new_preloads.extend(
            old_preloads[old_start..old_end]
                .iter()
                .filter(|preload| {
                    preload.as_pointer().is_some_and(|pointer| {
                        pointer.file_id == 0 && local_path_ids.contains(&pointer.path_id)
                    })
                })
                .cloned(),
        );
        if new_preloads.len() == preload_start {
            if let Some(asset_pointer) = metadata.get("asset").and_then(|value| value.as_pointer())
            {
                if asset_pointer.file_id == 0 && local_path_ids.contains(&asset_pointer.path_id) {
                    new_preloads.push(unity_local_pointer(0, asset_pointer.path_id));
                }
            }
        }
        let preload_size = new_preloads.len().saturating_sub(preload_start);
        set_unity_object_i64(metadata, "preloadIndex", preload_start as i64);
        set_unity_object_i64(metadata, "preloadSize", preload_size as i64);
    }
    let dropped = old_total.saturating_sub(new_preloads.len());
    object.insert(
        "m_PreloadTable".to_string(),
        fusionforge::UnityValue::Array(new_preloads),
    );
    dropped
}

pub(in super::super) fn normalize_clean_npc_assetbundle_preloads(
    value: &mut fusionforge::UnityValue,
    local_path_ids: &BTreeSet<i64>,
    external_pointers: &BTreeSet<(i32, i64)>,
) -> bool {
    let _ = external_pointers;
    let old_preloads = fusionforge::value_array(value.get("m_PreloadTable")).to_vec();
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let Some(container) = object
        .get_mut("m_Container")
        .and_then(fusionforge::UnityValue::as_array_mut)
    else {
        return false;
    };
    let has_mob = container.iter().any(|entry| {
        fusionforge::pair_name_value(entry)
            .map(|(path, _)| normalized_asset_path(path).starts_with("mob/"))
            .unwrap_or(false)
    });
    let texture_asset_ids = container
        .iter()
        .filter_map(|entry| {
            let (path, metadata) = fusionforge::pair_name_value(entry)?;
            normalized_asset_path(path)
                .starts_with("texture/")
                .then_some(metadata)?
                .get("asset")?
                .as_pointer()
                .filter(|pointer| pointer.file_id == 0)
                .map(|pointer| pointer.path_id)
        })
        .collect::<BTreeSet<_>>();
    if !has_mob || texture_asset_ids.is_empty() {
        return false;
    }

    let model_local_preloads = local_path_ids
        .iter()
        .copied()
        .filter(|path_id| *path_id != 1 && !texture_asset_ids.contains(path_id))
        .map(|path_id| unity_local_pointer(0, path_id))
        .collect::<Vec<_>>();
    let mut new_preloads = Vec::<fusionforge::UnityValue>::new();

    for entry in container {
        let Some((path, _)) = fusionforge::pair_name_value(entry) else {
            continue;
        };
        let normalized = normalized_asset_path(path);
        let preload_start = new_preloads.len();
        if normalized.starts_with("mob/") {
            new_preloads.extend(model_local_preloads.iter().cloned());
        } else if normalized.starts_with("texture/") {
            if let Some(asset_pointer) = pair_value_mut(entry)
                .and_then(|metadata| metadata.get("asset"))
                .and_then(fusionforge::UnityValue::as_pointer)
            {
                if asset_pointer.file_id == 0 {
                    new_preloads.push(unity_local_pointer(0, asset_pointer.path_id));
                }
            }
        } else {
            let old_preload_count = old_preloads.len();
            if let Some(metadata) = pair_value_mut(entry) {
                let (old_start, old_end) = metadata_preload_range(metadata, old_preload_count);
                new_preloads.extend(old_preloads[old_start..old_end].iter().filter_map(
                    |preload| {
                        let pointer = preload.as_pointer()?;
                        if pointer.file_id == 0 && local_path_ids.contains(&pointer.path_id) {
                            Some(unity_local_pointer(0, pointer.path_id))
                        } else {
                            None
                        }
                    },
                ));
            }
        }
        let preload_size = new_preloads.len().saturating_sub(preload_start);
        if let Some(metadata) = pair_value_mut(entry) {
            set_unity_object_i64(metadata, "preloadIndex", preload_start as i64);
            set_unity_object_i64(metadata, "preloadSize", preload_size as i64);
        }
    }

    object.insert(
        "m_PreloadTable".to_string(),
        fusionforge::UnityValue::Array(new_preloads),
    );
    true
}

pub(in super::super) fn merge_assetbundle_replacements_to_primary(
    assetbundle_replacements: &BTreeMap<(usize, i64), fusionforge::UnityValue>,
    env: &fusionforge::UnityEnvironment,
    primary_asset_index: usize,
    selected: &BTreeSet<(usize, i64)>,
    externalized: &BTreeMap<(usize, i64), NpcTargetExternalRef>,
    external_ref_indices: &BTreeMap<String, i32>,
    explicit_external_ref_indices: &BTreeMap<(String, [u8; 16], i32, String), i32>,
    output_path_ids: &BTreeMap<(usize, i64), i64>,
    hints: &NpcAssetHints,
) -> Option<((usize, i64), fusionforge::UnityValue)> {
    let primary_key = assetbundle_replacements
        .iter()
        .find_map(|(key, value)| {
            (key.0 == primary_asset_index && assetbundle_value_has_container_prefix(value, "mob/"))
                .then_some(*key)
        })
        .or_else(|| {
            assetbundle_replacements
                .keys()
                .find(|key| key.0 == primary_asset_index)
                .copied()
        })?;
    let mut ordered = assetbundle_replacements
        .iter()
        .map(|(key, value)| {
            let priority = if *key == primary_key {
                0
            } else if assetbundle_value_has_container_prefix(value, "mob/") {
                1
            } else if assetbundle_value_has_container_prefix(value, "texture/") {
                2
            } else {
                3
            };
            (priority, *key, value)
        })
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(priority, key, _)| (*priority, *key));

    let mut merged = assetbundle_replacements.get(&primary_key)?.clone();
    let mut merged_preloads = Vec::<fusionforge::UnityValue>::new();
    let mut merged_container = Vec::<fusionforge::UnityValue>::new();
    let mut seen_paths = BTreeSet::<String>::new();
    let mut main_asset = None::<(String, fusionforge::UnityValue)>;
    // Real container paths across all merged AssetBundles: an alias generated
    // from one bundle's entry must never shadow another bundle's real entry
    // (e.g. the spawn texture entry stealing "texture/fusion_rex.dds").
    let real_paths = ordered
        .iter()
        .flat_map(|(_, _, value)| fusionforge::value_array(value.get("m_Container")).iter())
        .filter_map(|entry| fusionforge::pair_name_value(entry))
        .map(|(path, _)| normalized_asset_path(path))
        .collect::<BTreeSet<_>>();

    for (_, _, value) in &ordered {
        let old_preloads = fusionforge::value_array(value.get("m_PreloadTable")).to_vec();
        for entry in fusionforge::value_array(value.get("m_Container")) {
            let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                continue;
            };
            if !seen_paths.insert(normalized_asset_path(path)) {
                continue;
            }
            let mut entry = entry.clone();
            let preload_start = merged_preloads.len();
            let (old_start, old_end) = metadata_preload_range(metadata, old_preloads.len());
            for preload in &old_preloads[old_start..old_end] {
                let mut preload = preload.clone();
                rewrite_npc_output_pointers(
                    &mut preload,
                    env,
                    primary_asset_index,
                    selected,
                    externalized,
                    external_ref_indices,
                    explicit_external_ref_indices,
                    output_path_ids,
                );
                merged_preloads.push(preload);
            }
            let preload_size = merged_preloads.len().saturating_sub(preload_start);
            rewrite_npc_output_pointers(
                &mut entry,
                env,
                primary_asset_index,
                selected,
                externalized,
                external_ref_indices,
                explicit_external_ref_indices,
                output_path_ids,
            );
            let mut output_paths = vec![path.to_string()];
            output_paths.extend(
                npc_container_alias_paths(path, hints)
                    .into_iter()
                    .filter(|alias| !real_paths.contains(&normalized_asset_path(alias))),
            );
            for alias_path in output_paths {
                if !seen_paths.insert(normalized_asset_path(&alias_path)) && alias_path != path {
                    continue;
                }
                let mut alias_entry = entry.clone();
                if alias_path != path {
                    rename_pair_key(&mut alias_entry, &alias_path);
                }
                if let Some(metadata) = pair_value_mut(&mut alias_entry) {
                    set_unity_object_i64(metadata, "preloadIndex", preload_start as i64);
                    set_unity_object_i64(metadata, "preloadSize", preload_size as i64);
                    if main_asset.is_none() {
                        if let Some(asset_pointer) = metadata.get("asset").cloned() {
                            main_asset = Some((alias_path.clone(), asset_pointer));
                        }
                    }
                }
                merged_container.push(alias_entry);
            }
        }
    }

    if let Some(object) = merged.as_object_mut() {
        object.insert(
            "m_Container".to_string(),
            fusionforge::UnityValue::Array(merged_container),
        );
        object.insert(
            "m_PreloadTable".to_string(),
            fusionforge::UnityValue::Array(merged_preloads),
        );
        if let Some((path, asset_pointer)) = main_asset {
            if let Some(main_asset) = object
                .get_mut("m_MainAsset")
                .and_then(fusionforge::UnityValue::as_object_mut)
            {
                main_asset.insert("name".to_string(), fusionforge::UnityValue::String(path));
                main_asset.insert("asset".to_string(), asset_pointer);
            }
        }
    }

    Some((primary_key, merged))
}

pub(in super::super) fn pointer_candidate_keys_for_rewrite(
    env: &fusionforge::UnityEnvironment,
    pointer: &fusionforge::Pointer,
) -> Vec<(usize, i64)> {
    if pointer_has_explicit_external_ref(env, pointer) {
        if let Ok(key) = env.resolve_pointer(pointer) {
            return vec![(key.asset, key.path_id)];
        }
        return Vec::new();
    }
    let mut keys = pointer_candidate_keys(env, pointer);
    let mut seen = keys.iter().copied().collect::<BTreeSet<_>>();
    for candidate in pointer_fallback_candidates(pointer) {
        for (asset_index, asset) in env.assets.iter().enumerate() {
            if asset.objects.contains_key(&candidate) && seen.insert((asset_index, candidate)) {
                keys.push((asset_index, candidate));
            }
        }
    }
    keys
}

pub(in super::super) fn rewrite_npc_output_pointers(
    value: &mut fusionforge::UnityValue,
    env: &fusionforge::UnityEnvironment,
    primary_asset_index: usize,
    selected: &BTreeSet<(usize, i64)>,
    externalized: &BTreeMap<(usize, i64), NpcTargetExternalRef>,
    external_ref_indices: &BTreeMap<String, i32>,
    explicit_external_ref_indices: &BTreeMap<(String, [u8; 16], i32, String), i32>,
    output_path_ids: &BTreeMap<(usize, i64), i64>,
) {
    match value {
        fusionforge::UnityValue::Array(items) => {
            for item in items {
                rewrite_npc_output_pointers(
                    item,
                    env,
                    primary_asset_index,
                    selected,
                    externalized,
                    external_ref_indices,
                    explicit_external_ref_indices,
                    output_path_ids,
                );
            }
        }
        fusionforge::UnityValue::Object(fields) => {
            for item in fields.values_mut() {
                rewrite_npc_output_pointers(
                    item,
                    env,
                    primary_asset_index,
                    selected,
                    externalized,
                    external_ref_indices,
                    explicit_external_ref_indices,
                    output_path_ids,
                );
            }
        }
        fusionforge::UnityValue::Pair(left, right) => {
            rewrite_npc_output_pointers(
                left,
                env,
                primary_asset_index,
                selected,
                externalized,
                external_ref_indices,
                explicit_external_ref_indices,
                output_path_ids,
            );
            rewrite_npc_output_pointers(
                right,
                env,
                primary_asset_index,
                selected,
                externalized,
                external_ref_indices,
                explicit_external_ref_indices,
                output_path_ids,
            );
        }
        fusionforge::UnityValue::Pointer(pointer) => {
            if pointer.is_null() {
                return;
            }
            if pointer_has_explicit_external_ref(env, pointer) {
                if let Some(source_ref) = env.assets.get(pointer.source_asset).and_then(|asset| {
                    usize::try_from(pointer.file_id)
                        .ok()
                        .and_then(|index| asset.asset_refs.get(index))
                }) {
                    if let Some(file_id) =
                        explicit_external_ref_indices.get(&explicit_asset_ref_key(source_ref))
                    {
                        pointer.source_asset = primary_asset_index;
                        pointer.file_id = *file_id;
                        return;
                    }
                }
            }
            let candidates = pointer_candidate_keys_for_rewrite(env, pointer);
            if let Some(external) = candidates
                .iter()
                .find_map(|key| externalized.get(key).map(|external| (*key, external)))
            {
                if let Some(file_id) = external_ref_indices.get(&external.1.asset_name) {
                    pointer.source_asset = primary_asset_index;
                    pointer.file_id = *file_id;
                    pointer.path_id = external.1.path_id;
                    return;
                }
            }
            if let Some((asset_index, path_id)) = candidates
                .into_iter()
                .find(|key| selected.contains(key) && !externalized.contains_key(key))
            {
                pointer.source_asset = primary_asset_index;
                pointer.file_id = 0;
                pointer.path_id = output_path_ids
                    .get(&(asset_index, path_id))
                    .copied()
                    .unwrap_or(path_id);
            }
        }
        _ => {}
    }
}

pub(in super::super) fn npc_referenced_external_output_pointers(
    env: &fusionforge::UnityEnvironment,
    selected: &BTreeSet<(usize, i64)>,
    externalized: &BTreeMap<(usize, i64), NpcTargetExternalRef>,
    external_ref_indices: &BTreeMap<String, i32>,
) -> Result<BTreeSet<(i32, i64)>, String> {
    let mut output_pointers = BTreeSet::new();
    for (asset_index, path_id) in selected {
        if externalized.contains_key(&(*asset_index, *path_id)) {
            continue;
        }
        let Some(asset) = env.assets.get(*asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(path_id) else {
            continue;
        };
        if asset.object_type_name(info) == "AssetBundle" {
            continue;
        }
        let body = asset.read_object(*asset_index, info)?;
        let mut pointers = Vec::new();
        collect_value_pointers(&body, &mut pointers);
        for pointer in pointers {
            if pointer.is_null() {
                continue;
            }
            if let Some(reference) = pointer_candidate_keys_for_rewrite(env, &pointer)
                .iter()
                .find_map(|key| externalized.get(key))
            {
                if let Some(file_id) = external_ref_indices.get(&reference.asset_name) {
                    output_pointers.insert((*file_id, reference.path_id));
                }
            }
        }
    }
    Ok(output_pointers)
}

pub(in super::super) fn standalone_npc_bundles_with_mob(out_dir: &Path, bundle_names: &[String]) -> Vec<String> {
    let mut filtered = Vec::new();
    for name in bundle_names {
        let bundle_path = out_dir.join(name);
        let Ok(temp) = native_build_temp_dir("npc_manifest_probe") else {
            continue;
        };
        if extract_bundle_native_to_dir(&bundle_path, temp.path()).is_err() {
            continue;
        }
        let has_mob = extracted_files_in_dir(temp.path()).into_iter().any(|file| {
            let path = PathBuf::from(&file.path);
            let Ok(asset) = fusionforge::Asset::from_path(&path) else {
                return false;
            };
            asset.objects.values().any(|info| {
                if asset.object_type_name(info) != "AssetBundle" {
                    return false;
                }
                asset
                    .read_object(0, info)
                    .map(|value| assetbundle_value_has_container_prefix(&value, "mob/"))
                    .unwrap_or(false)
            })
        });
        if has_mob {
            filtered.push(name.clone());
        }
    }
    filtered
}

pub(in super::super) fn pointer_fallback_candidates(pointer: &fusionforge::Pointer) -> Vec<i64> {
    let mut candidates = Vec::new();
    if pointer.path_id != 0 {
        candidates.push(pointer.path_id);
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

pub(in super::super) fn pointer_has_explicit_external_ref(
    env: &fusionforge::UnityEnvironment,
    pointer: &fusionforge::Pointer,
) -> bool {
    pointer.file_id != 0
        && env
            .assets
            .get(pointer.source_asset)
            .and_then(|asset| {
                usize::try_from(pointer.file_id)
                    .ok()
                    .and_then(|index| asset.asset_refs.get(index))
            })
            .is_some()
}

pub(in super::super) fn pointer_candidate_keys(
    env: &fusionforge::UnityEnvironment,
    pointer: &fusionforge::Pointer,
) -> Vec<(usize, i64)> {
    if let Ok(key) = env.resolve_pointer(pointer) {
        return vec![(key.asset, key.path_id)];
    }

    if pointer_has_explicit_external_ref(env, pointer) {
        return Vec::new();
    }

    let candidates = pointer_fallback_candidates(pointer);
    let mut keys = Vec::new();
    let mut seen = BTreeSet::<(usize, i64)>::new();
    if let Some(asset) = env.assets.get(pointer.source_asset) {
        for candidate in &candidates {
            if asset.objects.contains_key(candidate)
                && seen.insert((pointer.source_asset, *candidate))
            {
                keys.push((pointer.source_asset, *candidate));
            }
        }
    }

    for (asset_index, asset) in env.assets.iter().enumerate() {
        for candidate in &candidates {
            if asset.objects.contains_key(candidate) && seen.insert((asset_index, *candidate)) {
                keys.push((asset_index, *candidate));
            }
        }
    }
    keys
}

pub(in super::super) fn add_resolved_pointer_root(
    env: &fusionforge::UnityEnvironment,
    pointer: &fusionforge::Pointer,
    selected: &mut BTreeSet<(usize, i64)>,
    queue: &mut VecDeque<(usize, i64)>,
) {
    // Legacy format-7 assets can expose a bogus non-zero file_id while still
    // carrying a valid local path_id. UnityEnvironment::resolve_pointer (and
    // pointer_candidate_keys below) deliberately recover that case. Do not
    // apply the stricter preview-only external-reference guard here, otherwise
    // standalone NPC packing drops the complete local dependency graph.
    for key in pointer_candidate_keys(env, pointer) {
        if selected.insert(key) {
            queue.push_back(key);
        }
    }
}

pub(in super::super) fn rebind_imported_npc_blueprint_assets_from_sources(
    project: &Path,
    blueprint: &mut NpcBlueprint,
    source_paths: &[PathBuf],
    hints: &NpcAssetHints,
) -> Result<(), String> {
    if npc_blueprint_uses_authoring_model_bundle(blueprint) {
        return Ok(());
    }
    let resolved_paths = npc_resolved_container_paths_from_sources(project, source_paths)?;
    if resolved_paths.is_empty() {
        return Ok(());
    }

    if let Some(model) = first_matching_resolved_path(
        blueprint.model_asset.as_deref(),
        hints.model_paths.iter(),
        &resolved_paths,
    ) {
        blueprint.model_asset = Some(model);
    }
    if let Some(texture) = first_matching_resolved_path(
        blueprint.texture_asset.as_deref(),
        hints.texture_paths.iter(),
        &resolved_paths,
    ) {
        blueprint.texture_asset = Some(texture);
    }
    if blueprint.generated_icon.is_none() {
        if let Some(icon) = first_matching_resolved_path(
            blueprint.icon_asset.as_deref(),
            hints.icon_paths.iter(),
            &resolved_paths,
        ) {
            blueprint.icon_asset = Some(icon);
        }
    }
    Ok(())
}

pub(in super::super) fn npc_externalized_pointer_map(
    env: &fusionforge::UnityEnvironment,
    selected: &BTreeSet<(usize, i64)>,
    source_container_paths: &BTreeMap<(usize, i64), BTreeSet<String>>,
    required_container_paths: &BTreeSet<String>,
    target_index: &NpcTargetExternalIndex,
) -> BTreeMap<(usize, i64), NpcTargetExternalRef> {
    selected
        .iter()
        .filter_map(|key| {
            npc_external_ref_for_source_object(
                env,
                *key,
                source_container_paths,
                required_container_paths,
                target_index,
            )
            .map(|reference| (*key, reference))
        })
        .collect()
}
