use super::*;

pub(super) fn load_input(path: &Path) -> Result<LoadedInput, String> {
    let assets = super::super::direct_input::read_assets(path)?;
    let source_asset_names = assets.iter().map(|asset| asset.name.clone()).collect();
    let env = UnityEnvironment::from_assets(assets);
    Ok(LoadedInput { env, _session_dir: None, source_asset_names })
}

pub(super) fn load_input_with_sibling_dependencies(path: &Path) -> Result<LoadedInput, String> {
    let (env, source_asset_names) = super::super::direct_input::load(path)?;
    Ok(LoadedInput { env, _session_dir: None, source_asset_names })
}

pub(super) fn collect_transform_relative_paths(
    env: &UnityEnvironment,
    transform: &Pointer,
    current_path: &str,
    paths: &mut BTreeSet<String>,
    seen: &mut HashSet<ObjectKey>,
) -> Result<(), String> {
    let (key, asset, info, body) = resolved_body(env, transform)?;
    if asset.object_type_name(info) != "Transform" {
        return Err(format!(
            "expected Transform, got {} at {}#{}",
            asset.object_type_name(info),
            asset.name,
            info.path_id
        ));
    }
    if !seen.insert(key) {
        return Ok(());
    }
    for child in value_array(body.get("m_Children")) {
        let Some(child_pointer) = child.as_pointer() else {
            continue;
        };
        let (_, child_asset, child_info, child_body) = resolved_body(env, child_pointer)?;
        if child_asset.object_type_name(child_info) != "Transform" {
            return Err(format!(
                "expected child Transform, got {} at {}#{}",
                child_asset.object_type_name(child_info),
                child_asset.name,
                child_info.path_id
            ));
        }
        let gameobject_pointer = child_body
            .get("m_GameObject")
            .and_then(UnityValue::as_pointer)
            .ok_or_else(|| {
                format!(
                    "Transform {}#{} missing m_GameObject",
                    child_asset.name, child_info.path_id
                )
            })?;
        let (_, gameobject_asset, gameobject_info, gameobject_body) =
            resolved_body(env, gameobject_pointer)?;
        if gameobject_asset.object_type_name(gameobject_info) != "GameObject" {
            return Err(format!(
                "expected GameObject, got {} at {}#{}",
                gameobject_asset.object_type_name(gameobject_info),
                gameobject_asset.name,
                gameobject_info.path_id
            ));
        }
        let child_name = object_name(&gameobject_body);
        let child_path = if current_path.is_empty() {
            child_name.clone()
        } else if child_name.is_empty() {
            current_path.to_string()
        } else {
            format!("{current_path}/{child_name}")
        };
        if !child_path.is_empty() {
            paths.insert(child_path.clone());
        }
        collect_transform_relative_paths(env, child_pointer, &child_path, paths, seen)?;
    }
    Ok(())
}

pub(super) fn parse_extract_options(args: &[String]) -> Result<ExtractOptions, String> {
    let mut options = ExtractOptions {
        outdir: PathBuf::new(),
        ..Default::default()
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--all" => {
                options.types.extend([
                    "AudioClip".to_string(),
                    "Font".to_string(),
                    "Mesh".to_string(),
                    "MovieTexture".to_string(),
                    "Shader".to_string(),
                    "TextAsset".to_string(),
                    "Texture2D".to_string(),
                ]);
            }
            "--models" => {
                options.types.insert("Mesh".to_string());
            }
            "--images" => {
                options.types.insert("Texture2D".to_string());
            }
            "--text" => {
                options.types.insert("TextAsset".to_string());
            }
            "--shaders" => {
                options.types.insert("Shader".to_string());
            }
            "--fonts" => {
                options.types.insert("Font".to_string());
            }
            "--audio" => {
                options.types.insert("AudioClip".to_string());
            }
            "--video" => {
                options.types.insert("MovieTexture".to_string());
            }
            "-n" | "--dry-run" => options.dry_run = true,
            "-o" | "--outdir" => {
                index += 1;
                options.outdir = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "-o requires a directory".to_string())?,
                );
            }
            "--filter" => {
                index += 1;
                options.filters.push(
                    args.get(index)
                        .ok_or_else(|| "--filter requires a value".to_string())?
                        .to_lowercase(),
                );
            }
            value => options.files.push(PathBuf::from(value)),
        }
        index += 1;
    }

    if options.types.is_empty() {
        options.types.insert("Mesh".to_string());
        options.types.insert("Texture2D".to_string());
        options.types.insert("TextAsset".to_string());
        options.types.insert("Shader".to_string());
    }
    if options.outdir.as_os_str().is_empty() {
        options.outdir = PathBuf::from(".");
    }
    if options.files.is_empty() {
        return Err("unityextract requires at least one file".to_string());
    }
    Ok(options)
}

pub(super) fn find_first_type<'a>(
    env: &'a UnityEnvironment,
    type_name: &str,
) -> Result<Option<(usize, &'a Asset, &'a ObjectInfo, UnityValue)>, String> {
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) == type_name {
                let body = asset.read_object(asset_index, info)?;
                return Ok(Some((asset_index, asset, info, body)));
            }
        }
    }
    Ok(None)
}

pub(super) fn find_gameobject_root(env: &UnityEnvironment, wanted: &str) -> Option<super::super::unity::ObjectKey> {
    for (asset_index, asset) in env.assets.iter().enumerate() {
        if let Some(assetbundle) = asset.objects.get(&1) {
            if asset.object_type_name(assetbundle) == "AssetBundle" {
                let body = asset.read_object(asset_index, assetbundle).ok()?;
                for (path, metadata) in container_entries(&body) {
                    if path == wanted {
                        let pointer = metadata.get("asset").and_then(UnityValue::as_pointer)?;
                        return env.resolve_pointer(pointer).ok();
                    }
                }
            }
        }
    }
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for (path_id, info) in &asset.objects {
            if asset.object_type_name(info) == "GameObject" {
                let body = asset.read_object(asset_index, info).ok()?;
                if object_name(&body) == wanted {
                    return Some(super::super::unity::ObjectKey {
                        asset: asset_index,
                        path_id: *path_id,
                    });
                }
            }
        }
    }
    None
}

pub(super) fn collect_pointers(value: &UnityValue, key: &str) -> Vec<(String, Pointer)> {
    match value {
        UnityValue::Pointer(pointer) if !pointer.is_null() => {
            vec![(key.to_string(), pointer.clone())]
        }
        UnityValue::Array(values) => values
            .iter()
            .enumerate()
            .flat_map(|(index, value)| collect_pointers(value, &format!("{key}[{index}]")))
            .collect(),
        UnityValue::Object(values) => values
            .iter()
            .flat_map(|(child_key, value)| collect_pointers(value, child_key))
            .collect(),
        UnityValue::Pair(left, right) => {
            let mut values = collect_pointers(left, key);
            values.extend(collect_pointers(right, key));
            values
        }
        _ => Vec::new(),
    }
}

pub(super) fn parse_i64(value: &str) -> Result<i64, String> {
    if let Some(value) = value.strip_prefix("0x") {
        i64::from_str_radix(value, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<i64>().map_err(|err| err.to_string())
    }
}
