use super::*;

pub(super) const REVISION_INDEX_SCHEMA: &str = "ffone.conversion-metadata-revision-index.v1";

pub(super) const RUNTIME_WORLD_PATH: &str = "_runtime/world.json";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RevisionIndex {
    pub(super) schema: String,
    pub(super) source_build: String,
    pub(super) plan_blake3: String,
    pub(super) files: Vec<ArchivedRuntimeMetadata>,
}

pub(super) fn repaired_tutorial_ownership_path(project_root: &Path, asset_root: &Path) -> Result<PathBuf> {
    let active = safe_join(asset_root, TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH)?;
    if active.is_file() {
        return Ok(active);
    }
    let revisions = project_root.join(format!(
        "../FusionForge/work/ffone/migration-archive/{}/conversion-metadata/revisions",
        TUTORIAL_STATIC_WORLD_SOURCE_BUILD
    ));
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
            "repaired tutorial ownership candidate",
        ) else {
            continue;
        };
        if ownership.winding_repair.is_none() {
            continue;
        }
        let matches_current = ownership.scenes.iter().all(|scene| {
            safe_join(asset_root, &scene.path)
                .ok()
                .and_then(|path| fs::read(path).ok())
                .is_some_and(|bytes| {
                    bytes.len() as u64 == scene.installed_bytes
                        && hash_bytes(&bytes) == scene.installed_blake3
                })
        });
        if matches_current {
            matches.push(candidate);
        }
    }
    if matches.len() != 1 {
        return invalid(format!(
            "expected exactly one repaired tutorial ownership matching current scenes, found {}",
            matches.len()
        ));
    }
    Ok(matches.remove(0))
}

pub(super) fn read_index_accessor(
    bytes: &[u8],
    parsed: &ParsedGlb,
    accessor_index: usize,
    path: &str,
) -> Result<Vec<u32>> {
    let layout = accessor_layout(parsed, accessor_index, path)?;
    if layout.element_components != 1 || layout.count % 3 != 0 {
        return invalid(format!(
            "{path:?} index accessor {accessor_index} is not triangle SCALAR"
        ));
    }
    let width = component_width(layout.component_type).ok_or_else(|| {
        invalid_error(format!(
            "{path:?} index accessor {accessor_index} has unsupported component type {}",
            layout.component_type
        ))
    })?;
    let mut values = Vec::with_capacity(layout.count);
    for index in 0..layout.count {
        let offset = layout.offset + index * layout.stride;
        values.push(match width {
            1 => bytes[offset] as u32,
            2 => u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("fixed slice"))
                as u32,
            4 => u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed slice")),
            _ => unreachable!(),
        });
    }
    Ok(values)
}

pub(super) fn reverse_index_accessor(
    bytes: &mut [u8],
    parsed: &ParsedGlb,
    accessor_index: usize,
    path: &str,
) -> Result<()> {
    let layout = accessor_layout(parsed, accessor_index, path)?;
    if layout.element_components != 1 || layout.count % 3 != 0 {
        return invalid(format!(
            "{path:?} index accessor {accessor_index} is not triangle SCALAR"
        ));
    }
    let width = component_width(layout.component_type).ok_or_else(|| {
        invalid_error(format!(
            "{path:?} index accessor {accessor_index} has unsupported component type {}",
            layout.component_type
        ))
    })?;
    for triangle in 0..(layout.count / 3) {
        let second = layout.offset + (triangle * 3 + 1) * layout.stride;
        let third = layout.offset + (triangle * 3 + 2) * layout.stride;
        for byte in 0..width {
            bytes.swap(second + byte, third + byte);
        }
    }
    Ok(())
}

pub(super) fn path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
