use super::*;

pub(super) fn validate_exact_source(
    source: &JsonValue,
    candidate: &RouteCandidate,
) -> Result<
    (
        String,
        String,
        EquipmentPhysicalTarget,
        EquipmentSourceFacts,
    ),
    String,
> {
    require_string(source, "schema", LOGICAL_SOURCE_SCHEMA)?;
    require_string(source, "selectionMode", "exact-container-route")?;
    require_string(source, "status", "ready")?;
    let logical_name = source
        .get("logicalName")
        .and_then(JsonValue::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| "exact equipment source has no true logicalName".to_string())?
        .to_string();
    let source_route = source
        .get("exactContainerRoute")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| "exact equipment source has no exactContainerRoute".to_string())?;
    if normalize_logical_model_route(source_route) != candidate.route.normalized_route {
        return Err("exact source route differs from the table-owned route".to_string());
    }
    let warnings = source
        .get("warnings")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "exact equipment source has no warnings array".to_string())?;
    if !warnings.is_empty() {
        return Err(format!(
            "exact source retains {} warning(s); no lossy warning suppression is allowed",
            warnings.len()
        ));
    }
    let roots = source
        .pointer("/modelHierarchy/roots")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "exact equipment source has no modelHierarchy.roots".to_string())?;
    if roots.len() != 1
        || roots[0].get("name").and_then(JsonValue::as_str) != Some(logical_name.as_str())
    {
        return Err(
            "exact equipment source does not have one true-name hierarchy root".to_string(),
        );
    }
    let meshes = source
        .get("meshes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "exact equipment source has no meshes array".to_string())?;
    if meshes.is_empty() {
        return Err("exact equipment source contains no publishable mesh".to_string());
    }
    let targets = source
        .get("exactContainerTargets")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "exact equipment source has no exactContainerTargets proof".to_string())?
        .iter()
        .filter(|target| {
            target
                .get("exactRoute")
                .and_then(JsonValue::as_str)
                .map(normalize_logical_model_route)
                .as_deref()
                == Some(candidate.route.normalized_route.as_str())
        })
        .collect::<Vec<_>>();
    if targets.len() != 1 {
        return Err(format!(
            "exact equipment route has {} direct serialized container targets",
            targets.len()
        ));
    }
    let target = targets[0];
    let physical_target = EquipmentPhysicalTarget {
        asset_index: required_u64(target, "assetIndex")?,
        asset_name: required_owned_string(target, "assetName")?,
        path_id: required_i64(target, "pathId")?,
        object_type: required_owned_string(target, "objectType")?,
    };
    if physical_target.object_type != "GameObject" {
        return Err(format!(
            "exact equipment container target is {}, not GameObject",
            physical_target.object_type
        ));
    }
    let safe_glb = minimal_windows_glb_filename(&logical_name)
        .map_err(|err| format!("true equipment m_Name is not safely publishable: {err}"))?;
    let safe_true_name = safe_glb
        .strip_suffix(".glb")
        .ok_or_else(|| "safe true-name GLB has no suffix".to_string())?
        .to_string();
    let coordinate = source
        .get("nativeCoordinateContract")
        .ok_or_else(|| "exact equipment source has no nativeCoordinateContract".to_string())?;
    let hierarchy_nodes = source
        .pointer("/modelHierarchy/nodes")
        .and_then(JsonValue::as_array)
        .map_or(0, Vec::len);
    let skinned_meshes = meshes
        .iter()
        .filter(|mesh| mesh.get("skin").is_some_and(|skin| !skin.is_null()))
        .count();
    let facts = EquipmentSourceFacts {
        hierarchy_nodes: u64_count(hierarchy_nodes)?,
        meshes: u64_count(meshes.len())?,
        skinned_meshes: u64_count(skinned_meshes)?,
        joints: u64_count(
            source
                .pointer("/skeleton/joints")
                .and_then(JsonValue::as_array)
                .map_or(0, Vec::len),
        )?,
        animations: u64_count(
            source
                .get("animations")
                .and_then(JsonValue::as_array)
                .map_or(0, Vec::len),
        )?,
        materials: u64_count(
            source
                .get("materials")
                .and_then(JsonValue::as_object)
                .map_or(0, serde_json::Map::len),
        )?,
        textures: u64_count(
            source
                .get("textures")
                .and_then(JsonValue::as_object)
                .map_or(0, serde_json::Map::len),
        )?,
        coordinate_contract: required_owned_string(coordinate, "schema")?,
        origin_policy: required_owned_string(coordinate, "originPolicy")?,
        unit_scale: required_owned_string(coordinate, "unitScale")?,
    };
    Ok((logical_name, safe_true_name, physical_target, facts))
}

pub(super) fn reject_existing_destination(output: &Path, manifest: &Path) -> Result<(), String> {
    if fs::symlink_metadata(output).is_ok() {
        return Err(format!(
            "equipment source output must be fresh: {}",
            output.display()
        ));
    }
    if fs::symlink_metadata(manifest).is_ok() {
        return Err(format!(
            "equipment source manifest must be fresh: {}",
            manifest.display()
        ));
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(format!(
            "equipment source output parent must exist: {}",
            parent.display()
        ));
    }
    Ok(())
}

pub(super) fn require_string(value: &JsonValue, field: &str, expected: &str) -> Result<(), String> {
    let actual = value.get(field).and_then(JsonValue::as_str);
    if actual != Some(expected) {
        return Err(format!(
            "exact source field {field:?} must be {expected:?}, got {actual:?}"
        ));
    }
    Ok(())
}
