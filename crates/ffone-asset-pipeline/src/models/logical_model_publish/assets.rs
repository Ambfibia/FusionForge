use super::*;

pub(super) fn normalized_hierarchy_path_component(name: &str) -> &str {
    // The legacy preview/export path grammar trims each slash-delimited
    // component, while `name` remains the exact Unity Transform m_Name.
    // Keeping those roles separate preserves authored names such as
    // `Bip01 ` in glTF without pretending that a normalized lookup path is
    // the display name. Duplicate normalized paths are still rejected above.
    name.trim()
}

pub(super) fn source_track_path(track: SourceRecoveryTrackRef<'_>) -> &str {
    match track {
        SourceRecoveryTrackRef::Vec3(track) => &track.path,
        SourceRecoveryTrackRef::Quaternion(track) => &track.path,
    }
}

pub(super) fn source_track_index(track: SourceRecoveryTrackRef<'_>) -> usize {
    match track {
        SourceRecoveryTrackRef::Vec3(track) => track.source_index,
        SourceRecoveryTrackRef::Quaternion(track) => track.source_index,
    }
}

pub(super) fn exact_binding_node_index(
    source: &SourceDocument,
    binding: &SourceBinding,
    context: &str,
) -> Result<u32> {
    let matches = source
        .model_hierarchy
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            (node.path == binding.transform_path
                && node.source_asset_index == binding.transform_asset_index
                && node.transform_path_id == binding.transform_path_id)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [index] => u32_index(*index, "hierarchy node count"),
        [] => invalid(format!(
            "{context} path {:?} has no exact Transform identity {}:{} in the hierarchy",
            binding.transform_path, binding.transform_asset_index, binding.transform_path_id
        )),
        _ => invalid(format!(
            "{context} path {:?} repeats exact Transform identity {}:{} in the hierarchy",
            binding.transform_path, binding.transform_asset_index, binding.transform_path_id
        )),
    }
}

pub(super) fn exact_node_path(paths: &BTreeMap<String, u32>, path: &str, context: &str) -> Result<u32> {
    paths
        .get(path)
        .copied()
        .ok_or_else(|| invalid_error(format!("{context} path {path:?} is missing from hierarchy")))
}

pub(super) fn suffix_node_path(paths: &BTreeMap<String, u32>, path: &str, context: &str) -> Result<u32> {
    if path.is_empty() || path.contains('\\') || path.trim_matches('/') != path {
        return invalid(format!("{context} path {path:?} is invalid"));
    }
    let suffix = format!("/{path}");
    let mut matches = paths
        .iter()
        .filter_map(|(candidate, index)| {
            (candidate == path || candidate.ends_with(&suffix))
                .then_some((candidate.matches('/').count(), *index))
        })
        .collect::<Vec<_>>();
    // Skeleton and skin paths are relative to their renderer/root space. If
    // the true logical-model root repeats the skeleton root's name (Sneaky
    // Spawn), both name and root/name are suffix matches. The deepest
    // match is the authored skeleton node; its serialized Transform identity
    // is subsequently checked by validate_skeleton, so this remains strict.
    let deepest = matches.iter().map(|(depth, _)| *depth).max();
    if let Some(deepest) = deepest {
        matches.retain(|(depth, _)| *depth == deepest);
    }
    match matches.as_slice() {
        [(_, index)] => Ok(*index),
        [] => invalid(format!(
            "{context} path {path:?} has no suffix match under the true root"
        )),
        _ => invalid(format!(
            "{context} path {path:?} is ambiguous under the true root"
        )),
    }
}

pub(super) fn normalized_route(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_matches('/')
        .to_ascii_lowercase()
}

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn u32_index(value: usize, label: &str) -> Result<u32> {
    value
        .try_into()
        .map_err(|_| invalid_error(format!("{label} exceeds u32")))
}
