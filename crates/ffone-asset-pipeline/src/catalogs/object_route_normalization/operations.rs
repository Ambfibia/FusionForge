use super::*;

pub(super) fn build_plan(
    asset_root: &Path,
    source_root: &Path,
    catalog: &JsonValue,
) -> Result<NormalizationPlan> {
    let mut entries = catalog
        .get("resourceSets")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error("map catalog has no resourceSets array"))?
        .iter()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| {
        entry
            .pointer("/definition/path")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
    });
    let mut used_packages = BTreeMap::<String, BTreeSet<String>>::new();
    let mut file_routes = BTreeMap::new();
    let mut package_routes = Vec::with_capacity(entries.len());
    let mut members = 0usize;
    let mut textures = 0usize;
    let mut renamed_members = 0usize;

    for entry in entries {
        let resource_set_id = required_string(entry, "id", "resource set id")?;
        let category = compact_slug(
            required_string(entry, "category", "resource set category")?,
            "unclassified",
        );
        let name = required_string(entry, "name", "resource set name")?;
        let definition = entry
            .pointer("/definition/path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error("resource set has no definition path"))?;
        let source_package = definition
            .strip_suffix("/set.json")
            .ok_or_else(|| invalid_error(format!("invalid resource-set route {definition:?}")))?;
        if !source_package.starts_with("map/objects/") {
            return invalid(format!(
                "resource set is outside legacy map/objects root: {source_package}"
            ));
        }
        let source_package_path = checked_join(asset_root, source_package)?;
        if !source_package_path.is_dir() || !source_package_path.starts_with(source_root) {
            return invalid(format!("resource-set package is absent: {source_package}"));
        }
        let package_name = allocate_component(
            &compact_slug(name, "object_set"),
            used_packages.entry(category.clone()).or_default(),
        );
        let destination_package = format!("objects/{category}/{package_name}");
        let set_bytes = read_file(&source_package_path.join("set.json"), "resource set")?;
        let set: JsonValue =
            serde_json::from_slice(&set_bytes).map_err(|source| PipelineError::Json {
                path: definition.to_owned(),
                source,
            })?;
        let set_members = set
            .get("members")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| invalid_error("resource set has no members array"))?;
        let set_textures = set
            .get("textures")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| invalid_error("resource set has no textures array"))?;
        insert_route(
            &mut file_routes,
            definition.to_owned(),
            format!("{destination_package}/set.json"),
        )?;

        let mut used_textures = BTreeSet::new();
        for texture in set_textures {
            let texture_path = texture
                .get("path")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| invalid_error("resource-set texture has no path"))?;
            let texture_source = checked_join(asset_root, texture_path)?;
            let texture_extension = texture_source
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("png")
                .to_ascii_lowercase();
            let source_stem = texture_source
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| invalid_error("resource-set texture has no UTF-8 stem"))?;
            let texture_stem = if set_textures.len() == 1 {
                "texture".to_owned()
            } else {
                allocate_component(&compact_slug(source_stem, "texture"), &mut used_textures)
            };
            insert_route(
                &mut file_routes,
                texture_path.to_owned(),
                format!("{destination_package}/textures/{texture_stem}.{texture_extension}"),
            )?;
            let mip_root = texture_source.with_file_name(format!("{source_stem}.mips"));
            if mip_root.is_dir() {
                for mip in collect_files(&mip_root)? {
                    let relative = mip
                        .strip_prefix(&mip_root)
                        .map_err(|_| invalid_error("texture mip escaped its source directory"))?;
                    insert_route(
                        &mut file_routes,
                        asset_relative(asset_root, &mip)?,
                        format!(
                            "{destination_package}/textures/{texture_stem}.mips/{}",
                            slash_path(relative)
                        ),
                    )?;
                }
            }
            textures += 1;
        }

        let mut used_members = BTreeSet::new();
        for member in set_members {
            let member_name = required_string(member, "name", "resource-set member name")?;
            let definition_path = member
                .pointer("/definition/path")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| invalid_error("resource-set member has no definition path"))?;
            let source_member = Path::new(definition_path)
                .parent()
                .ok_or_else(|| invalid_error("member definition has no parent"))?;
            let old_leaf = source_member
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| invalid_error("member directory has no UTF-8 name"))?;
            let member_leaf =
                allocate_component(&compact_slug(member_name, "object"), &mut used_members);
            if member_leaf != old_leaf {
                renamed_members += 1;
            }
            let destination_member = format!("{destination_package}/models/{member_leaf}");
            let source_member_path = checked_join(asset_root, &slash_path(source_member))?;
            for file in collect_files(&source_member_path)? {
                let within = file
                    .strip_prefix(&source_member_path)
                    .map_err(|_| invalid_error("resource-set member file escaped its package"))?;
                insert_route(
                    &mut file_routes,
                    asset_relative(asset_root, &file)?,
                    format!("{destination_member}/{}", slash_path(within)),
                )?;
            }
            members += 1;
        }

        let actual = collect_files(&source_package_path)?
            .into_iter()
            .map(|path| asset_relative(asset_root, &path))
            .collect::<Result<BTreeSet<_>>>()?;
        let planned = file_routes
            .keys()
            .filter(|path| path.starts_with(&format!("{source_package}/")))
            .cloned()
            .collect::<BTreeSet<_>>();
        if actual != planned {
            let missing = actual
                .difference(&planned)
                .take(5)
                .cloned()
                .collect::<Vec<_>>();
            let extra = planned
                .difference(&actual)
                .take(5)
                .cloned()
                .collect::<Vec<_>>();
            return invalid(format!(
                "resource-set route closure mismatch for {resource_set_id}: unplanned={missing:?}, absent={extra:?}"
            ));
        }
        package_routes.push(ObjectPackageRoute {
            resource_set_id: resource_set_id.to_owned(),
            category,
            source: source_package.to_owned(),
            destination: destination_package,
        });
    }
    let destinations = file_routes.values().collect::<BTreeSet<_>>();
    if destinations.len() != file_routes.len() {
        return invalid("object route plan contains duplicate destinations");
    }
    Ok(NormalizationPlan {
        file_routes,
        package_routes,
        members,
        textures,
        renamed_members,
    })
}

pub(super) fn stage_map_json(
    asset_root: &Path,
    map_root: &Path,
    object_stage: &Path,
    map_stage: &Path,
    catalog: &JsonValue,
    routes: &BTreeMap<String, String>,
) -> Result<Vec<String>> {
    fs::create_dir(map_stage).map_err(|error| io_at(map_stage, error))?;
    let tiles = catalog
        .get("tiles")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error("map catalog has no tiles array"))?;
    let mut files = Vec::with_capacity(tiles.len() * 2 + 1);
    for tile in tiles {
        let tile_id = required_string(tile, "tileId", "map tile id")?;
        let relative = format!("map/tiles/{tile_id}/scene.json");
        let source = checked_join(asset_root, &relative)?;
        let mut value = read_json(&source, "map tile scene")?;
        replace_paths(&mut value, routes);
        refresh_artifacts(&mut value, asset_root, object_stage, map_stage)?;
        write_new(&checked_join(map_stage, &relative)?, &pretty_json(&value)?)?;
        files.push(relative);
    }
    for tile in tiles {
        let tile_id = required_string(tile, "tileId", "map tile id")?;
        let relative = format!("map/tiles/{tile_id}/tile.json");
        let source = checked_join(asset_root, &relative)?;
        let mut value = read_json(&source, "map tile manifest")?;
        replace_paths(&mut value, routes);
        refresh_artifacts(&mut value, asset_root, object_stage, map_stage)?;
        write_new(&checked_join(map_stage, &relative)?, &pretty_json(&value)?)?;
        files.push(relative);
    }
    let mut updated_catalog = catalog.clone();
    replace_paths(&mut updated_catalog, routes);
    refresh_artifacts(&mut updated_catalog, asset_root, object_stage, map_stage)?;
    let catalog_relative = "map/catalog.json".to_owned();
    write_new(
        &checked_join(map_stage, &catalog_relative)?,
        &pretty_json(&updated_catalog)?,
    )?;
    files.push(catalog_relative);
    files.sort();
    files.dedup();
    if map_root != asset_root.join("map") {
        return invalid("map root changed during object route normalization");
    }
    Ok(files)
}

pub(super) fn rollback(
    asset_root: &Path,
    source_root: &Path,
    destination_root: &Path,
    object_backup: &Path,
    map_backup: &Path,
    map_files: &[String],
) {
    for relative in map_files {
        let backup = checked_join(map_backup, relative);
        let live = checked_join(asset_root, relative);
        if let (Ok(backup), Ok(live)) = (backup, live)
            && let Ok(bytes) = fs::read(backup)
        {
            let _ = fs::write(live, bytes);
        }
    }
    if destination_root.is_dir() {
        let _ = fs::remove_dir_all(destination_root);
    }
    if object_backup.is_dir() && !source_root.exists() {
        let _ = fs::rename(object_backup, source_root);
    }
    let _ = fs::remove_dir_all(map_backup);
}

pub(super) fn replace_paths(value: &mut JsonValue, routes: &BTreeMap<String, String>) {
    match value {
        JsonValue::String(text) => {
            if let Some(replacement) = routes.get(text) {
                *text = replacement.clone();
            }
        }
        JsonValue::Array(values) => {
            for value in values {
                replace_paths(value, routes);
            }
        }
        JsonValue::Object(values) => {
            for value in values.values_mut() {
                replace_paths(value, routes);
            }
        }
        _ => {}
    }
}

pub(super) fn compact_slug(value: &str, fallback: &str) -> String {
    let slug = safe_slug(value, fallback);
    let mut tokens = slug
        .split('_')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if let Some(index) = tokens.iter().position(|token| *token == "dds") {
        tokens.truncate(index);
    }
    if tokens.len() >= 5 {
        let first = (tokens[0], tokens[1]);
        if let Some(index) = (2..tokens.len().saturating_sub(1))
            .find(|index| (tokens[*index], tokens[*index + 1]) == first)
        {
            tokens.truncate(index);
        }
    }
    let joined = if tokens.is_empty() {
        safe_slug(fallback, "object")
    } else {
        tokens.join("_")
    };
    truncate_component(&joined, MAX_COMPONENT_LEN)
}

pub(super) fn allocate_component(candidate: &str, used: &mut BTreeSet<String>) -> String {
    let base = truncate_component(candidate, MAX_COMPONENT_LEN);
    if used.insert(base.clone()) {
        return base;
    }
    for variant in 2u64.. {
        let suffix = format!("_variant_{variant:04}");
        let prefix = truncate_component(&base, MAX_COMPONENT_LEN - suffix.len());
        let name = format!("{prefix}{suffix}");
        if used.insert(name.clone()) {
            return name;
        }
    }
    unreachable!()
}

pub(super) fn truncate_component(value: &str, maximum: usize) -> String {
    if value.len() <= maximum {
        return value.to_owned();
    }
    let mut end = maximum;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    let prefix = &value[..end];
    prefix
        .rsplit_once('_')
        .filter(|(head, _)| head.len() >= maximum / 2)
        .map_or(prefix, |(head, _)| head)
        .trim_end_matches('_')
        .to_owned()
}

pub(super) fn safe_slug(value: &str, fallback: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            result.push(character);
            separator = false;
        } else if !separator && !result.is_empty() {
            result.push('_');
            separator = true;
        }
    }
    while result.ends_with('_') {
        result.pop();
    }
    if result.is_empty() {
        fallback.to_owned()
    } else {
        result
    }
}

pub(super) fn required_string<'a>(value: &'a JsonValue, key: &str, label: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_error(format!("{label} is absent")))
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let path = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    if !path.is_dir() {
        return invalid(format!("{label} is not a directory: {}", path.display()));
    }
    Ok(path)
}

pub(super) fn checked_join(root: &Path, relative: impl AsRef<Path>) -> Result<PathBuf> {
    let mut output = root.to_path_buf();
    for component in relative.as_ref().components() {
        match component {
            Component::Normal(value) => output.push(value),
            Component::CurDir => {}
            _ => {
                return invalid(format!(
                    "unsafe relative asset path: {:?}",
                    relative.as_ref()
                ));
            }
        }
    }
    Ok(output)
}

pub(super) fn create_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("output file has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))
}

pub(super) fn pretty_json(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(generated_json_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn normal_components(path: &Path) -> Result<Vec<String>> {
    path.components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid_error("asset route is not UTF-8")),
            Component::CurDir => Ok(".".to_owned()),
            _ => invalid("asset route contains an unsafe component"),
        })
        .filter(|value| !matches!(value, Ok(part) if part == "."))
        .collect()
}

pub(super) fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
