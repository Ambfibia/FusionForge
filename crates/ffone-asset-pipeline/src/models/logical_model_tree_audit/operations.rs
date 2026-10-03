use super::*;

pub(super) fn walk_files(
    root: &Path,
    violations: &mut Vec<LogicalModelTreeViolation>,
) -> Result<Vec<PathBuf>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| io_at(&directory, error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| io_at(&directory, error))?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries.into_iter().rev() {
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| io_at(&path, error))?;
            if file_type.is_symlink() {
                let relative = relative_string(root, &path)?;
                push_violation(
                    violations,
                    "symlink_forbidden",
                    &relative,
                    "candidate asset trees may not contain symbolic links",
                );
            } else if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

pub(super) fn recovery_sampler_times_match(keys: &DuplicateTrsKeys, times: &[f32]) -> bool {
    let key_times = match keys {
        DuplicateTrsKeys::Vec3(keys) => keys.iter().map(|key| key.time).collect::<Vec<_>>(),
        DuplicateTrsKeys::Quaternion(keys) => keys.iter().map(|key| key.time).collect::<Vec<_>>(),
    };
    key_times.len() == times.len()
        && key_times
            .iter()
            .zip(times)
            .all(|(source, wire)| (*source as f32).to_bits() == wire.to_bits())
}

pub(super) fn trs_source_rank(kind: EmptyTrsBindingKind, encoding: EmptyTrsSourceEncoding) -> Option<u8> {
    match (kind, encoding) {
        (EmptyTrsBindingKind::Translation, EmptyTrsSourceEncoding::Plain) => Some(0),
        (EmptyTrsBindingKind::Rotation, EmptyTrsSourceEncoding::Plain) => Some(1),
        (EmptyTrsBindingKind::Rotation, EmptyTrsSourceEncoding::Compressed) => Some(2),
        (EmptyTrsBindingKind::Scale, EmptyTrsSourceEncoding::Plain) => Some(3),
        (_, EmptyTrsSourceEncoding::Compressed) => None,
    }
}

pub(super) fn same_optional_f64_bits(left: Option<f64>, right: Option<f64>) -> bool {
    left.map(f64::to_bits) == right.map(f64::to_bits)
}

pub(super) fn typed_key_provenance_is_complete(
    canonical_indices: &[u32],
    duplicate_keys: &[DuplicateAnimationKey],
    source_key_count: u32,
) -> bool {
    if canonical_indices.is_empty() || source_key_count == 0 {
        return false;
    }
    let mut identities = BTreeSet::new();
    let canonical_ordered = canonical_indices
        .iter()
        .copied()
        .try_fold(None, |previous, index| {
            (index < source_key_count
                && previous.is_none_or(|previous| previous < index)
                && identities.insert(index))
            .then_some(Some(index))
        });
    if canonical_ordered.is_none() {
        return false;
    }
    let duplicate_ordered = duplicate_keys.iter().try_fold(None, |previous, duplicate| {
        (duplicate.source_key_index < source_key_count
            && previous.is_none_or(|previous| previous < duplicate.source_key_index)
            && identities.insert(duplicate.source_key_index)
            && (duplicate.canonical_key_index as usize) < canonical_indices.len())
        .then_some(Some(duplicate.source_key_index))
    });
    duplicate_ordered.is_some()
        && identities.len() == source_key_count as usize
        && (0..source_key_count).all(|index| identities.contains(&index))
}

pub(super) fn valid_coordinate_bounds(bounds: crate::logical_model_publish::CoordinateBounds) -> bool {
    bounds.min.into_iter().chain(bounds.max).all(f64::is_finite)
        && (0..3).all(|axis| bounds.min[axis] <= bounds.max[axis])
}

pub(super) fn json_f64_array<const N: usize>(value: Option<&Value>) -> Option<[f64; N]> {
    value?
        .as_array()?
        .iter()
        .map(Value::as_f64)
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()
}

pub(super) fn accumulate_file_counts(tree: &mut LogicalModelTreeCounts, file: &LogicalModelFileCounts) {
    tree.meshes += file.meshes;
    tree.materials += file.materials;
    tree.images += file.images;
    tree.textures += file.textures;
    tree.samplers += file.samplers;
    tree.skins += file.skins;
    tree.standard_animation_clips += file.standard_animation_clips;
    tree.metadata_only_animation_clips += file.metadata_only_animation_clips;
    tree.features.nodes += file.features.nodes;
    tree.features.mesh_parts += file.features.mesh_parts;
    tree.features.material_slots += file.features.material_slots;
    tree.features.skinned_meshes += file.features.skinned_meshes;
    tree.features.joints += file.features.joints;
    tree.features.inverse_bind_matrices += file.features.inverse_bind_matrices;
    tree.features.weighted_vertices += file.features.weighted_vertices;
    tree.features.animation_clips += file.features.animation_clips;
    tree.features.animation_channels += file.features.animation_channels;
    tree.features.empty_trs_bindings += file.features.empty_trs_bindings;
    tree.features.duplicate_trs_bindings += file.features.duplicate_trs_bindings;
    tree.features.duplicate_trs_keyframes += file.features.duplicate_trs_keyframes;
    tree.features.duplicate_same_time_keys += file.features.duplicate_same_time_keys;
    tree.features.animation_time_recoveries += file.features.animation_time_recoveries;
    tree.features.recovered_animation_keyframes += file.features.recovered_animation_keyframes;
    tree.features.animation_curve_recoveries += file.features.animation_curve_recoveries;
    tree.features.rejected_conflicting_trs_bindings +=
        file.features.rejected_conflicting_trs_bindings;
    tree.features.rejected_conflicting_trs_keyframes +=
        file.features.rejected_conflicting_trs_keyframes;
    tree.features.animation_keyframes += file.features.animation_keyframes;
    tree.features.cubic_spline_keyframes += file.features.cubic_spline_keyframes;
    tree.features.animation_events += file.features.animation_events;
    tree.features.animation_event_null_object_pointers +=
        file.features.animation_event_null_object_pointers;
    tree.features.object_reference_keys += file.features.object_reference_keys;
    tree.features.colliders += file.features.colliders;
    tree.features.lod_levels += file.features.lod_levels;
}

pub(super) fn aliased_field<'a>(value: &'a Value, camel: &str, snake: &str) -> Option<&'a Value> {
    value.get(camel).or_else(|| value.get(snake))
}

pub(super) fn feature_count_fields(
    counts: &LogicalModelFeatureCounts,
) -> [(&'static str, &'static str, u64); 25] {
    [
        ("nodes", "nodes", counts.nodes),
        ("meshParts", "mesh_parts", counts.mesh_parts),
        ("materialSlots", "material_slots", counts.material_slots),
        ("skinnedMeshes", "skinned_meshes", counts.skinned_meshes),
        ("joints", "joints", counts.joints),
        (
            "inverseBindMatrices",
            "inverse_bind_matrices",
            counts.inverse_bind_matrices,
        ),
        (
            "weightedVertices",
            "weighted_vertices",
            counts.weighted_vertices,
        ),
        ("animationClips", "animation_clips", counts.animation_clips),
        (
            "animationChannels",
            "animation_channels",
            counts.animation_channels,
        ),
        (
            "emptyTrsBindings",
            "empty_trs_bindings",
            counts.empty_trs_bindings,
        ),
        (
            "duplicateTrsBindings",
            "duplicate_trs_bindings",
            counts.duplicate_trs_bindings,
        ),
        (
            "duplicateTrsKeyframes",
            "duplicate_trs_keyframes",
            counts.duplicate_trs_keyframes,
        ),
        (
            "duplicateSameTimeKeys",
            "duplicate_same_time_keys",
            counts.duplicate_same_time_keys,
        ),
        (
            "animationTimeRecoveries",
            "animation_time_recoveries",
            counts.animation_time_recoveries,
        ),
        (
            "recoveredAnimationKeyframes",
            "recovered_animation_keyframes",
            counts.recovered_animation_keyframes,
        ),
        (
            "animationCurveRecoveries",
            "animation_curve_recoveries",
            counts.animation_curve_recoveries,
        ),
        (
            "rejectedConflictingTrsBindings",
            "rejected_conflicting_trs_bindings",
            counts.rejected_conflicting_trs_bindings,
        ),
        (
            "rejectedConflictingTrsKeyframes",
            "rejected_conflicting_trs_keyframes",
            counts.rejected_conflicting_trs_keyframes,
        ),
        (
            "animationKeyframes",
            "animation_keyframes",
            counts.animation_keyframes,
        ),
        (
            "cubicSplineKeyframes",
            "cubic_spline_keyframes",
            counts.cubic_spline_keyframes,
        ),
        (
            "animationEvents",
            "animation_events",
            counts.animation_events,
        ),
        (
            "animationEventNullObjectPointers",
            "animation_event_null_object_pointers",
            counts.animation_event_null_object_pointers,
        ),
        (
            "objectReferenceKeys",
            "object_reference_keys",
            counts.object_reference_keys,
        ),
        ("colliders", "colliders", counts.colliders),
        ("lodLevels", "lod_levels", counts.lod_levels),
    ]
}

pub(super) fn attribute_accessor<'a>(
    attributes: Option<&serde_json::Map<String, Value>>,
    name: &str,
    accessors: &'a [Option<AccessorInfo>],
) -> Option<&'a AccessorInfo> {
    attributes
        .and_then(|values| values.get(name))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .and_then(|index| accessors.get(index))
        .and_then(Option::as_ref)
}

pub(super) fn safe_image_relative(glb_relative: &str, uri: &str) -> Option<String> {
    if uri.is_empty()
        || uri.contains(['\\', ':', '?', '#', '%'])
        || Path::new(uri).extension().and_then(|value| value.to_str()) != Some("png")
        || Path::new(uri)
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::ParentDir))
    {
        return None;
    }
    let parent = Path::new(glb_relative)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let mut resolved = PathBuf::new();
    for part in parent.join(uri).components() {
        match part {
            Component::Normal(name) => resolved.push(name),
            Component::ParentDir if resolved.pop() => (),
            _ => return None,
        }
    }
    Some(slash_path(&resolved))
}

pub(super) fn report_sidecar(glb_relative: &str) -> String {
    let mut path = PathBuf::from(glb_relative);
    path.set_extension("publish.json");
    slash_path(&path)
}

pub(super) fn array_or_empty<'a>(document: &'a Value, key: &str) -> &'a [Value] {
    document
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub(super) fn accessor_component_count(kind: &str) -> Option<usize> {
    match kind {
        "SCALAR" => Some(1),
        "VEC2" => Some(2),
        "VEC3" => Some(3),
        "VEC4" | "MAT2" => Some(4),
        "MAT3" => Some(9),
        "MAT4" => Some(16),
        _ => None,
    }
}

pub(super) fn component_size(component_type: u32) -> Option<usize> {
    match component_type {
        5_120 | 5_121 => Some(1),
        5_122 | 5_123 => Some(2),
        5_125 | 5_126 => Some(4),
        _ => None,
    }
}

pub(super) fn json_usize(value: Option<&Value>) -> Option<usize> {
    value
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

pub(super) fn json_usize_default(value: Option<&Value>, default: usize) -> Option<usize> {
    match value {
        Some(value) => value.as_u64().and_then(|value| usize::try_from(value).ok()),
        None => Some(default),
    }
}

pub(super) fn relative_string(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        PipelineError::ModelAudit(format!(
            "candidate path {:?} escapes output root {:?}",
            path, root
        ))
    })?;
    Ok(slash_path(relative))
}

pub(super) fn push_violation(
    violations: &mut Vec<LogicalModelTreeViolation>,
    code: &str,
    path: &str,
    message: impl Into<String>,
) {
    violations.push(LogicalModelTreeViolation {
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.into(),
    });
}

pub(super) fn to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
