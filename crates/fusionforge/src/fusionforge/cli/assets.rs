use super::*;

pub(super) fn catalog_logical_models(args: &[String]) -> Result<(), String> {
    if !(1..=2).contains(&args.len()) {
        return Err("catalog-logical-models <bundle-index.json> [out.json|-]".to_string());
    }
    let input_path = Path::new(&args[0]);
    let catalog = super::super::logical_model_catalog::catalog_logical_models_from_path(input_path)?;
    let output = format!(
        "{}\n",
        serde_json::to_string_pretty(&catalog).map_err(|err| err.to_string())?
    );

    match args.get(1).map(String::as_str) {
        None | Some("-") => {
            print!("{output}");
            Ok(())
        }
        Some(output_path) => {
            let output_path = Path::new(output_path);
            if output_path == input_path {
                return Err("catalog output must not overwrite bundle-index.json".to_string());
            }
            fs::write(output_path, output).map_err(|err| {
                format!(
                    "could not write logical-model catalog {}: {err}",
                    output_path.display()
                )
            })
        }
    }
}

pub(super) fn cli_normalized_asset_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .to_lowercase()
}

pub(super) fn caching_manifest_entry(file_name: &str, version: i64) -> UnityValue {
    let mut object = BTreeMap::new();
    object.insert(
        "fileName".to_string(),
        UnityValue::String(file_name.to_string()),
    );
    object.insert("version".to_string(), UnityValue::Int(version));
    UnityValue::Object(object)
}

pub(super) fn patch_caching_manifest_bundles(args: &[String]) -> Result<(), String> {
    let path = required_path(
        args,
        0,
        "patch-caching-manifest-bundles <sharedassets.assets> <bundle-name>...",
    )?;
    if args.len() < 2 {
        return Err("patch-caching-manifest-bundles needs at least one bundle name".to_string());
    }
    let asset = Asset::from_path(&path)?;
    let mut additions = BTreeMap::<String, String>::new();
    for name in args.iter().skip(1) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.ends_with(".resourcefile") || lower.ends_with(".unity3d") {
            additions
                .entry(lower)
                .or_insert_with(|| trimmed.to_string());
        }
    }
    if additions.is_empty() {
        return Ok(());
    }

    let mut replacements = BTreeMap::<i64, Vec<u8>>::new();
    let mut patched = 0usize;
    let mut added = 0usize;
    for info in asset.objects.values() {
        if asset.object_type_name(info) != "MonoBehaviour" {
            continue;
        }
        let mut value = asset.read_object(0, info)?;
        if object_name(&value) != "CachingManifest" {
            continue;
        }
        let Some(entries) = value
            .get_mut("m_CharacterCreation")
            .and_then(UnityValue::as_array_mut)
        else {
            continue;
        };
        let existing = entries
            .iter()
            .filter_map(|entry| entry.get("fileName").and_then(UnityValue::as_str))
            .map(|name| name.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        let mut version = entries
            .iter()
            .filter_map(|entry| entry.get("version").and_then(UnityValue::as_i64))
            .max()
            .unwrap_or(800000)
            .saturating_add(1);
        for (lower, name) in &additions {
            if existing.contains(lower) {
                continue;
            }
            entries.push(caching_manifest_entry(name, version));
            version = version.saturating_add(1);
            added += 1;
        }
        replacements.insert(info.path_id, asset.serialize_object_value(0, info, &value)?);
        patched += 1;
    }

    if patched == 0 {
        return Err(format!(
            "{}: CachingManifest MonoBehaviour was not found",
            path.display()
        ));
    }
    if added == 0 {
        println!(
            "{}: CachingManifest already contains requested bundles",
            path.display()
        );
        return Ok(());
    }
    let rebuilt = asset.rebuild_with_object_data(&replacements)?;
    fs::write(&path, rebuilt).map_err(|err| format!("{}: {err}", path.display()))?;
    println!(
        "{}: added {} bundle(s) to CachingManifest.m_CharacterCreation",
        path.display(),
        added
    );
    Ok(())
}

pub(super) fn remove_caching_manifest_bundles(args: &[String]) -> Result<(), String> {
    let path = required_path(
        args,
        0,
        "remove-caching-manifest-bundles <sharedassets.assets> <bundle-name>...",
    )?;
    if args.len() < 2 {
        return Err("remove-caching-manifest-bundles needs at least one bundle name".to_string());
    }
    let remove = args
        .iter()
        .skip(1)
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .collect::<BTreeSet<_>>();
    if remove.is_empty() {
        return Ok(());
    }

    let asset = Asset::from_path(&path)?;
    let mut replacements = BTreeMap::<i64, Vec<u8>>::new();
    let mut removed = 0usize;
    for info in asset.objects.values() {
        if asset.object_type_name(info) != "MonoBehaviour" {
            continue;
        }
        let mut value = asset.read_object(0, info)?;
        if object_name(&value) != "CachingManifest" {
            continue;
        }
        let Some(entries) = value
            .get_mut("m_CharacterCreation")
            .and_then(UnityValue::as_array_mut)
        else {
            continue;
        };
        let before = entries.len();
        entries.retain(|entry| {
            entry
                .get("fileName")
                .and_then(UnityValue::as_str)
                .map(|name| !remove.contains(&name.to_ascii_lowercase()))
                .unwrap_or(true)
        });
        removed += before.saturating_sub(entries.len());
        replacements.insert(info.path_id, asset.serialize_object_value(0, info, &value)?);
    }

    if replacements.is_empty() {
        return Err(format!(
            "{}: CachingManifest MonoBehaviour was not found",
            path.display()
        ));
    }
    if removed == 0 {
        println!("{}: no requested bundles were present", path.display());
        return Ok(());
    }
    let rebuilt = asset.rebuild_with_object_data(&replacements)?;
    fs::write(&path, rebuilt).map_err(|err| format!("{}: {err}", path.display()))?;
    println!(
        "{}: removed {} bundle(s) from CachingManifest.m_CharacterCreation",
        path.display(),
        removed
    );
    Ok(())
}

pub(super) fn show_caching_manifest(args: &[String]) -> Result<(), String> {
    let args = args.to_vec();
    run_large_stack_task(move || {
        let path = required_path(&args, 0, "show-caching-manifest <asset-or-bundle> [filter]")?;
        let filter = args.get(1).map(|value| value.to_ascii_lowercase());
        let loaded = load_input(&path)?;
        let mut found = false;
        for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
            for info in asset.objects.values() {
                if asset.object_type_name(info) != "MonoBehaviour" {
                    continue;
                }
                let body = asset.read_object(asset_index, info)?;
                if object_name(&body) != "CachingManifest" {
                    continue;
                }
                found = true;
                println!("# {}#{} CachingManifest", asset.name, info.path_id);
                for section in [
                    "m_CharacterCreation",
                    "m_CharacterSelection",
                    "m_Tutorial",
                    "m_FreeZone",
                    "m_FreeZoneComplete",
                    "m_PaidZone",
                    "m_PaidZoneComplete",
                    "m_Map",
                    "m_DongResources",
                    "m_Resource",
                    "m_Scene",
                ] {
                    let entries = value_array(body.get(section));
                    if entries.is_empty() {
                        continue;
                    }
                    println!("{section}: {}", entries.len());
                    for entry in entries {
                        let name = entry
                            .get("fileName")
                            .and_then(UnityValue::as_str)
                            .unwrap_or_default();
                        if let Some(filter) = &filter {
                            if !name.to_ascii_lowercase().contains(filter) {
                                continue;
                            }
                        }
                        let version = entry
                            .get("version")
                            .and_then(UnityValue::as_i64)
                            .unwrap_or_default();
                        println!("  {version}\t{name}");
                    }
                }
            }
        }
        if !found {
            return Err(format!("{}: CachingManifest was not found", path.display()));
        }
        Ok(())
    })
}

pub(super) fn collect_pointers_with_path(
    value: &UnityValue,
    path: &str,
    pointers: &mut Vec<(String, Pointer)>,
) {
    match value {
        UnityValue::Pointer(pointer) if !pointer.is_null() => {
            pointers.push((path.to_string(), pointer.clone()));
        }
        UnityValue::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                let child = format!("{path}[{index}]");
                collect_pointers_with_path(item, &child, pointers);
            }
        }
        UnityValue::Object(values) => {
            for (key, item) in values {
                let child = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                collect_pointers_with_path(item, &child, pointers);
            }
        }
        UnityValue::Pair(left, right) => {
            collect_pointers_with_path(left, &format!("{path}.key"), pointers);
            collect_pointers_with_path(right, &format!("{path}.value"), pointers);
        }
        _ => {}
    }
}

pub(super) fn asset_ref_label(env: &UnityEnvironment, pointer: &Pointer) -> String {
    let Some(asset) = env.assets.get(pointer.source_asset) else {
        return "missing source asset".to_string();
    };
    let Some(asset_ref) = asset.asset_refs.get(pointer.file_id as usize) else {
        return format!("missing asset ref {}", pointer.file_id);
    };
    if !asset_ref.file_path.is_empty() {
        return asset_ref.file_path.clone();
    }
    asset_ref.asset_path.clone()
}

/// Build the `cache/bundle-index.json` a source build needs before any exact
/// logical-model export can resolve cross-bundle materials, shaders and
/// textures. Without this index the dependency resolver silently finds nothing.
pub(super) fn index_client_project_command(args: &[String]) -> Result<(), String> {
    if !(1..=2).contains(&args.len()) {return Err("index-client <raw-build-root> [final-reference.json]".into());}
    let source=Path::new(&args[0]).canonicalize().map_err(|e|e.to_string())?;
    let mut paths=fs::read_dir(&source).map_err(|e|e.to_string())?.map(|entry|entry.map(|e|e.path()).map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?;
    paths.sort();
    let mut rows=Vec::new();
    for path in paths {
        if !path.is_file() {continue;}
        if !matches!(path.extension().and_then(|s|s.to_str()),Some("unity3d"|"resourceFile")) {continue;}
        let entries=ffbuildtool::bundle::AssetBundle::entry_names(&path)?;
        rows.push(json!({"container":path.file_name().unwrap().to_string_lossy(),"bytes":fs::metadata(&path).map_err(|e|e.to_string())?.len(),"entries":entries}));
    }
    let result=json!({"sourceDir":source,"bundles":rows.len(),"containers":rows,"scope":"top-level immutable containers; no extraction or object cache"});
    if let Some(destination)=args.get(1) {
        let output=super::super::workspace::resolve_destination(Path::new(destination))?;
        if output.starts_with(&source) {return Err("reference output cannot be inside the source build".into());}
        let root=output.parent().ok_or("output has no parent")?;
        ffone_asset_pipeline::direct_output::install(root,&[(PathBuf::from(output.file_name().ok_or("output file missing")?),serde_json::to_vec_pretty(&result).map_err(|e|e.to_string())?)])?;
    }
    println!("{}",serde_json::to_string(&result).map_err(|e|e.to_string())?);
    Ok(())
}

pub(super) fn npc_snapshot_container_output_path(path: &str, object_type: &str) -> PathBuf {
    let normalized = path.replace('\\', "/");
    let mut parts = normalized.split('/').collect::<Vec<_>>();
    let file_name = parts.pop().unwrap_or("asset");
    let folder = match normalized.to_ascii_lowercase().as_str() {
        value if value.starts_with("mob/") => "models",
        value if value.starts_with("texture/") => "textures",
        value if value.starts_with("icons/") => "icons",
        value
            if value.starts_with("vo/")
                || value.starts_with("sound/")
                || value.starts_with("ui sound/") =>
        {
            "audio"
        }
        _ => "containers",
    };
    let mut output = PathBuf::from(folder);
    if folder == "audio" {
        for part in parts {
            output.push(safe_name(part, 0));
        }
    }
    output.push(fix_extension_for_type(file_name, object_type));
    output
}

pub(super) fn npc_snapshot_dependency_output_path(object_type: &str, name: &str, path_id: i64) -> PathBuf {
    let folder = match object_type {
        "AnimationClip" => "animations",
        "GameObject" => "gameobjects",
        "Material" => "materials",
        "Mesh" => "meshes",
        "SkinnedMeshRenderer" | "MeshRenderer" => "renderers",
        "Transform" => "transforms",
        "Texture2D" => "textures",
        "AudioClip" => "audio",
        _ => "objects",
    };
    let extension = match object_type {
        "AnimationClip" => "anim",
        "GameObject" => "gameobject",
        "Material" => "mat",
        "Mesh" => "mesh",
        "SkinnedMeshRenderer" | "MeshRenderer" => "renderer",
        "Transform" => "transform",
        "Texture2D" => "png",
        "AudioClip" => "ogg",
        _ => "unityobj",
    };
    PathBuf::from(folder).join(format!(
        "{}__{}.{}",
        safe_name(name, path_id),
        path_id,
        extension
    ))
}

pub(super) fn resolved_object_index_info<'a>(
    env: &'a UnityEnvironment,
    source_asset_index: usize,
    pointer: &Pointer,
) -> Option<(usize, &'a ObjectInfo)> {
    let key = env.resolve_pointer(pointer).ok().or_else(|| {
        (pointer.file_id == 0).then_some(ObjectKey {
            asset: source_asset_index,
            path_id: pointer.path_id,
        })
    })?;
    let asset = env.assets.get(key.asset)?;
    let info = asset.objects.get(&key.path_id)?;
    Some((key.asset, info))
}

pub(super) fn required_path(args: &[String], index: usize, usage: &str) -> Result<PathBuf, String> {
    args.get(index)
        .map(PathBuf::from)
        .ok_or_else(|| format!("usage: {usage}"))
}

pub(super) fn fix_export_path(path: &str) -> PathBuf {
    fix_export_path_for_type(path, "")
}

pub(super) fn fix_export_path_for_type(path: &str, object_type: &str) -> PathBuf {
    let mut path = PathBuf::from(path);
    if let Some(name) = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|name| fix_extension_for_type(name, object_type))
    {
        path.set_file_name(name);
    }
    path
}
