use super::*;

pub(super) fn load_world_map_scope(asset_root: &Path) -> Result<ScopeWork> {
    let target = safe_join(asset_root, WORLD_MAP_STATIC_WORLD_OWNERSHIP_PATH)?;
    let ownership = read_json::<TutorialStaticWorldOwnership>(&target, "world-map ownership")?;
    Ok(ScopeWork {
        label: "worldMap",
        ownership_source: target.clone(),
        ownership_target: target,
        ownership,
        file_start: 0,
        file_end: 0,
        scene_hashes: BTreeMap::new(),
        source_set_blake3: String::new(),
        result_set_blake3: String::new(),
        converted_files: 0,
        unchanged_files: 0,
    })
}

pub(super) fn load_tutorial_scope(project_root: &Path, asset_root: &Path) -> Result<ScopeWork> {
    let target = safe_join(asset_root, TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH)?;
    let source = if target.is_file() {
        target.clone()
    } else {
        find_matching_tutorial_archive(project_root, asset_root)?
    };
    let ownership = read_json::<TutorialStaticWorldOwnership>(&source, "tutorial ownership")?;
    Ok(ScopeWork {
        label: "tutorial",
        ownership_source: source,
        ownership_target: target,
        ownership,
        file_start: 0,
        file_end: 0,
        scene_hashes: BTreeMap::new(),
        source_set_blake3: String::new(),
        result_set_blake3: String::new(),
        converted_files: 0,
        unchanged_files: 0,
    })
}

pub(super) fn find_matching_tutorial_archive(project_root: &Path, asset_root: &Path) -> Result<PathBuf> {
    let revisions = project_root.join(format!(
        "../FusionForge/work/ffone/migration-archive/{}/conversion-metadata/revisions",
        TUTORIAL_STATIC_WORLD_SOURCE_BUILD
    ));
    let metadata = fs::symlink_metadata(&revisions).map_err(|error| io_at(&revisions, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return invalid("tutorial conversion-metadata revisions root is invalid");
    }
    let mut matches = Vec::new();
    for entry in fs::read_dir(&revisions).map_err(|error| io_at(&revisions, error))? {
        let entry = entry.map_err(|error| io_at(&revisions, error))?;
        let candidate = entry
            .path()
            .join("files")
            .join(TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH);
        if !candidate.is_file() {
            continue;
        }
        let Ok(ownership) = read_json::<TutorialStaticWorldOwnership>(
            &candidate,
            "archived tutorial ownership candidate",
        ) else {
            continue;
        };
        if ownership.source_build != TUTORIAL_STATIC_WORLD_SOURCE_BUILD
            || ownership.winding_repair.is_some()
        {
            continue;
        }
        // Tutorial scenes were sanitized after their static publication, so
        // their installed scene hashes intentionally differ from the archived
        // ownership proof. The owned GLBs themselves are immutable runtime
        // inputs and uniquely identify the revision that produced the current
        // model tree.
        let visual_entries = ownership
            .owned_files
            .iter()
            .filter(|entry| is_visual_glb(&entry.path, "tutorial"))
            .collect::<Vec<_>>();
        let matches_current = !visual_entries.is_empty()
            && visual_entries.iter().all(|entry| {
                safe_join(asset_root, &entry.path)
                    .ok()
                    .and_then(|path| fs::read(path).ok())
                    .is_some_and(|bytes| {
                        bytes.len() as u64 == entry.bytes && hash_bytes(&bytes) == entry.blake3
                    })
            });
        if matches_current {
            matches.push(candidate);
        }
    }
    if matches.len() != 1 {
        return invalid(format!(
            "expected exactly one archived tutorial ownership matching current scenes, found {}",
            matches.len()
        ));
    }
    Ok(matches.remove(0))
}

pub(super) fn read_vec3_accessor(
    bytes: &[u8],
    parsed: &ParsedGlb,
    accessor_index: usize,
    path: &str,
) -> Result<Vec<[f32; 3]>> {
    let layout = accessor_layout(parsed, accessor_index, path)?;
    if layout.component_type != GLTF_FLOAT || layout.element_components != 3 {
        return invalid(format!(
            "{path:?} accessor {accessor_index} is not FLOAT VEC3"
        ));
    }
    let mut values = Vec::with_capacity(layout.count);
    for index in 0..layout.count {
        let offset = layout.offset + index * layout.stride;
        values.push([
            f32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed slice")),
            f32::from_le_bytes(
                bytes[offset + 4..offset + 8]
                    .try_into()
                    .expect("fixed slice"),
            ),
            f32::from_le_bytes(
                bytes[offset + 8..offset + 12]
                    .try_into()
                    .expect("fixed slice"),
            ),
        ]);
    }
    Ok(values)
}

pub(super) fn read_regular_file(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return invalid(format!(
            "{label} must be a regular file: {}",
            path.display()
        ));
    }
    fs::read(path).map_err(|error| io_at(path, error))
}

pub(super) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, label: &str) -> Result<T> {
    let bytes = read_regular_file(path, label)?;
    parse_json(&bytes, path)
}

pub(super) fn parse_json<T: for<'de> Deserialize<'de>>(bytes: &[u8], path: &Path) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })
}
