use std::{
    collections::BTreeMap,
    env, fs,
    path::{Component, Path, PathBuf},
};

use serde_json::Value;

pub(super) fn run(command_args: &[String]) -> Result<(), String> {
    let project_root = command_args.iter().map(std::ffi::OsString::from).next()
        .map(PathBuf::from)
        .unwrap_or(env::current_dir().map_err(|error| error.to_string())?);
    let asset_root = project_root.join("assets/game");
    let object_root = asset_root.join("objects");
    let legacy_root = asset_root.join("map/objects");
    let catalog_path = asset_root.join("map/catalog.json");
    if !object_root.is_dir() {
        return Err(format!(
            "normalized object root is absent: {}",
            object_root.display()
        ));
    }
    if legacy_root.exists() {
        return Err(format!(
            "refusing an ambiguous repair while the legacy root exists: {}",
            legacy_root.display()
        ));
    }

    let mut catalog = read_json(&catalog_path)?;
    let published = load_published_objects(&asset_root, &object_root)?;
    let resource_sets = required_array_mut(&mut catalog, "resourceSets")?;
    if resource_sets.len() != published.sets.len() {
        return Err(format!(
            "catalog/published resource-set count differs: {} != {}",
            resource_sets.len(),
            published.sets.len()
        ));
    }

    let mut directory_routes = BTreeMap::<String, String>::new();
    let mut set_roots = BTreeMap::<String, (String, String)>::new();
    for entry in resource_sets {
        let id = required_string(entry, "id", "catalog resource set")?.to_owned();
        let current = published
            .sets
            .get(&id)
            .ok_or_else(|| format!("published resource set is absent for {id}"))?;
        let definition = entry
            .get_mut("definition")
            .ok_or_else(|| format!("resource set {id} has no definition"))?;
        let old_path = artifact_path(definition, "catalog resource set")?.to_owned();
        let old_root = parent_route(&old_path)?;
        let current_root = parent_route(artifact_path(current, "published resource set")?)?;
        set_roots.insert(id, (old_root, current_root));
        *definition = current.clone();
    }

    let prefabs = required_array_mut(&mut catalog, "objects")?;
    let prefab_count = prefabs.len();
    for entry in prefabs {
        let id = required_string(entry, "id", "catalog object")?.to_owned();
        let current = published
            .members
            .get(&id)
            .ok_or_else(|| format!("published object is absent for {id}"))?;
        let definition = entry
            .get_mut("definition")
            .ok_or_else(|| format!("object {id} has no definition"))?;
        let old_parent = parent_route(artifact_path(definition, "catalog object")?)?;
        let current_parent = parent_route(artifact_path(current, "published object")?)?;
        if let Some(previous) = directory_routes.insert(old_parent.clone(), current_parent.clone())
            && previous != current_parent
        {
            return Err(format!(
                "legacy object directory {old_parent:?} maps to both {previous:?} and {current_parent:?}"
            ));
        }
        *definition = current.clone();
    }

    let composites = optional_array_mut(&mut catalog, "compositeObjects")?;
    let composite_count = composites.len();
    if prefab_count + composite_count != published.members.len() {
        return Err(format!(
            "catalog/published object count differs: {} + {} != {}",
            prefab_count,
            composite_count,
            published.members.len()
        ));
    }
    for entry in composites {
        let id = required_string(entry, "id", "composite object")?.to_owned();
        let current = published
            .members
            .get(&id)
            .ok_or_else(|| format!("published composite object is absent for {id}"))?;
        let definition = entry
            .get_mut("definition")
            .ok_or_else(|| format!("composite object {id} has no definition"))?;
        let old_parent = parent_route(artifact_path(definition, "composite object")?)?;
        let current_parent = parent_route(artifact_path(current, "published object")?)?;
        if let Some(previous) = directory_routes.insert(old_parent.clone(), current_parent.clone())
            && previous != current_parent
        {
            return Err(format!(
                "legacy object directory {old_parent:?} maps to both {previous:?} and {current_parent:?}"
            ));
        }
        *definition = current.clone();
    }

    for entry in required_array_mut(&mut catalog, "geometry")? {
        let id = required_string(entry, "id", "map geometry")?.to_owned();
        let model = entry
            .get_mut("model")
            .ok_or_else(|| format!("geometry {id} has no model"))?;
        repair_member_artifact(&asset_root, model, &directory_routes, &id)?;
    }

    for entry in optional_array_mut(&mut catalog, "compositeObjects")? {
        let id = required_string(entry, "id", "composite object")?.to_owned();
        let resource_set = required_string(entry, "resourceSet", "composite object")?.to_owned();
        let (old_set_root, current_set_root) = set_roots
            .get(&resource_set)
            .ok_or_else(|| format!("composite object {id} has unknown set {resource_set}"))?;
        let current_textures = published
            .textures
            .get(&resource_set)
            .ok_or_else(|| format!("published textures are absent for {resource_set}"))?;
        let files = entry
            .get_mut("files")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| format!("composite object {id} has no files"))?;
        for artifact in files {
            let old_path = artifact_path(artifact, "composite object file")?.to_owned();
            let repaired = if let Some(path) = replace_directory(&old_path, &directory_routes) {
                path
            } else if let Some(texture_suffix) = old_path
                .strip_prefix(old_set_root)
                .and_then(|suffix| suffix.strip_prefix("/textures/"))
            {
                repair_texture_route(
                    &asset_root,
                    texture_suffix,
                    current_set_root,
                    current_textures,
                    &resource_set,
                    artifact,
                )?
            } else {
                return Err(format!(
                    "composite object {id} owns an unmapped legacy file {old_path:?}"
                ));
            };
            set_artifact(&asset_root, artifact, &repaired)?;
        }
    }

    for artifact in optional_array_mut(&mut catalog, "sharedFiles")? {
        let old_path = artifact_path(artifact, "map shared file")?.to_owned();
        if !old_path.starts_with("map/objects/") {
            continue;
        }
        let repaired = if let Some(path) = replace_directory(&old_path, &directory_routes) {
            path
        } else {
            let (resource_set, old_set_root, current_set_root) = set_roots
                .iter()
                .find_map(|(id, (old_root, current_root))| {
                    old_path
                        .strip_prefix(old_root)
                        .and_then(|suffix| suffix.strip_prefix("/textures/"))
                        .map(|suffix| (id, suffix, current_root))
                })
                .ok_or_else(|| format!("map shared file has no normalized owner: {old_path:?}"))?;
            let current_textures = published
                .textures
                .get(resource_set)
                .ok_or_else(|| format!("published textures are absent for {resource_set}"))?;
            repair_texture_route(
                &asset_root,
                old_set_root,
                current_set_root,
                current_textures,
                resource_set,
                artifact,
            )?
        };
        set_artifact(&asset_root, artifact, &repaired)?;
    }

    let registered_mips = register_unowned_texture_mips(&asset_root, &published, &mut catalog)?;
    let registered_metadata = register_unowned_object_metadata(&asset_root, &mut catalog)?;
    refresh_existing_artifacts(&asset_root, &mut catalog)?;
    let encoded = serde_json::to_vec_pretty(&catalog).map_err(|error| error.to_string())?;
    let old_route_count = encoded
        .windows(b"map/objects/".len())
        .filter(|window| *window == b"map/objects/")
        .count();
    if old_route_count != 0 {
        return Err(format!(
            "repaired catalog still contains {old_route_count} legacy map/objects routes"
        ));
    }
    let mut encoded = encoded;
    encoded.push(b'\n');
    write_atomic(&catalog_path, &encoded)?;
    println!(
        "repaired {} resource sets, {} objects and {} directory routes; registered {} exact mip sidecars and {} object metadata files in {}",
        published.sets.len(),
        published.members.len(),
        directory_routes.len(),
        registered_mips,
        registered_metadata,
        catalog_path.display()
    );
    Ok(())
}

struct PublishedObjects {
    sets: BTreeMap<String, Value>,
    members: BTreeMap<String, Value>,
    textures: BTreeMap<String, Vec<Value>>,
}

fn load_published_objects(
    asset_root: &Path,
    object_root: &Path,
) -> Result<PublishedObjects, String> {
    let mut sets = BTreeMap::new();
    let mut members = BTreeMap::new();
    let mut textures = BTreeMap::new();
    for set_path in collect_named_files(object_root, "set.json")? {
        let document = read_json(&set_path)?;
        let id = required_string(&document, "id", "published resource set")?.to_owned();
        let route = asset_route(asset_root, &set_path)?;
        let set_artifact = artifact(asset_root, &route)?;
        if sets.insert(id.clone(), set_artifact).is_some() {
            return Err(format!("duplicate published resource set {id}"));
        }
        let set_textures = document
            .get("textures")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("published resource set {id} has no textures"))?
            .clone();
        textures.insert(id.clone(), set_textures);
        for member in document
            .get("members")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("published resource set {id} has no members"))?
        {
            let member_id = required_string(member, "id", "published object")?.to_owned();
            let definition = member
                .get("definition")
                .ok_or_else(|| format!("published object {member_id} has no definition"))?
                .clone();
            let path = artifact_path(&definition, "published object")?;
            let exact = artifact(asset_root, path)?;
            if definition != exact {
                return Err(format!(
                    "published object {member_id} definition metadata is stale"
                ));
            }
            if members.insert(member_id.clone(), definition).is_some() {
                return Err(format!("duplicate published object {member_id}"));
            }
        }
    }
    Ok(PublishedObjects {
        sets,
        members,
        textures,
    })
}

fn repair_member_artifact(
    asset_root: &Path,
    artifact: &mut Value,
    directory_routes: &BTreeMap<String, String>,
    label: &str,
) -> Result<(), String> {
    let old_path = artifact_path(artifact, "map geometry")?;
    let repaired = replace_directory(old_path, directory_routes).ok_or_else(|| {
        format!("geometry {label} has no published object route for {old_path:?}")
    })?;
    set_artifact(asset_root, artifact, &repaired)
}

fn replace_directory(path: &str, routes: &BTreeMap<String, String>) -> Option<String> {
    let parent = parent_route(path).ok()?;
    routes.get(&parent).map(|destination| {
        format!(
            "{destination}/{}",
            path.rsplit('/').next().unwrap_or_default()
        )
    })
}

fn repair_texture_route(
    asset_root: &Path,
    old_suffix: &str,
    current_set_root: &str,
    current_textures: &[Value],
    set_id: &str,
    old_artifact: &Value,
) -> Result<String, String> {
    let old_hash = required_string(old_artifact, "blake3", "legacy texture artifact")?;
    let texture_root = checked_join(asset_root, &format!("{current_set_root}/textures"))?;
    let mut hash_matches = Vec::new();
    for file in collect_all_files(&texture_root)? {
        let bytes =
            fs::read(&file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        if blake3::hash(&bytes).to_hex().as_str() == old_hash {
            hash_matches.push(asset_route(asset_root, &file)?);
        }
    }
    if hash_matches.len() == 1 {
        return Ok(hash_matches.remove(0));
    }
    if hash_matches.len() > 1 {
        let old_name = old_suffix.rsplit('/').next().unwrap_or_default();
        let mut name_matches = hash_matches
            .iter()
            .filter(|path| path.rsplit('/').next() == Some(old_name))
            .cloned()
            .collect::<Vec<_>>();
        if name_matches.len() == 1 {
            return Ok(name_matches.remove(0));
        }
        return Err(format!(
            "resource set {set_id} has ambiguous current files for legacy texture {old_suffix:?} ({old_hash})"
        ));
    }
    if current_textures.len() != 1 {
        return Err(format!(
            "resource set {set_id} has no current hash match for legacy texture {old_suffix:?} ({old_hash}) among {} textures",
            current_textures.len(),
        ));
    }
    let base = artifact_path(&current_textures[0], "published texture")?;
    if !base.starts_with(&format!("{current_set_root}/textures/")) {
        return Err(format!(
            "resource set {set_id} publishes texture outside its package: {base:?}"
        ));
    }
    if let Some((_, mip)) = old_suffix.split_once(".mips/") {
        let stem = base
            .strip_suffix(".png")
            .ok_or_else(|| format!("published texture is not PNG: {base:?}"))?;
        Ok(format!("{stem}.mips/{mip}"))
    } else {
        Ok(base.to_owned())
    }
}

fn refresh_existing_artifacts(asset_root: &Path, value: &mut Value) -> Result<(), String> {
    match value {
        Value::Array(values) => {
            for value in values {
                refresh_existing_artifacts(asset_root, value)?;
            }
        }
        Value::Object(values) => {
            if let Some(path) = values
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_owned)
            {
                let absolute = checked_join(asset_root, &path)?;
                if absolute.is_file() {
                    let bytes = fs::read(&absolute)
                        .map_err(|error| format!("cannot read {}: {error}", absolute.display()))?;
                    if values.contains_key("bytes") {
                        values.insert("bytes".to_owned(), Value::from(bytes.len() as u64));
                    }
                    if values.contains_key("blake3") {
                        values.insert(
                            "blake3".to_owned(),
                            Value::String(blake3::hash(&bytes).to_hex().to_string()),
                        );
                    }
                }
            }
            for value in values.values_mut() {
                refresh_existing_artifacts(asset_root, value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn register_unowned_texture_mips(
    asset_root: &Path,
    published: &PublishedObjects,
    catalog: &mut Value,
) -> Result<usize, String> {
    let mut referenced = BTreeMap::<String, ()>::new();
    collect_artifact_paths(catalog, &mut referenced);
    let mut additions = Vec::new();
    for textures in published.textures.values() {
        for texture in textures {
            let route = artifact_path(texture, "published texture")?;
            let base = checked_join(asset_root, route)?;
            let stem = base
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| format!("published texture has no UTF-8 stem: {route:?}"))?;
            let mip_root = base.with_file_name(format!("{stem}.mips"));
            if !mip_root.is_dir() {
                continue;
            }
            for mip in collect_all_files(&mip_root)? {
                let mip_route = asset_route(asset_root, &mip)?;
                if referenced.insert(mip_route.clone(), ()).is_none() {
                    additions.push(artifact(asset_root, &mip_route)?);
                }
            }
        }
    }
    additions.sort_by(|left, right| {
        artifact_path(left, "mip artifact")
            .unwrap_or_default()
            .cmp(artifact_path(right, "mip artifact").unwrap_or_default())
    });
    let count = additions.len();
    let shared = optional_array_mut(catalog, "sharedFiles")?;
    shared.extend(additions);
    shared.sort_by(|left, right| {
        artifact_path(left, "shared artifact")
            .unwrap_or_default()
            .cmp(artifact_path(right, "shared artifact").unwrap_or_default())
    });
    Ok(count)
}

fn register_unowned_object_metadata(
    asset_root: &Path,
    catalog: &mut Value,
) -> Result<usize, String> {
    let mut referenced = BTreeMap::<String, ()>::new();
    collect_artifact_paths(catalog, &mut referenced);
    let object_root = asset_root.join("objects");
    let mut additions = Vec::new();
    for entry in fs::read_dir(&object_root)
        .map_err(|error| format!("cannot enumerate {}: {error}", object_root.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        if kind.is_symlink() {
            return Err(format!(
                "object library metadata is a symlink: {}",
                path.display()
            ));
        }
        if !kind.is_file() {
            continue;
        }
        let route = asset_route(asset_root, &path)?;
        if referenced.insert(route.clone(), ()).is_none() {
            additions.push(artifact(asset_root, &route)?);
        }
    }
    additions.sort_by(|left, right| {
        artifact_path(left, "object metadata")
            .unwrap_or_default()
            .cmp(artifact_path(right, "object metadata").unwrap_or_default())
    });
    let count = additions.len();
    let shared = optional_array_mut(catalog, "sharedFiles")?;
    shared.extend(additions);
    shared.sort_by(|left, right| {
        artifact_path(left, "shared artifact")
            .unwrap_or_default()
            .cmp(artifact_path(right, "shared artifact").unwrap_or_default())
    });
    Ok(count)
}

fn collect_artifact_paths(value: &Value, paths: &mut BTreeMap<String, ()>) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_artifact_paths(value, paths);
            }
        }
        Value::Object(values) => {
            if let Some(path) = values.get("path").and_then(Value::as_str)
                && values.contains_key("bytes")
                && values.contains_key("blake3")
            {
                paths.insert(path.to_owned(), ());
            }
            for value in values.values() {
                collect_artifact_paths(value, paths);
            }
        }
        _ => {}
    }
}

fn artifact(asset_root: &Path, route: &str) -> Result<Value, String> {
    let path = checked_join(asset_root, route)?;
    let bytes = fs::read(&path)
        .map_err(|error| format!("cannot read published artifact {}: {error}", path.display()))?;
    Ok(serde_json::json!({
        "blake3": blake3::hash(&bytes).to_hex().to_string(),
        "bytes": bytes.len() as u64,
        "path": route,
    }))
}

fn set_artifact(asset_root: &Path, value: &mut Value, route: &str) -> Result<(), String> {
    *value = artifact(asset_root, route)?;
    Ok(())
}

fn artifact_path<'a>(value: &'a Value, label: &str) -> Result<&'a str, String> {
    value
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} artifact has no path"))
}

fn required_string<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{label} has no {key}"))
}

fn required_array_mut<'a>(value: &'a mut Value, key: &str) -> Result<&'a mut Vec<Value>, String> {
    value
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("map catalog has no {key} array"))
}

fn optional_array_mut<'a>(value: &'a mut Value, key: &str) -> Result<&'a mut Vec<Value>, String> {
    if value.get(key).is_none() {
        value
            .as_object_mut()
            .ok_or_else(|| "map catalog root is not an object".to_owned())?
            .insert(key.to_owned(), Value::Array(Vec::new()));
    }
    required_array_mut(value, key)
}

fn parent_route(route: &str) -> Result<String, String> {
    route
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_owned())
        .filter(|parent| !parent.is_empty())
        .ok_or_else(|| format!("asset route has no parent: {route:?}"))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid JSON {}: {error}", path.display()))
}

fn collect_named_files(root: &Path, name: &str) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
            if kind.is_symlink() {
                return Err(format!(
                    "object library contains a symlink: {}",
                    path.display()
                ));
            }
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() && entry.file_name() == name {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn collect_all_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
            if kind.is_symlink() {
                return Err(format!(
                    "object package contains a symlink: {}",
                    path.display()
                ));
            }
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn asset_route(asset_root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(asset_root)
        .map_err(|_| format!("asset escaped root: {}", path.display()))?;
    Ok(slash_path(relative))
}

fn checked_join(root: &Path, route: &str) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for component in Path::new(route).components() {
        match component {
            Component::Normal(value) => path.push(value),
            _ => return Err(format!("unsafe asset route {route:?}")),
        }
    }
    Ok(path)
}

fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
    if temporary.exists() {
        return Err(format!("stale temporary catalog: {}", temporary.display()));
    }
    fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("cannot replace {}: {error}", path.display()))
}
