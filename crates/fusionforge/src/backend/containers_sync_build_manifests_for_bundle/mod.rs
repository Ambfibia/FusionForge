use super::super::*;

pub(in super::super) fn remove_assetbundle_container_prefix(value: &mut fusionforge::UnityValue, prefix: &str) -> usize {
    let Some(container) = value
        .as_object_mut()
        .and_then(|object| object.get_mut("m_Container"))
        .and_then(fusionforge::UnityValue::as_array_mut)
    else {
        return 0;
    };
    let before = container.len();
    container.retain(|entry| {
        fusionforge::pair_name_value(entry)
            .map(|(path, _)| !path.starts_with(prefix))
            .unwrap_or(true)
    });
    before.saturating_sub(container.len())
}

pub(in super::super) fn unity_value_points_to_key(
    env: &fusionforge::UnityEnvironment,
    value: &fusionforge::UnityValue,
    key: (usize, i64),
) -> bool {
    value
        .as_pointer()
        .is_some_and(|pointer| pointer_candidate_keys(env, pointer).contains(&key))
}

pub(in super::super) fn unity_color(r: f64, g: f64, b: f64, a: f64) -> fusionforge::UnityValue {
    fusionforge::UnityValue::Object(BTreeMap::from([
        ("r".to_string(), fusionforge::UnityValue::Float(r)),
        ("g".to_string(), fusionforge::UnityValue::Float(g)),
        ("b".to_string(), fusionforge::UnityValue::Float(b)),
        ("a".to_string(), fusionforge::UnityValue::Float(a)),
    ]))
}

pub(in super::super) fn clean_object_info(path_id: i64, class_id: i32) -> fusionforge::ObjectInfo {
    fusionforge::ObjectInfo {
        path_id,
        data_offset: 0,
        size: 0,
        type_id: class_id,
        class_id,
    }
}

pub(in super::super) fn unity_value_shape(value: &fusionforge::UnityValue) -> String {
    match value {
        fusionforge::UnityValue::Object(object) => {
            let keys = object.keys().take(8).cloned().collect::<Vec<_>>().join(",");
            format!("object[{}]{{{keys}}}", object.len())
        }
        fusionforge::UnityValue::Array(items) => format!("array[{}]", items.len()),
        fusionforge::UnityValue::Pair(_, _) => "pair".to_string(),
        fusionforge::UnityValue::Pointer(pointer) => {
            format!("ptr(fileId={},pathId={})", pointer.file_id, pointer.path_id)
        }
        fusionforge::UnityValue::String(value) => format!("string[{}]", value.len()),
        fusionforge::UnityValue::Bytes(value) => format!("bytes[{}]", value.len()),
        fusionforge::UnityValue::Bool(_) => "bool".to_string(),
        fusionforge::UnityValue::Int(_) => "int".to_string(),
        fusionforge::UnityValue::UInt(_) => "uint".to_string(),
        fusionforge::UnityValue::Float(_) => "float".to_string(),
    }
}

pub(in super::super) fn unity_vec2(x: f64, y: f64) -> fusionforge::UnityValue {
    fusionforge::UnityValue::Object(BTreeMap::from([
        ("x".to_string(), fusionforge::UnityValue::Float(x)),
        ("y".to_string(), fusionforge::UnityValue::Float(y)),
    ]))
}

pub(in super::super) fn unity_vec3(x: f64, y: f64, z: f64) -> fusionforge::UnityValue {
    fusionforge::UnityValue::Object(BTreeMap::from([
        ("x".to_string(), fusionforge::UnityValue::Float(x)),
        ("y".to_string(), fusionforge::UnityValue::Float(y)),
        ("z".to_string(), fusionforge::UnityValue::Float(z)),
    ]))
}

pub(in super::super) fn unity_quat(x: f64, y: f64, z: f64, w: f64) -> fusionforge::UnityValue {
    fusionforge::UnityValue::Object(BTreeMap::from([
        ("x".to_string(), fusionforge::UnityValue::Float(x)),
        ("y".to_string(), fusionforge::UnityValue::Float(y)),
        ("z".to_string(), fusionforge::UnityValue::Float(z)),
        ("w".to_string(), fusionforge::UnityValue::Float(w)),
    ]))
}

pub(in super::super) fn unity_pair(
    left: fusionforge::UnityValue,
    right: fusionforge::UnityValue,
) -> fusionforge::UnityValue {
    fusionforge::UnityValue::Pair(Box::new(left), Box::new(right))
}

pub(in super::super) fn unity_named(name: &str) -> fusionforge::UnityValue {
    fusionforge::UnityValue::Object(BTreeMap::from([(
        "name".to_string(),
        fusionforge::UnityValue::String(name.to_string()),
    )]))
}

pub(in super::super) fn set_object_field(
    value: &mut fusionforge::UnityValue,
    key: &str,
    field: fusionforge::UnityValue,
) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), field);
    }
}

pub(in super::super) fn unity_component_ref(path_id: i64) -> fusionforge::UnityValue {
    fusionforge::UnityValue::Object(BTreeMap::from([(
        "component".to_string(),
        unity_local_pointer(0, path_id),
    )]))
}

pub(in super::super) fn clean_game_object_with_active(
    asset: &fusionforge::Asset,
    name: &str,
    components: &[(i64, i64)],
    active: bool,
) -> Result<fusionforge::UnityValue, String> {
    let mut value = asset.empty_object_value_for_class(1)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "m_Name".to_string(),
            fusionforge::UnityValue::String(name.to_string()),
        );
        object.insert(
            "m_IsActive".to_string(),
            fusionforge::UnityValue::Int(i64::from(active)),
        );
        object.insert("m_Layer".to_string(), fusionforge::UnityValue::Int(16));
        object.insert("m_Tag".to_string(), fusionforge::UnityValue::Int(0));
        object.insert(
            "m_Component".to_string(),
            fusionforge::UnityValue::Array(
                components
                    .iter()
                    .map(|(_class_id, path_id)| unity_component_ref(*path_id))
                    .collect(),
            ),
        );
    }
    Ok(value)
}

pub(in super::super) fn first_object_value_for_class(
    asset: &fusionforge::Asset,
    class_id: i32,
) -> Result<Option<fusionforge::UnityValue>, String> {
    let Some(info) = asset
        .objects
        .values()
        .find(|info| info.class_id == class_id)
    else {
        return Ok(None);
    };
    asset.read_object(0, info).map(Some)
}

pub(in super::super) fn maybe_copy_object_field(
    target: &mut fusionforge::UnityValue,
    source: &fusionforge::UnityValue,
    field: &str,
) {
    let replacement = source
        .as_object()
        .and_then(|object| object.get(field))
        .cloned();
    if let (Some(target_object), Some(replacement)) = (target.as_object_mut(), replacement) {
        target_object.insert(field.to_string(), replacement);
    }
}

pub(in super::super) fn bundle_internal_serialized_name(bundle_path: &Path) -> Option<String> {
    let (_, bundle) =
        ffbuildtool::bundle::AssetBundle::from_file(&bundle_path.to_string_lossy()).ok()?;
    let info = bundle.get_uncompressed_info(0).ok()?;
    info.keys()
        .filter(|name| !name.to_ascii_lowercase().ends_with(".json"))
        .min()
        .cloned()
}

pub(in super::super) fn is_bundle_file_external_ref_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".resourcefile") || lower.ends_with(".unity3d")
}

/// Rewrites external asset references that still point at bundle *file* names
/// (e.g. "Tutorial.resourceFile") so they use the lowercase *internal*
/// serialized file name of that bundle (e.g.
/// "customassetbundle-b4f543c102ded400fbc6f1da25d9679a"), which is the only
/// form the Unity 2.x web player can resolve at runtime. The first entry is
/// the self reference and is left untouched.
pub(in super::super) fn remap_bundle_file_external_refs(
    asset_refs: &mut [fusionforge::AssetRef],
    bundle_search_dir: Option<&Path>,
) -> Result<(), String> {
    let mut search_dirs = Vec::<PathBuf>::new();
    if let Some(dir) = bundle_search_dir {
        search_dirs.push(dir.to_path_buf());
    }
    let builds_root = default_repo_root().join("builds");
    if let Ok(entries) = fs::read_dir(&builds_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                search_dirs.push(path);
            }
        }
    }
    for asset_ref in asset_refs.iter_mut().skip(1) {
        if !is_bundle_file_external_ref_name(&asset_ref.file_path) {
            continue;
        }
        let file_name = asset_ref.file_path.clone();
        let Some(bundle_path) = search_dirs
            .iter()
            .map(|dir| dir.join(&file_name))
            .find(|candidate| candidate.is_file())
        else {
            return Err(format!(
                "External reference '{file_name}' could not be remapped to an internal serialized name: bundle file was not found near the layout or under {}",
                builds_root.display()
            ));
        };
        let Some(internal_name) = bundle_internal_serialized_name(&bundle_path) else {
            return Err(format!(
                "External reference '{file_name}' could not be remapped: {} did not expose an internal serialized file name",
                bundle_path.display()
            ));
        };
        asset_ref.file_path = internal_name.to_ascii_lowercase();
    }
    Ok(())
}

pub(in super::super) fn sync_build_manifests_for_bundle(bundle_path: &Path) -> Result<(), String> {
    let Some(build_dir) = bundle_path.parent() else {
        return Ok(());
    };
    let Some(bundle_name) = bundle_path.file_name().and_then(|value| value.to_str()) else {
        return Ok(());
    };
    let manifest_paths = build_manifest_paths(build_dir);
    if manifest_paths.is_empty() {
        return Ok(());
    }

    let compressed_hash = ffbuildtool::util::get_file_hash(&bundle_path.to_string_lossy())
        .map_err(|err| format!("{}: {err}", bundle_path.display()))?;
    let compressed_size = fs::metadata(bundle_path)
        .map_err(|err| format!("{}: {err}", bundle_path.display()))?
        .len();
    let (_, bundle) = ffbuildtool::bundle::AssetBundle::from_file(&bundle_path.to_string_lossy())?;
    let uncompressed_info = bundle
        .get_uncompressed_info(0)
        .map_err(|err| format!("{}: {err}", bundle_path.display()))?;

    for manifest_path in manifest_paths {
        let mut manifest = serde_json::from_str::<JsonValue>(
            &fs::read_to_string(&manifest_path)
                .map_err(|err| format!("{}: {err}", manifest_path.display()))?,
        )
        .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
        let Some(bundles) = manifest
            .get_mut("bundles")
            .and_then(JsonValue::as_object_mut)
        else {
            continue;
        };
        if !bundles.contains_key(bundle_name) {
            continue;
        }
        bundles.insert(
            bundle_name.to_string(),
            json!({
                "compressed_info": {
                    "hash": compressed_hash,
                    "size": compressed_size,
                },
                "uncompressed_info": uncompressed_info,
            }),
        );
        let total_compressed_size = bundles
            .values()
            .filter_map(|bundle| {
                bundle
                    .get("compressed_info")
                    .and_then(|info| info.get("size"))
                    .and_then(JsonValue::as_u64)
            })
            .sum::<u64>();
        let total_uncompressed_size = bundles
            .values()
            .flat_map(|bundle| {
                bundle
                    .get("uncompressed_info")
                    .and_then(JsonValue::as_object)
                    .into_iter()
                    .flat_map(|info| info.values())
            })
            .filter_map(|info| info.get("size").and_then(JsonValue::as_u64))
            .sum::<u64>();
        manifest["total_compressed_size"] = json!(total_compressed_size);
        manifest["total_uncompressed_size"] = json!(total_uncompressed_size);
        let data = serde_json::to_string_pretty(&manifest)
            .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
        fs::write(&manifest_path, format!("{data}\n"))
            .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    }
    Ok(())
}

pub(in super::super) fn unity_local_pointer(asset_index: usize, path_id: i64) -> fusionforge::UnityValue {
    fusionforge::UnityValue::Pointer(fusionforge::Pointer {
        source_asset: asset_index,
        file_id: 0,
        path_id,
    })
}

pub(in super::super) fn unity_external_pointer(file_id: i32, path_id: i64) -> fusionforge::UnityValue {
    fusionforge::UnityValue::Pointer(fusionforge::Pointer {
        source_asset: 0,
        file_id,
        path_id,
    })
}

pub(in super::super) fn unity_null_pointer() -> fusionforge::UnityValue {
    unity_external_pointer(0, 0)
}

pub(in super::super) fn find_standalone_npc_schema_bundle(
    project: &Path,
    prefer_skinned: bool,
) -> Result<(PathBuf, BTreeSet<String>), String> {
    let mut candidates = Vec::new();
    if prefer_skinned {
        candidates.extend(standalone_npc_schema_candidates(
            project,
            "Character_Johnny_Test.resourceFile",
            "mob/npc_johnnytest.kfm",
        ));
    }
    candidates.extend(standalone_npc_schema_candidates(
        project,
        "Character_Stanley.resourceFile",
        "mob/npc_stanley.kfm",
    ));

    for (candidate, model_paths) in candidates {
        if candidate.is_file() {
            return Ok((candidate, model_paths));
        }
    }
    Err("Could not find a standalone NPC schema bundle.".to_string())
}

pub(in super::super) fn add_unique_npc_source_bundle(
    bundles: &mut Vec<PathBuf>,
    seen: &mut BTreeSet<String>,
    path: PathBuf,
) {
    let key = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    if seen.insert(key) {
        bundles.push(path);
    }
}

pub(in super::super) fn is_npc_shared_dependency_bundle(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "charactercreation.resourcefile"
            | "characterselection.resourcefile"
            | "traininggrounds.resourcefile"
    )
}

pub(in super::super) fn client_project_serialized_format(project: &Path) -> Option<u32> {
    let index_path = project.join("cache").join("bundle-index.json");
    let index =
        serde_json::from_str::<ClientFileIndex>(&fs::read_to_string(index_path).ok()?).ok()?;
    for preferred in ["TableData.resourceFile", "CharacterCreation.resourceFile"] {
        if let Some(format) = index
            .bundles
            .iter()
            .filter(|bundle| bundle.name.eq_ignore_ascii_case(preferred))
            .flat_map(|bundle| bundle.extracted_files.iter())
            .filter_map(|file| fusionforge::Asset::from_path(Path::new(&file.path)).ok())
            .map(|asset| asset.format)
            .find(|format| *format > 0)
        {
            return Some(format);
        }
    }
    index
        .bundles
        .iter()
        .flat_map(|bundle| bundle.extracted_files.iter())
        .filter_map(|file| fusionforge::Asset::from_path(Path::new(&file.path)).ok())
        .map(|asset| asset.format)
        .find(|format| *format > 0)
}

pub(in super::super) fn npc_standalone_required_container_paths(
    blueprint: &NpcBlueprint,
    hints: &NpcAssetHints,
    source_index_paths: Option<&BTreeSet<String>>,
) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for asset_path in [
        blueprint.model_asset.as_deref(),
        blueprint.texture_asset.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        let normalized = normalized_asset_path(asset_path);
        if !normalized.is_empty() && !normalized.eq_ignore_ascii_case("null") {
            // The client looks the model up under both mob/<name>.kfm and
            // mob/<name>.nif; keep both container entries.
            if let Some(stem) = normalized.strip_suffix(".kfm") {
                paths.insert(format!("{stem}.nif"));
            } else if let Some(stem) = normalized.strip_suffix(".nif") {
                paths.insert(format!("{stem}.kfm"));
            }
            paths.insert(normalized);
        }
    }
    if let Some(generated_icon) = blueprint.generated_icon.as_ref() {
        let normalized = normalized_asset_path(&generated_icon.template_asset_path);
        if !normalized.is_empty() && !normalized.eq_ignore_ascii_case("null") {
            paths.insert(normalized);
        }
    } else {
        if let Some(asset_path) = blueprint.icon_asset.as_deref() {
            let normalized = normalized_asset_path(asset_path);
            if !normalized.is_empty() && !normalized.eq_ignore_ascii_case("null") {
                paths.insert(normalized);
            }
        }
    }

    let source_has = |path: &String| {
        source_index_paths
            .map(|index_paths| index_paths.contains(path))
            .unwrap_or(true)
    };
    paths.extend(
        hints
            .model_paths
            .iter()
            .filter(|path| source_has(path))
            .cloned(),
    );
    paths.extend(
        hints
            .texture_paths
            .iter()
            .filter(|path| source_has(path))
            .cloned(),
    );
    paths.extend(
        hints
            .icon_paths
            .iter()
            .filter(|path| source_has(path))
            .cloned(),
    );
    paths.extend(
        hints
            .audio_paths
            .iter()
            .filter(|path| source_has(path))
            .cloned(),
    );
    paths
}

pub(in super::super) fn npc_container_should_follow_preload(path: &str) -> bool {
    matches!(npc_container_expected_object_type(path), Some("GameObject"))
}

pub(in super::super) fn set_unity_object_i64(value: &mut fusionforge::UnityValue, key: &str, number: i64) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), fusionforge::UnityValue::Int(number));
    }
}

pub(in super::super) fn set_unity_object_string(value: &mut fusionforge::UnityValue, key: &str, text: &str) {
    if let Some(object) = value.as_object_mut() {
        object.insert(
            key.to_string(),
            fusionforge::UnityValue::String(text.to_string()),
        );
    }
}

pub(in super::super) fn npc_target_external_bundle_priority(name: &str) -> Option<u8> {
    match name.to_ascii_lowercase().as_str() {
        "tutorial.resourcefile" => Some(0),
        "npctexture.resourcefile" => Some(1),
        "traininggrounds.resourcefile" => Some(2),
        "charactercreation.resourcefile" => Some(10),
        "characterselection.resourcefile" => Some(11),
        _ => None,
    }
}

pub(in super::super) fn npc_target_external_bundle_name(name: &str) -> bool {
    npc_target_external_bundle_priority(name).is_some()
}

pub(in super::super) fn npc_bundle_should_externalize_dependencies(blueprint: &NpcBlueprint) -> bool {
    // Clean authoring (GLB) bundles inline their toon shader and ToonRamp9 as
    // local objects so the runtime never has to resolve a cross-bundle PPtr;
    // externalizing them again (by shader fingerprint/name) would reintroduce
    // the unresolved-shader failure mode that renders the NPC invisible.
    !npc_blueprint_uses_authoring_model_bundle(blueprint)
}

pub(in super::super) fn npc_bundle_should_use_safe_imported_fallback(blueprint: &NpcBlueprint) -> bool {
    let _ = blueprint;
    false
}

pub(in super::super) fn assetbundle_value_has_container_prefix(value: &fusionforge::UnityValue, prefix: &str) -> bool {
    fusionforge::value_array(value.get("m_Container"))
        .iter()
        .any(|entry| {
            fusionforge::pair_name_value(entry)
                .map(|(path, _)| normalized_asset_path(path).starts_with(prefix))
                .unwrap_or(false)
        })
}

pub(in super::super) fn unity_object_type_tree_key(info: &fusionforge::ObjectInfo) -> i32 {
    if info.type_id != 0 {
        info.type_id
    } else {
        info.class_id
    }
}

pub(in super::super) fn serialize_npc_output_object_value(
    asset: &fusionforge::Asset,
    asset_index: usize,
    info: &fusionforge::ObjectInfo,
    value: &fusionforge::UnityValue,
) -> Result<Vec<u8>, String> {
    let tree = asset.object_type_tree(info)?;
    let coerced = fusionforge::coerce_value_for_type(tree, value)?;
    asset.serialize_object_value_with_tree(asset_index, tree, &coerced)
}

pub(in super::super) fn normalize_imported_npc_output_object(object_type: &str, value: &mut fusionforge::UnityValue) {
    match object_type {
        "GameObject" => set_gameobject_active(value, true),
        "MeshRenderer" | "SkinnedMeshRenderer" => enable_renderer(value),
        "MonoBehaviour" => {
            if let Some(script) = value.get_mut("m_Script") {
                *script = fusionforge::UnityValue::Pointer(fusionforge::Pointer {
                    source_asset: 0,
                    file_id: 0,
                    path_id: 0,
                });
            }
        }
        _ => {}
    }
}

pub(in super::super) fn npc_container_expected_object_type(path: &str) -> Option<&'static str> {
    let normalized = normalized_asset_path(path);
    if normalized.starts_with("mob/") {
        Some("GameObject")
    } else if normalized.starts_with("texture/") || normalized.starts_with("icons/") {
        Some("Texture2D")
    } else {
        None
    }
}

pub(in super::super) fn npc_container_expected_object_types(path: &str) -> &'static [&'static str] {
    let normalized = normalized_asset_path(path);
    if normalized.starts_with("mob/") {
        if normalized.ends_with(".kfm") || normalized.ends_with(".nif") {
            &["GameObject", "TextAsset", "Mesh"]
        } else {
            &["GameObject"]
        }
    } else if normalized.starts_with("texture/") || normalized.starts_with("icons/") {
        &["Texture2D"]
    } else {
        &[]
    }
}

pub(in super::super) fn npc_container_pointer_quality(
    env: &fusionforge::UnityEnvironment,
    source_asset_index: usize,
    container_path: &str,
    pointer: &fusionforge::Pointer,
) -> i32 {
    let expected_types = npc_container_expected_object_types(container_path);
    let expected_name = asset_stem_from_container_path(container_path)
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default();
    let mut best = 0;
    for (asset_index, path_id) in pointer_candidate_keys(env, pointer) {
        let Some(asset) = env.assets.get(asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(&path_id) else {
            continue;
        };
        let object_type = asset.object_type_name(info);
        if !expected_types.is_empty() && !expected_types.contains(&object_type.as_str()) {
            continue;
        }
        let Ok(value) = asset.read_object(asset_index, info) else {
            continue;
        };
        let object_name = fusionforge::object_name(&value).to_ascii_lowercase();
        if !expected_name.is_empty() {
            if object_type == "GameObject" {
                let expected_variants = npc_expected_model_name_variants(&expected_name);
                let exact_match = expected_variants.iter().any(|variant| {
                    object_name == *variant || object_name.starts_with(&format!("{variant}:"))
                });
                let loose_match = expected_variants
                    .iter()
                    .any(|variant| object_name.contains(variant));
                if !exact_match && !loose_match {
                    continue;
                }
                best = best.max(if exact_match {
                    if asset_index == source_asset_index {
                        2
                    } else {
                        1
                    }
                } else {
                    1
                });
                continue;
            }
        }
        best = best.max(if asset_index == source_asset_index {
            2
        } else {
            1
        });
    }
    best
}

pub(in super::super) fn npc_resolved_container_paths_from_sources(
    project: &Path,
    source_paths: &[PathBuf],
) -> Result<BTreeSet<String>, String> {
    let mut assets = Vec::<fusionforge::Asset>::new();
    for source_path in source_paths {
        assets.extend(npc_source_assets_from_path(project, source_path)?);
    }
    let env = fusionforge::UnityEnvironment::from_assets(assets);
    let mut resolved = BTreeSet::<String>::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let body = asset.read_object(asset_index, info)?;
            let preload_table = fusionforge::value_array(body.get("m_PreloadTable")).to_vec();
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
                let mut quality = npc_container_pointer_quality(&env, asset_index, path, pointer);
                if quality == 0 {
                    quality = npc_container_preload_root_pointer(
                        &env,
                        asset_index,
                        path,
                        &preload_table,
                        metadata,
                    )
                    .map(|(_, preload_quality)| preload_quality)
                    .unwrap_or_default();
                }
                if quality > 0 {
                    resolved.insert(normalized_asset_path(path));
                }
            }
        }
    }
    Ok(resolved)
}

pub(in super::super) fn npc_container_preload_root_pointer(
    env: &fusionforge::UnityEnvironment,
    source_asset_index: usize,
    container_path: &str,
    preload_table: &[fusionforge::UnityValue],
    metadata: &fusionforge::UnityValue,
) -> Option<(fusionforge::Pointer, i32)> {
    let (start, end) = metadata_preload_range(metadata, preload_table.len());
    let mut best: Option<(fusionforge::Pointer, i32)> = None;
    for preload in &preload_table[start..end] {
        let Some(pointer) = preload.as_pointer() else {
            continue;
        };
        let quality =
            npc_container_pointer_quality(env, source_asset_index, container_path, pointer);
        if quality == 0 {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|(_, best_quality)| quality > *best_quality)
        {
            best = Some((pointer.clone(), quality));
            if quality >= 2 {
                break;
            }
        }
    }
    best
}
