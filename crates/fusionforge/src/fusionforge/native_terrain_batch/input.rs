use super::*;

pub(super) fn load_map_scene(
    source: &TileSource,
    build_root: &Path,
    repo_root: &Path,
    archive_index: &HashMap<String, PathBuf>,
    session_path: &Path,
) -> Result<LoadedMapScene, String> {
    if session_path.exists() {
        return Err(format!(
            "map scene extraction session already exists: {}",
            session_path.display()
        ));
    }
    if let Some(parent) = session_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
    }
    fs::create_dir(session_path)
        .map_err(|err| format!("could not create {}: {err}", session_path.display()))?;
    let session = SessionDirectory::new(session_path.to_path_buf());
    extract_bundle(&source.map_scene_path, session.path())?;
    let mut env = UnityEnvironment::from_dir(session.path());
    let map_asset_names = env
        .assets
        .iter()
        .map(|asset| asset.name.clone())
        .collect::<BTreeSet<_>>();
    if map_asset_names.is_empty() {
        return Err(format!(
            "{} extracted no serialized map-scene assets",
            source.map_scene_path.display()
        ));
    }
    let mut extracted_paths = HashSet::from([normalize_filesystem_path(&source.map_scene_path)]);
    let mut dependency_sides = Vec::<JsonValue>::new();
    let mut missing = HashSet::<String>::new();
    let mut indexes = HashMap::from([(build_root.to_path_buf(), archive_index.clone())]);
    loop {
        let dependencies = normalized_dependency_requests(collect_archive_dependencies(&env));
        if !extract_archives(
            &dependencies,
            Some(&source.map_scene_path),
            Some(build_root),
            repo_root,
            session.path(),
            &mut extracted_paths,
            &mut dependency_sides,
            &mut missing,
            &mut indexes,
        ) {
            break;
        }
        env = UnityEnvironment::from_dir(session.path());
    }
    if !missing.is_empty() {
        let mut missing = missing.into_iter().collect::<Vec<_>>();
        missing.sort();
        return Err(format!(
            "unresolved map scene dependency archives: {}",
            missing.join(", ")
        ));
    }
    Ok(LoadedMapScene {
        env,
        map_asset_names,
        _session: session,
    })
}

pub(super) fn find_exact_scene_script_objects(
    env: &UnityEnvironment,
    map_asset_names: &BTreeSet<String>,
    script_true_name: &str,
) -> Result<Vec<ExactSceneObject>, String> {
    let mut matches = Vec::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        if !map_asset_names.contains(&asset.name) {
            continue;
        }
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "MonoBehaviour" {
                continue;
            }
            let body = asset.read_object(asset_index, info).map_err(|err| {
                format!(
                    "could not read MonoBehaviour {}#{}: {err}",
                    asset.name, info.path_id
                )
            })?;
            let script_pointer = body
                .get("m_Script")
                .and_then(UnityValue::as_pointer)
                .cloned();
            let script_name = script_pointer
                .as_ref()
                .and_then(|pointer| resolved_object_name(env, pointer));
            if script_name.as_deref() != Some(script_true_name) {
                continue;
            }
            matches.push(exact_scene_object_from_body(
                asset_index,
                asset,
                info,
                body,
                script_pointer,
                script_name,
            )?);
        }
    }
    matches.sort_by(|left, right| {
        left.asset_name
            .cmp(&right.asset_name)
            .then_with(|| left.key.path_id.cmp(&right.key.path_id))
    });
    Ok(matches)
}

pub(super) fn parse_tile_coordinates(tile_id: &str) -> Result<(i64, i64), String> {
    let (x, y) = tile_id
        .split_once('_')
        .ok_or_else(|| format!("invalid tile id {tile_id:?}"))?;
    if y.contains('_') || !valid_tile_component(x) || !valid_tile_component(y) {
        return Err(format!("invalid tile id {tile_id:?}"));
    }
    Ok((
        x.parse()
            .map_err(|_| format!("invalid tile x in {tile_id:?}"))?,
        y.parse()
            .map_err(|_| format!("invalid tile y in {tile_id:?}"))?,
    ))
}

pub(super) fn resolve_effective_build_root(input: &Path) -> Result<(PathBuf, &'static str), String> {
    if !input.is_dir() {
        return Err(format!(
            "native terrain batch input must be a client-project or effective build directory: {}",
            input.display()
        ));
    }
    if directory_has_dong_resources(input)? {
        return Ok((
            fs::canonicalize(input).map_err(|err| format!("{}: {err}", input.display()))?,
            "effective-build-root",
        ));
    }
    let config_path = input.join("ffpatch.json");
    if !config_path.is_file() {
        return Err(format!(
            "{} is neither an effective build root nor an FusionForge project with ffpatch.json",
            input.display()
        ));
    }
    let bytes = fs::read(&config_path)
        .map_err(|err| format!("could not read {}: {err}", config_path.display()))?;
    let config: JsonValue = serde_json::from_slice(&bytes)
        .map_err(|err| format!("could not parse {}: {err}", config_path.display()))?;
    let source = config
        .get("Source")
        .and_then(JsonValue::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{} has no non-empty Source", config_path.display()))?;
    let source = PathBuf::from(source);
    let source = if source.is_absolute() {
        source
    } else {
        input.join(source)
    };
    if !source.is_dir() || !directory_has_dong_resources(&source)? {
        return Err(format!(
            "{} Source is not an effective build root with DongResources tiles: {}",
            config_path.display(),
            source.display()
        ));
    }
    Ok((
        fs::canonicalize(&source).map_err(|err| format!("{}: {err}", source.display()))?,
        "client-project-source",
    ))
}

pub(super) fn load_tile(
    source: &TileSource,
    build_root: &Path,
    repo_root: &Path,
    archive_index: &HashMap<String, PathBuf>,
    session_path: &Path,
) -> Result<LoadedTile, String> {
    if session_path.exists() {
        return Err(format!(
            "tile extraction session already exists: {}",
            session_path.display()
        ));
    }
    if let Some(parent) = session_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
    }
    fs::create_dir(session_path)
        .map_err(|err| format!("could not create {}: {err}", session_path.display()))?;
    let session = SessionDirectory::new(session_path.to_path_buf());
    extract_bundle(&source.resource_path, session.path())?;
    let resource_env = UnityEnvironment::from_dir(session.path());
    let resource_asset_names = resource_env
        .assets
        .iter()
        .map(|asset| asset.name.clone())
        .collect::<BTreeSet<_>>();
    if resource_asset_names.is_empty() {
        return Err(format!(
            "{} extracted no serialized assets",
            source.resource_path.display()
        ));
    }
    extract_bundle(&source.map_scene_path, session.path())?;
    let map_and_resource_env = UnityEnvironment::from_dir(session.path());
    let map_asset_names = map_and_resource_env
        .assets
        .iter()
        .map(|asset| asset.name.clone())
        .filter(|name| !resource_asset_names.contains(name))
        .collect::<BTreeSet<_>>();
    if map_asset_names.is_empty() {
        return Err(format!(
            "{} extracted no distinct map-scene serialized assets",
            source.map_scene_path.display()
        ));
    }

    let mut env = map_and_resource_env;
    let mut extracted_paths = HashSet::from([
        normalize_filesystem_path(&source.resource_path),
        normalize_filesystem_path(&source.map_scene_path),
    ]);
    let mut dependency_sides = Vec::<JsonValue>::new();
    let mut missing = HashSet::<String>::new();
    let mut indexes = HashMap::from([(build_root.to_path_buf(), archive_index.clone())]);
    loop {
        let dependencies = normalized_dependency_requests(collect_archive_dependencies(&env));
        if !extract_archives(
            &dependencies,
            Some(&source.map_scene_path),
            Some(build_root),
            repo_root,
            session.path(),
            &mut extracted_paths,
            &mut dependency_sides,
            &mut missing,
            &mut indexes,
        ) {
            break;
        }
        env = UnityEnvironment::from_dir(session.path());
    }
    if !missing.is_empty() {
        let mut missing = missing.into_iter().collect::<Vec<_>>();
        missing.sort();
        return Err(format!(
            "unresolved dependency archives: {}",
            missing.join(", ")
        ));
    }
    Ok(LoadedTile {
        env,
        resource_asset_names,
        map_asset_names,
        _session: session,
    })
}

pub(super) fn parse_dong_tile_name(file_name: &str) -> Option<String> {
    let suffix = file_name
        .strip_prefix("DongResources_")?
        .strip_suffix(".resourceFile")?;
    let (x, y) = suffix.split_once('_')?;
    if y.contains('_') || !valid_tile_component(x) || !valid_tile_component(y) {
        return None;
    }
    Some(format!("{x}_{y}"))
}
