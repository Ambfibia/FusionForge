use super::super::*;

pub(in super::super) fn npc_asset_hints_from_source(
    project: &Path,
    context: &NpcImportManifestContext,
) -> Result<NpcAssetHints, String> {
    let mut hints = NpcAssetHints::default();
    let Some(npc_id) = context.source_npc_id else {
        return Ok(hints);
    };
    let body = if let Some(patch_path) = context
        .table_data_patch_path
        .as_ref()
        .filter(|path| path.exists())
    {
        table_data_patch_value(patch_path, 0)?
    } else if let (Some(bundle), Some(asset_name), Some(path_id)) = (
        context.source_table_data_bundle.as_ref(),
        context.source_asset.as_ref(),
        context.source_path_id,
    ) {
        read_table_data_object_body(project, bundle, asset_name, path_id)?
    } else {
        return Ok(hints);
    };
    if let Some(npc_table) = npc_table_from_body(&body) {
        collect_npc_asset_hints_from_table(npc_table, npc_id, &mut hints);
    }
    Ok(hints)
}

pub(in super::super) fn extend_npc_asset_hints(target: &mut NpcAssetHints, source: NpcAssetHints) {
    target.tokens.extend(source.tokens);
    target.exact_paths.extend(source.exact_paths);
    target.model_paths.extend(source.model_paths);
    target.model_names.extend(source.model_names);
    target.texture_paths.extend(source.texture_paths);
    target.icon_paths.extend(source.icon_paths);
    target.audio_paths.extend(source.audio_paths);
}

pub(in super::super) fn normalized_asset_path(value: &str) -> String {
    value.replace('\\', "/").to_ascii_lowercase()
}

pub(in super::super) fn npc_imported_asset_unique_stem(npc_id: i64, source_path: &str) -> String {
    let base = asset_stem_from_container_path(source_path)
        .unwrap_or_else(|| "asset".to_string())
        .trim_start_matches("npc_")
        .trim_start_matches("mob_")
        .to_string();
    format!(
        "npc_{}_{}",
        npc_id.max(0),
        safe_segment(&base).to_ascii_lowercase()
    )
}

pub(in super::super) fn npc_imported_icon_target_path(npc_id: i64, source_path: &str) -> String {
    parse_npc_icon_asset_path(source_path)
        .map(|icon| format!("icons/{}_{npc_id:02}.png", icon.prefix))
        .unwrap_or_else(|| normalized_asset_path(source_path))
}

pub(in super::super) fn npc_hint_container_path_matches(
    path: &str,
    tokens: &BTreeSet<String>,
    hints: &NpcAssetHints,
) -> bool {
    let lower = normalized_asset_path(path);
    if hints
        .exact_paths
        .iter()
        .any(|exact| lower == *exact || lower.ends_with(&format!("/{exact}")))
    {
        return true;
    }
    if lower.starts_with("map/") || lower.starts_with("textures/") {
        return false;
    }
    tokens.iter().any(|token| lower.contains(token))
}

pub(in super::super) fn npc_standalone_container_path_matches(path: &str, required_paths: &BTreeSet<String>) -> bool {
    let lower = normalized_asset_path(path);
    required_paths
        .iter()
        .any(|required| lower == *required || lower.ends_with(&format!("/{required}")))
}

pub(in super::super) fn standalone_npc_asset_name(bundle_path: &Path, blueprint: &NpcBlueprint) -> String {
    bundle_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            npc_blueprint_bundle_name(blueprint)
                .trim_end_matches(".resourceFile")
                .trim_end_matches(".resourcefile")
                .to_string()
        })
}

#[derive(Debug, Clone, Default)]
pub(in super::super) struct NpcTargetExternalIndex {
    pub(in super::super) by_container_path: BTreeMap<String, NpcTargetExternalRef>,
    pub(in super::super) shader_by_fingerprint: BTreeMap<u64, NpcTargetExternalRef>,
    pub(in super::super) shader_by_name: BTreeMap<String, NpcTargetExternalRef>,
    pub(in super::super) texture_by_name: BTreeMap<String, NpcTargetExternalRef>,
}

#[derive(Debug, Clone, Default)]
pub(in super::super) struct NpcTargetTypeTreeIndex {
    pub(in super::super) by_key: BTreeMap<i32, fusionforge::TypeTree>,
}

impl NpcTargetTypeTreeIndex {
    pub(in super::super) fn tree_for_object(&self, info: &fusionforge::ObjectInfo) -> Option<&fusionforge::TypeTree> {
        let primary = unity_object_type_tree_key(info);
        self.by_key
            .get(&primary)
            .or_else(|| self.by_key.get(&info.class_id))
            .or_else(|| self.by_key.get(&info.type_id))
    }
}

pub(in super::super) fn build_npc_target_external_index(project: &Path) -> Result<NpcTargetExternalIndex, String> {
    let index_path = project.join("cache").join("bundle-index.json");
    if !index_path.is_file() {
        return Ok(NpcTargetExternalIndex::default());
    }
    let index = serde_json::from_str::<ClientFileIndex>(
        &fs::read_to_string(&index_path)
            .map_err(|err| format!("{}: {err}", index_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", index_path.display()))?;

    let mut external_bundles = index
        .bundles
        .iter()
        .filter(|bundle| npc_target_external_bundle_name(&bundle.name))
        .collect::<Vec<_>>();
    external_bundles.sort_by_key(|bundle| {
        (
            npc_target_external_bundle_priority(&bundle.name).unwrap_or(u8::MAX),
            bundle.name.to_ascii_lowercase(),
        )
    });

    let mut assets = Vec::<fusionforge::Asset>::new();
    for bundle in external_bundles {
        for file in &bundle.extracted_files {
            let path = PathBuf::from(&file.path);
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if name.ends_with(".json") {
                continue;
            }
            if let Ok(mut asset) = fusionforge::Asset::from_path(&path) {
                // IMPORTANT: the Unity 2.x web player resolves cross-bundle PPtr
                // externals by the *internal* serialized file name (for example
                // "customassetbundle-b4f543c102ded400fbc6f1da25d9679a"), not by the
                // .resourceFile name on disk. Original game bundles always store
                // these references lowercased, so keep the extracted serialized
                // file name (lowercased) as the external asset name.
                asset.name = asset.name.to_ascii_lowercase();
                assets.push(asset);
            }
        }
    }

    let env = fusionforge::UnityEnvironment::from_assets(assets);
    let mut index = NpcTargetExternalIndex::default();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            let object_type = asset.object_type_name(info);
            let Ok(body) = asset.read_object(asset_index, info) else {
                continue;
            };
            if object_type == "AssetBundle" {
                for entry in fusionforge::value_array(body.get("m_Container")) {
                    let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                        continue;
                    };
                    let Some(pointer) = metadata
                        .get("asset")
                        .and_then(fusionforge::UnityValue::as_pointer)
                    else {
                        continue;
                    };
                    let Ok(key) = env.resolve_pointer(pointer) else {
                        continue;
                    };
                    if let Some(reference) =
                        npc_target_external_ref_from_key(&env, key.asset, key.path_id)
                    {
                        index
                            .by_container_path
                            .entry(normalized_asset_path(path))
                            .or_insert(reference);
                    }
                }
                continue;
            }
            let Some(reference) = npc_target_external_ref_from_key(&env, asset_index, info.path_id)
            else {
                continue;
            };
            match object_type.as_str() {
                "Shader" => {
                    if let Some(bytes) = object_payload_bytes(&body) {
                        index
                            .shader_by_fingerprint
                            .entry(npc_payload_fingerprint(&bytes))
                            .or_insert_with(|| reference.clone());
                    }
                    if let Some(shader_name) = npc_shader_name_from_body(&body) {
                        index
                            .shader_by_name
                            .entry(shader_name)
                            .or_insert_with(|| reference.clone());
                    }
                }
                "Texture2D" => {
                    let name = reference.object_name.trim().to_ascii_lowercase();
                    if !name.is_empty() {
                        index.texture_by_name.entry(name).or_insert(reference);
                    }
                }
                _ => {}
            }
        }
    }
    Ok(index)
}

pub(in super::super) fn build_npc_target_type_tree_index(
    project: &Path,
    target_serialized_format: Option<u32>,
) -> Result<NpcTargetTypeTreeIndex, String> {
    let index_path = project.join("cache").join("bundle-index.json");
    if !index_path.is_file() {
        return Ok(NpcTargetTypeTreeIndex::default());
    }
    let index = serde_json::from_str::<ClientFileIndex>(
        &fs::read_to_string(&index_path)
            .map_err(|err| format!("{}: {err}", index_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", index_path.display()))?;

    let mut target = NpcTargetTypeTreeIndex::default();
    for file in index
        .bundles
        .iter()
        .flat_map(|bundle| bundle.extracted_files.iter())
    {
        let path = PathBuf::from(&file.path);
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if name.ends_with(".json") {
            continue;
        }
        let Ok(asset) = fusionforge::Asset::from_path(&path) else {
            continue;
        };
        if target_serialized_format.is_some_and(|format| asset.format != format) {
            continue;
        }
        for (key, tree) in asset.tree.type_trees {
            target.by_key.entry(key).or_insert(tree);
        }
    }
    Ok(target)
}

pub(in super::super) fn npc_standalone_output_path_overrides(
    source_blueprint: &NpcBlueprint,
    blueprint: &NpcBlueprint,
    hints: &NpcAssetHints,
) -> BTreeMap<String, String> {
    let mut overrides = BTreeMap::new();
    if let (Some(source_model), Some(target_model)) = (
        source_blueprint.model_asset.as_deref(),
        blueprint.model_asset.as_deref(),
    ) {
        let source_model = normalized_asset_path(source_model);
        let target_model = normalized_asset_path(target_model);
        if !source_model.is_empty() && !target_model.is_empty() && source_model != target_model {
            overrides.insert(source_model.clone(), target_model.clone());
            if let Some(source_nif) = npc_model_sibling_path(&source_model, "nif") {
                if let Some(target_nif) = npc_model_sibling_path(&target_model, "nif") {
                    if source_nif != target_nif {
                        overrides.insert(source_nif, target_nif);
                    }
                }
            }
            if let Some(source_kfm) = npc_model_sibling_path(&source_model, "kfm") {
                if let Some(target_kfm) = npc_model_sibling_path(&target_model, "kfm") {
                    if source_kfm != target_kfm {
                        overrides.insert(source_kfm, target_kfm);
                    }
                }
            }
        }
    }
    if let (Some(source_texture), Some(target_texture)) = (
        source_blueprint.texture_asset.as_deref(),
        blueprint.texture_asset.as_deref(),
    ) {
        let source_texture = normalized_asset_path(source_texture);
        let target_texture = normalized_asset_path(target_texture);
        if !source_texture.is_empty()
            && !target_texture.is_empty()
            && source_texture != target_texture
        {
            overrides.insert(source_texture, target_texture);
        }
    }
    if let Some(icon) = blueprint.generated_icon.as_ref() {
        let template_path = normalized_asset_path(&icon.template_asset_path);
        let target_path = normalized_asset_path(&icon.asset_path);
        if !template_path.is_empty()
            && !target_path.is_empty()
            && !template_path.eq_ignore_ascii_case("null")
            && !target_path.eq_ignore_ascii_case("null")
            && template_path != target_path
        {
            overrides.insert(template_path, target_path);
        }
    }
    if blueprint.generated_icon.is_some() {
        return overrides;
    }
    let source_icon = source_blueprint
        .icon_asset
        .as_deref()
        .map(normalized_asset_path)
        .filter(|path| !path.is_empty())
        .or_else(|| preferred_npc_icon_asset_path(hints));
    let target_icon_path = retargeted_npc_icon_asset_path(blueprint, hints).or_else(|| {
        blueprint
            .icon_asset
            .as_deref()
            .map(normalized_asset_path)
            .filter(|path| !path.is_empty())
    });
    if let (Some(source_icon), Some(target_icon_path)) = (source_icon, target_icon_path) {
        if target_icon_path != source_icon {
            overrides.insert(source_icon, target_icon_path);
        }
    }
    overrides
}

pub(in super::super) fn npc_referenced_external_asset_names(
    env: &fusionforge::UnityEnvironment,
    selected: &BTreeSet<(usize, i64)>,
    externalized: &BTreeMap<(usize, i64), NpcTargetExternalRef>,
) -> Result<BTreeSet<String>, String> {
    let mut names = BTreeSet::new();
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
                names.insert(reference.asset_name.clone());
            }
        }
    }
    Ok(names)
}

pub(in super::super) fn npc_compact_output_path_ids(
    env: &fusionforge::UnityEnvironment,
    selected: &BTreeSet<(usize, i64)>,
    externalized: &BTreeMap<(usize, i64), NpcTargetExternalRef>,
    primary_asset_index: Option<usize>,
) -> BTreeMap<(usize, i64), i64> {
    let mut output_objects = Vec::<(u8, usize, i64)>::new();
    for key in selected {
        if externalized.contains_key(key) {
            continue;
        }
        let Some(asset) = env.assets.get(key.0) else {
            continue;
        };
        let Some(info) = asset.objects.get(&key.1) else {
            continue;
        };
        let object_type = asset.object_type_name(info);
        if let Some(primary) = primary_asset_index {
            if key.0 != primary && object_type == "AssetBundle" {
                continue;
            }
        }
        let priority = if object_type == "AssetBundle" && primary_asset_index == Some(key.0) {
            0
        } else if object_type == "AssetBundle" {
            1
        } else {
            2
        };
        output_objects.push((priority, key.0, key.1));
    }
    output_objects
        .sort_by_key(|(priority, asset_index, path_id)| (*priority, *asset_index, *path_id));

    let mut next_path_id = 1_i64;
    let mut output_path_ids = BTreeMap::new();
    for (_, asset_index, path_id) in output_objects {
        output_path_ids.insert((asset_index, path_id), next_path_id);
        next_path_id = next_path_id.saturating_add(1);
    }
    output_path_ids
}

pub(in super::super) fn npc_asset_with_target_type_trees<'a>(
    asset: &fusionforge::Asset,
    target_type_trees: &NpcTargetTypeTreeIndex,
    infos: impl IntoIterator<Item = &'a fusionforge::ObjectInfo>,
) -> fusionforge::Asset {
    let mut output_asset = asset.clone();
    for info in infos {
        let Some(tree) = target_type_trees.tree_for_object(info) else {
            continue;
        };
        output_asset
            .tree
            .type_trees
            .insert(unity_object_type_tree_key(info), tree.clone());
    }
    output_asset
}

pub(in super::super) fn npc_output_asset_with_target_type_trees<'a>(
    asset: &fusionforge::Asset,
    target_serialized_format: Option<u32>,
    target_type_trees: &NpcTargetTypeTreeIndex,
    infos: impl IntoIterator<Item = &'a fusionforge::ObjectInfo>,
) -> fusionforge::Asset {
    let typed_asset = npc_asset_with_target_type_trees(asset, target_type_trees, infos);
    target_serialized_format
        .map(|format| typed_asset.with_serialized_format(format))
        .unwrap_or(typed_asset)
}

pub(in super::super) fn caching_manifest_entry(file_name: &str, version: i64) -> fusionforge::UnityValue {
    let mut object = BTreeMap::new();
    object.insert(
        "fileName".to_string(),
        fusionforge::UnityValue::String(file_name.to_string()),
    );
    object.insert("version".to_string(), fusionforge::UnityValue::Int(version));
    fusionforge::UnityValue::Object(object)
}

pub(in super::super) fn stable_manifest_version(name: &str, bytes: &[u8]) -> i64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in name
        .as_bytes()
        .iter()
        .copied()
        .chain(std::iter::once(0))
        .chain(bytes.iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    1_000_000 + i64::try_from(hash % 1_000_000_000).unwrap_or(0)
}

pub(in super::super) fn caching_manifest_bundle_versions(
    out_dir: &Path,
    bundle_names: &[String],
) -> BTreeMap<String, i64> {
    let mut versions = BTreeMap::new();
    for name in bundle_names {
        let path = out_dir.join(name);
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        versions.insert(
            name.to_ascii_lowercase(),
            stable_manifest_version(name, &bytes),
        );
    }
    versions
}

pub(in super::super) fn patch_caching_manifest_npc_bundles(
    extracted_main_dir: &Path,
    bundle_names: &[String],
    bundle_versions: &BTreeMap<String, i64>,
    sections: &[&str],
) -> Result<usize, String> {
    let requested = bundle_names
        .iter()
        .filter_map(|name| {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return None;
            }
            let lower = trimmed.to_ascii_lowercase();
            (lower.ends_with(".resourcefile") || lower.ends_with(".unity3d"))
                .then(|| (lower, trimmed.to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    if requested.is_empty() {
        return Ok(0);
    }

    let mut added = 0usize;
    for file in extracted_files_in_dir(extracted_main_dir) {
        let asset_path = PathBuf::from(&file.path);
        let Ok(asset) = fusionforge::Asset::from_path(&asset_path) else {
            continue;
        };
        let mut replacements = BTreeMap::<i64, Vec<u8>>::new();
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "MonoBehaviour" {
                continue;
            }
            let mut value = asset.read_object(0, info)?;
            if fusionforge::object_name(&value) != "CachingManifest" {
                continue;
            }
            let mut max_version = 800000i64;
            let mut requested_versions = BTreeMap::<String, i64>::new();
            if let fusionforge::UnityValue::Object(fields) = &value {
                for entries in fields
                    .values()
                    .filter_map(fusionforge::UnityValue::as_array)
                {
                    for entry in entries {
                        let version = entry
                            .get("version")
                            .and_then(fusionforge::UnityValue::as_i64)
                            .unwrap_or_default();
                        max_version = max_version.max(version);
                        let Some(name) = entry
                            .get("fileName")
                            .and_then(fusionforge::UnityValue::as_str)
                        else {
                            continue;
                        };
                        let lower = name.to_ascii_lowercase();
                        if requested.contains_key(&lower) {
                            requested_versions.entry(lower.clone()).or_insert_with(|| {
                                bundle_versions.get(&lower).copied().unwrap_or(version)
                            });
                        }
                    }
                }
            }
            let mut next_version = max_version.saturating_add(1);
            for lower in requested.keys() {
                requested_versions.entry(lower.clone()).or_insert_with(|| {
                    let version = bundle_versions.get(lower).copied().unwrap_or(next_version);
                    next_version = next_version.saturating_add(1);
                    version
                });
            }
            for section in sections {
                let Some(entries) = value
                    .get_mut(section)
                    .and_then(fusionforge::UnityValue::as_array_mut)
                else {
                    continue;
                };
                let mut existing = BTreeSet::new();
                for entry in entries.iter_mut() {
                    let Some(name) = entry
                        .get("fileName")
                        .and_then(fusionforge::UnityValue::as_str)
                    else {
                        continue;
                    };
                    let lower = name.to_ascii_lowercase();
                    if !requested.contains_key(&lower) {
                        continue;
                    }
                    existing.insert(lower.clone());
                    if let Some(version) = requested_versions.get(&lower).copied() {
                        if entry
                            .get("version")
                            .and_then(fusionforge::UnityValue::as_i64)
                            != Some(version)
                        {
                            set_unity_object_i64(entry, "version", version);
                            added += 1;
                        }
                    }
                }
                for (lower, name) in &requested {
                    if existing.contains(lower) {
                        continue;
                    }
                    let version = requested_versions.get(lower).copied().unwrap_or(800000);
                    entries.push(caching_manifest_entry(name, version));
                    added += 1;
                }
            }
            replacements.insert(info.path_id, asset.serialize_object_value(0, info, &value)?);
        }
        if !replacements.is_empty() {
            let rebuilt = asset.rebuild_with_object_data(&replacements)?;
            fs::write(&asset_path, rebuilt)
                .map_err(|err| format!("{}: {err}", asset_path.display()))?;
        }
    }
    Ok(added)
}

/// Replaces the legacy resource layout in CachingManifest instead of merely
/// appending names. Runtime LoadWWW retries missing manifest entries, so a
/// retired filename must not survive in any of the seven loading stages. The
/// supplied version map also covers rewritten retained consumers (Map_*, main,
/// TableData, and music): keeping their old cache version would make Unity load
/// a stale pre-layout bundle even when the launcher manifest is current.
pub(in super::super) fn patch_caching_manifest_layout(
    extracted_main_dir: &Path,
    registrations: &[(String, Vec<String>)],
    retired_bundle_names: &[String],
    bundle_versions: &BTreeMap<String, i64>,
) -> Result<usize, String> {
    let retired = retired_bundle_names
        .iter()
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .collect::<BTreeSet<_>>();
    let requested = registrations
        .iter()
        .map(|(name, sections)| {
            (
                name.trim().to_string(),
                sections
                    .iter()
                    .map(|section| section.trim().to_string())
                    .filter(|section| !section.is_empty())
                    .collect::<Vec<_>>(),
            )
        })
        .filter(|(name, sections)| !name.is_empty() && !sections.is_empty())
        .collect::<Vec<_>>();
    let requested_names = requested
        .iter()
        .map(|(name, _)| name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    if retired.is_empty() && requested.is_empty() {
        return Ok(0);
    }

    let mut changed = 0usize;
    for file in extracted_files_in_dir(extracted_main_dir) {
        let asset_path = PathBuf::from(&file.path);
        let Ok(asset) = fusionforge::Asset::from_path(&asset_path) else {
            continue;
        };
        let mut replacements = BTreeMap::<i64, Vec<u8>>::new();
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "MonoBehaviour" {
                continue;
            }
            let mut value = asset.read_object(0, info)?;
            if fusionforge::object_name(&value) != "CachingManifest" {
                continue;
            }
            let Some(fields) = value.as_object_mut() else {
                continue;
            };
            for entries in fields
                .values_mut()
                .filter_map(fusionforge::UnityValue::as_array_mut)
            {
                for entry in entries.iter_mut() {
                    let Some(name) = entry
                        .get("fileName")
                        .and_then(fusionforge::UnityValue::as_str)
                    else {
                        continue;
                    };
                    let lower = name.to_ascii_lowercase();
                    if retired.contains(&lower) || requested_names.contains(&lower) {
                        continue;
                    }
                    let Some(version) = bundle_versions.get(&lower).copied() else {
                        continue;
                    };
                    if entry
                        .get("version")
                        .and_then(fusionforge::UnityValue::as_i64)
                        != Some(version)
                    {
                        set_unity_object_i64(entry, "version", version);
                        changed += 1;
                    }
                }
                let before = entries.len();
                entries.retain(|entry| {
                    let Some(name) = entry
                        .get("fileName")
                        .and_then(fusionforge::UnityValue::as_str)
                    else {
                        return true;
                    };
                    let lower = name.to_ascii_lowercase();
                    !retired.contains(&lower) && !requested_names.contains(&lower)
                });
                changed += before.saturating_sub(entries.len());
            }
            for (name, sections) in &requested {
                let lower = name.to_ascii_lowercase();
                let version = bundle_versions.get(&lower).copied().ok_or_else(|| {
                    format!("CachingManifest layout bundle has no stable version: {name}")
                })?;
                for section in sections {
                    let entries = fields
                        .get_mut(section)
                        .and_then(fusionforge::UnityValue::as_array_mut)
                        .ok_or_else(|| {
                            format!("CachingManifest section was not found: {section}")
                        })?;
                    entries.push(caching_manifest_entry(name, version));
                    changed += 1;
                }
            }
            replacements.insert(info.path_id, asset.serialize_object_value(0, info, &value)?);
        }
        if !replacements.is_empty() {
            let rebuilt = asset.rebuild_with_object_data(&replacements)?;
            fs::write(&asset_path, rebuilt)
                .map_err(|err| format!("{}: {err}", asset_path.display()))?;
        }
    }
    Ok(changed)
}
