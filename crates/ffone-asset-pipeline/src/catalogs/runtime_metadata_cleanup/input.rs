use super::*;

pub(super) fn collect_proven_conversion_payloads(
    asset_root: &Path,
) -> Result<BTreeMap<String, PayloadIdentity>> {
    let world_root = asset_root.join("world");
    let mut files = BTreeMap::new();
    if world_root.is_dir() {
        collect_proven_conversion_payloads_at(asset_root, &world_root, &mut files)?;
    }
    Ok(files)
}

pub(super) fn collect_proven_conversion_payloads_at(
    asset_root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, PayloadIdentity>,
) -> Result<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| io_at(directory, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(directory, error))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| io_at(&path, error))?;
        if metadata.file_type().is_symlink() {
            return invalid(format!(
                "proven conversion payload scan refuses symlink {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_proven_conversion_payloads_at(asset_root, &path, files)?;
            continue;
        }
        if !metadata.is_file() {
            return invalid(format!(
                "unsupported entry below world assets: {}",
                path.display()
            ));
        }
        let relative = normalized_asset_path(asset_root, &path)?;
        if classify_proven_conversion_payload(&relative).is_none() {
            continue;
        }
        let identity = PayloadIdentity {
            bytes: metadata.len(),
            blake3: hash_file(&path)?,
        };
        if files.insert(relative.clone(), identity).is_some() {
            return invalid(format!(
                "duplicate physical proven conversion payload {relative:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn resolve_json_reference_targets(document_path: &str, reference: &str) -> Vec<String> {
    let normalized = reference.replace('\\', "/");
    let normalized = normalized
        .strip_prefix("assets/game/")
        .unwrap_or(&normalized);
    let mut targets = Vec::new();
    if let Some(direct) = normalize_direct_asset_reference(normalized) {
        targets.push(direct);
    }
    if let Some(relative) = resolve_json_relative_reference(document_path, normalized)
        && !targets.contains(&relative)
    {
        targets.push(relative);
    }
    targets
}

pub(super) fn resolve_json_relative_reference(document_path: &str, reference: &str) -> Option<String> {
    if reference.is_empty() || reference.starts_with('/') {
        return None;
    }
    let parent = document_path
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent);
    let mut components = parent
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    for component in reference.split('/') {
        match component {
            "" => return None,
            "." => {}
            ".." => {
                components.pop()?;
            }
            component => components.push(component),
        }
    }
    Some(components.join("/"))
}

pub(super) fn discover_unregistered_world_roots(
    asset_root: &Path,
    registry: &RuntimeWorldRegistry,
) -> Result<BTreeSet<String>> {
    let maps_root = asset_root.join("world/maps");
    let mut discovered = BTreeSet::new();
    if !maps_root.is_dir() {
        return Ok(discovered);
    }
    let mut entries = fs::read_dir(&maps_root)
        .map_err(|error| io_at(&maps_root, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(&maps_root, error))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| io_at(&path, error))?;
        if metadata.file_type().is_symlink() {
            return invalid(format!(
                "symlink is forbidden below world/maps: {}",
                path.display()
            ));
        }
        if !metadata.is_dir() {
            continue;
        }
        let id = entry
            .file_name()
            .to_str()
            .ok_or_else(|| invalid_error("world map directory is not valid UTF-8"))?
            .to_owned();
        let root = format!("world/maps/{id}");
        if path.join("terrain/terrain.json").is_file()
            && !path.join("scene.json").exists()
            && !runtime_world_references_root(registry, &id, &root)
        {
            discovered.insert(root);
        }
    }
    Ok(discovered)
}

pub(super) fn collect_json_paths(asset_root: &Path) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    for top in [
        "audio",
        "characters",
        "data",
        "icons",
        "localization",
        "tutorial",
        "ui",
        "world",
    ] {
        let root = asset_root.join(top);
        if root.is_dir() {
            collect_json_paths_from(asset_root, &root, &mut paths)?;
        }
    }
    paths.sort();
    Ok(paths)
}

pub(super) fn collect_json_paths_from(
    asset_root: &Path,
    directory: &Path,
    output: &mut Vec<String>,
) -> Result<()> {
    let entries = fs::read_dir(directory)
        .map_err(|error| io_at(directory, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(directory, error))?;
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| io_at(entry.path(), error))?;
        if file_type.is_symlink() {
            return invalid(format!(
                "metadata cleanup refuses to traverse symlink {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            collect_json_paths_from(asset_root, &entry.path(), output)?;
        } else if file_type.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            let entry_path = entry.path();
            let relative = entry_path.strip_prefix(asset_root).map_err(|_| {
                invalid_error("collected metadata escaped the canonical asset root")
            })?;
            let normalized = relative
                .components()
                .map(|component| match component {
                    Component::Normal(value) => value
                        .to_str()
                        .ok_or_else(|| invalid_error("metadata path is not UTF-8")),
                    _ => Err(invalid_error("metadata path is not a normal relative path")),
                })
                .collect::<Result<Vec<_>>>()?
                .join("/");
            output.push(normalized);
        }
    }
    Ok(())
}

pub(super) fn collect_regular_paths_from(
    archive_root: &Path,
    directory: &Path,
    output: &mut Vec<String>,
) -> Result<()> {
    let entries = fs::read_dir(directory)
        .map_err(|error| io_at(directory, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(directory, error))?;
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| io_at(entry.path(), error))?;
        if file_type.is_symlink() {
            return invalid(format!(
                "conversion-metadata archive refuses symlink {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            collect_regular_paths_from(archive_root, &entry.path(), output)?;
        } else if file_type.is_file() {
            let entry_path = entry.path();
            let relative = entry_path.strip_prefix(archive_root).map_err(|_| {
                invalid_error("archived payload escaped its immutable archive root")
            })?;
            let normalized = relative
                .components()
                .map(|component| match component {
                    Component::Normal(value) => value
                        .to_str()
                        .ok_or_else(|| invalid_error("archived payload path is not UTF-8")),
                    _ => Err(invalid_error(
                        "archived payload path is not a normal relative path",
                    )),
                })
                .collect::<Result<Vec<_>>>()?
                .join("/");
            output.push(normalized);
        } else {
            return invalid(format!(
                "conversion-metadata archive contains unsupported entry {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}
