use super::*;

pub(super) fn remove_owned_map_session(path: &Path) -> Result<(), String> {
    let child_name_ok = path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.starts_with("map-"));
    let Some(parent) = path.parent() else {
        return Err(format!("map session has no parent: {}", path.display()));
    };
    if !child_name_ok || !is_owned_audit_temp_dir(parent) {
        return Err(format!(
            "refusing to remove unowned map audit session: {}",
            path.display()
        ));
    }
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|err| format!("could not canonicalize {}: {err}", parent.display()))?;
    let canonical_path = fs::canonicalize(path)
        .map_err(|err| format!("could not canonicalize {}: {err}", path.display()))?;
    if canonical_path.parent() != Some(canonical_parent.as_path()) {
        return Err(format!(
            "refusing map audit cleanup outside owned parent: {}",
            canonical_path.display()
        ));
    }
    fs::remove_dir_all(&canonical_path).map_err(|err| {
        format!(
            "could not remove generated map audit session {}: {err}",
            canonical_path.display()
        )
    })
}

pub(super) fn failed_map_session_report(
    map_path: &Path,
    session_path: &Path,
    error: std::io::Error,
) -> MapTransformAudit {
    let map = map_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("<invalid-map-name>")
        .to_string();
    let errors = vec![WorldAuditError::new(
        Some(&map),
        "mapAuditSessionCreateFailed",
        format!("could not create {}: {error}", session_path.display()),
    )];
    let expected_native_tile_horizontal_origin = expected_native_tile_horizontal_origin(&map);
    MapTransformAudit {
        map,
        bundle_path: readable_absolute_path(map_path),
        bundle_blake3: blake3_file(map_path).unwrap_or_default(),
        passed: false,
        scene_assets: Vec::new(),
        declared_dependency_archives: Vec::new(),
        resolved_dependency_bundles: Vec::new(),
        missing_dependencies: Vec::new(),
        ambiguous_dependency_archives: Vec::new(),
        counts: MapTransformAuditCounts {
            errors: errors.len(),
            ..MapTransformAuditCounts::default()
        },
        max_abs_scale: [0.0; 3],
        max_abs_scale_component: 0.0,
        expected_native_tile_horizontal_origin,
        root_origin_matches_tile_contract: false,
        root_origins: Vec::new(),
        transform_exceptions: Vec::new(),
        collider_exceptions: Vec::new(),
        errors,
    }
}
