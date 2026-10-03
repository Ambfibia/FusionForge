use super::*;

pub(super) fn quaternion(value: Option<&UnityValue>) -> Option<(f64, f64, f64, f64)> {
    let value = value?.as_object()?;
    Some((
        value.get("x")?.as_f64()?,
        value.get("y")?.as_f64()?,
        value.get("z")?.as_f64()?,
        value.get("w")?.as_f64()?,
    ))
}

pub(super) fn append_side_errors(map: &str, code: &str, side: &JsonValue, errors: &mut Vec<WorldAuditError>) {
    for detail in side
        .get("errors")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .filter(|detail| !detail.is_empty())
    {
        errors.push(WorldAuditError::new(Some(map), code, detail));
    }
}

pub(super) fn effective_archive_candidates(
    archive: &str,
    archive_candidates: &HashMap<String, Vec<PathBuf>>,
) -> Vec<PathBuf> {
    let target = normalize_bundle_name(archive);
    let mut candidates = archive_candidates.get(&target).cloned().unwrap_or_default();
    let direct_candidates = candidates
        .iter()
        .filter(|candidate| {
            candidate
                .file_stem()
                .and_then(|value| value.to_str())
                .is_some_and(|stem| normalize_bundle_name(stem) == target)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !direct_candidates.is_empty() {
        candidates = direct_candidates;
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

pub(super) fn aggregate_counts(maps: &[MapTransformAudit]) -> WorldCatalogAuditCounts {
    let mut counts = WorldCatalogAuditCounts {
        map_archives: maps.len(),
        ..WorldCatalogAuditCounts::default()
    };
    let mut dependency_paths = BTreeSet::<String>::new();
    counts.processed_map_archives = maps.len();
    for map in maps {
        if map.passed {
            counts.passed_map_archives += 1;
        } else {
            counts.failed_map_archives += 1;
        }
        dependency_paths.extend(
            map.resolved_dependency_bundles
                .iter()
                .map(|bundle| bundle.path.clone()),
        );
        counts.missing_dependencies += map.counts.missing_dependencies;
        counts.ambiguous_dependency_archives += map.counts.ambiguous_dependency_archives;
        counts.scene_assets += map.counts.scene_assets;
        counts.scene_nodes += map.counts.scene_nodes;
        counts.root_nodes += map.counts.root_nodes;
        counts.unit_scale_nodes += map.counts.unit_scale_nodes;
        counts.nonunit_scale_nodes += map.counts.nonunit_scale_nodes;
        counts.uniform_scale_nodes += map.counts.uniform_scale_nodes;
        counts.nonuniform_scale_nodes += map.counts.nonuniform_scale_nodes;
        counts.positive_scale_nodes += map.counts.positive_scale_nodes;
        counts.negative_scale_nodes += map.counts.negative_scale_nodes;
        counts.zero_scale_nodes += map.counts.zero_scale_nodes;
        counts.singular_scale_nodes += map.counts.singular_scale_nodes;
        counts.nonfinite_nodes += map.counts.nonfinite_nodes;
        counts.invalid_rotation_nodes += map.counts.invalid_rotation_nodes;
        counts.nonzero_root_origins += map.counts.nonzero_root_origins;
        counts.root_origin_contract_mismatches += map.counts.root_origin_contract_mismatches;
        counts.colliders += map.counts.colliders;
        counts.collider_centers += map.counts.collider_centers;
        counts.collider_sizes += map.counts.collider_sizes;
        counts.collider_radii += map.counts.collider_radii;
        counts.collider_heights += map.counts.collider_heights;
        for (collider_type, count) in &map.counts.collider_types {
            *counts
                .collider_types
                .entry(collider_type.clone())
                .or_default() += count;
        }
        counts.invalid_colliders += map.counts.invalid_colliders;
        counts.transform_exceptions += map.counts.transform_exceptions;
        counts.collider_exceptions += map.counts.collider_exceptions;
        counts.errors += map.counts.errors;
        counts.max_abs_scale_component = counts
            .max_abs_scale_component
            .max(map.max_abs_scale_component);
    }
    counts.dependency_bundles = dependency_paths.len();
    counts
}

pub(super) fn expected_native_tile_horizontal_origin(map: &str) -> Option<[f64; 2]> {
    let mut parts = map.split('_');
    if !parts.next()?.eq_ignore_ascii_case("Map") {
        return None;
    }
    let tile_x = parse_signed_decimal(parts.next()?)? as f64;
    let tile_y = parse_signed_decimal(parts.next()?)? as f64;
    if parts.next().is_some() {
        return None;
    }
    Some([-tile_x * 512.0, tile_y * 512.0])
}

pub(super) fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

pub(super) fn approx_eq(left: f64, right: f64, epsilon: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= epsilon
}

pub(super) fn blake3_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|err| format!("could not hash {}: {err}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| format!("could not hash {}: {err}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}
