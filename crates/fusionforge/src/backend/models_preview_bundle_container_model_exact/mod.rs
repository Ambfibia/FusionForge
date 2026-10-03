use super::super::*;

pub(in super::super) fn preview_bundle_container_model(
    bundle_path: String,
    project_dir: Option<String>,
    container_paths: Vec<String>,
) -> EditorResult<serde_json::Value> {
    preview_bundle_container_model_with_mode(bundle_path, project_dir, container_paths, false)
}

pub(in super::super) fn suppress_exact_nif_parse_warning_for_serialized_game_object(
    result: &mut JsonValue,
    normalized_route: &str,
    matched_paths: &[String],
) -> Option<JsonValue> {
    if !normalized_route.ends_with(".nif") || matched_paths.len() != 1 {
        return None;
    }
    let targets = result.get("exactContainerTargets")?.as_array()?;
    if targets.len() != 1 {
        return None;
    }
    let target = targets[0].clone();
    if target.get("proof").and_then(JsonValue::as_str) != Some("directAssetBundleContainerPointer")
        || target.get("objectType").and_then(JsonValue::as_str) != Some("GameObject")
        || target
            .get("exactRoute")
            .and_then(JsonValue::as_str)
            .map(normalized_asset_path)
            .as_deref()
            != Some(normalized_route)
    {
        return None;
    }

    let roots = result.pointer("/modelHierarchy/roots")?.as_array()?;
    if roots.len() != 1 {
        return None;
    }
    let root = roots[0].clone();
    if root.get("sourceAssetIndex").and_then(JsonValue::as_u64)
        != target.get("assetIndex").and_then(JsonValue::as_u64)
        || root.get("gameObjectPathId").and_then(JsonValue::as_i64)
            != target.get("pathId").and_then(JsonValue::as_i64)
    {
        return None;
    }

    let warning_prefix = format!("{}: NIF parse failed:", matched_paths[0]);
    let warnings = result.get_mut("warnings")?.as_array_mut()?;
    let matching = warnings
        .iter()
        .enumerate()
        .filter(|(_, warning)| {
            warning
                .as_str()
                .is_some_and(|warning| warning.starts_with(&warning_prefix))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if matching.len() > 1 {
        return None;
    }
    let suppressed_warning = matching
        .first()
        .and_then(|index| warnings.remove(*index).as_str().map(str::to_string));
    let byte_parse_disposition = if suppressed_warning.is_some() {
        "attempted-then-suppressed-as-non-authoritative"
    } else {
        "skipped-because-direct-serialized-game-object-is-authoritative"
    };
    let evidence = json!({
        "schema": "ffone.exact-serialized-game-object-closure.v1",
        "proof": "directAssetBundleContainerPointerMatchesOnlyHierarchyRoot",
        "exactRoute": matched_paths[0],
        "containerAssetName": target.get("containerAssetName"),
        "assetIndex": target.get("assetIndex"),
        "assetName": target.get("assetName"),
        "gameObjectPathId": target.get("pathId"),
        "rootTransformPathId": root.get("transformPathId"),
        "rootName": root.get("name"),
        "suppressedWarning": suppressed_warning,
        "byteParseDisposition": byte_parse_disposition,
        "warningDisposition": "legacy-nif-byte-parse-is-non-authoritative-for-direct-serialized-game-object-route",
    });
    result.as_object_mut()?.insert(
        "exactSerializedGameObjectClosure".to_string(),
        evidence.clone(),
    );
    Some(evidence)
}

pub(in super::super) fn resolve_exact_unrelated_mesh_filter_warning(result: &mut JsonValue) -> Option<JsonValue> {
    const PREFIX: &str = "Ignored ";
    const SUFFIX: &str = " mesh(es) from unrelated character roots in the shared preload range.";
    let warnings = result.get_mut("warnings")?.as_array_mut()?;
    let matching = warnings
        .iter()
        .enumerate()
        .filter_map(|(index, warning)| {
            let warning = warning.as_str()?;
            let count = warning
                .strip_prefix(PREFIX)?
                .strip_suffix(SUFFIX)?
                .parse::<usize>()
                .ok()?;
            Some((index, count, warning.to_string()))
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 || matching[0].1 == 0 {
        return None;
    }
    let (index, excluded_candidate_meshes, source_warning) = matching[0].clone();
    warnings.remove(index);
    let selected_meshes = result
        .get("meshes")
        .and_then(JsonValue::as_array)
        .map_or(0, Vec::len);
    let evidence = json!({
        "schema": "ffone.exact-character-root-mesh-selection.v1",
        "policy": "retain-only-meshes-under-proven-selected-character-roots",
        "excludedCandidateMeshes": excluded_candidate_meshes,
        "selectedMeshes": selected_meshes,
        "sourceWarning": source_warning,
        "warningDisposition": "resolved-as-positive-character-root-ownership-proof",
    });
    result
        .as_object_mut()?
        .insert("exactMeshSelectionProof".to_string(), evidence.clone());
    Some(evidence)
}

pub(in super::super) fn preview_bundle_container_model_exact(
    bundle_path: String,
    project_dir: Option<String>,
    container_path: String,
) -> EditorResult<serde_json::Value> {
    let normalized = normalized_asset_path(&container_path);
    if normalized.is_empty() {
        return Err("exact logical-model container route is empty".to_string());
    }
    let mut result = preview_bundle_container_model_with_mode(
        bundle_path,
        project_dir,
        vec![container_path.clone()],
        true,
    )?;
    let matched = result
        .get("matchedPaths")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .filter(|path| normalized_asset_path(path) == normalized)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let unique_matched = matched
        .iter()
        .map(|path| normalized_asset_path(path))
        .collect::<BTreeSet<_>>();
    if unique_matched.len() != 1 {
        return Err(format!(
            "exact logical-model route '{}' resolved to {} distinct routes; refusing an ambiguous or missing export",
            container_path,
            unique_matched.len()
        ));
    }
    if result
        .get("meshes")
        .and_then(JsonValue::as_array)
        .is_none_or(Vec::is_empty)
    {
        return Err(format!(
            "exact logical-model route '{}' contains no publishable mesh",
            container_path
        ));
    }
    let hierarchy_roots = result
        .pointer("/modelHierarchy/roots")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "exact logical-model export has no Transform hierarchy".to_string())?;
    if hierarchy_roots.len() != 1 {
        return Err(format!(
            "exact logical-model route '{}' resolved {} logical roots; refusing to scatter them into one file",
            container_path,
            hierarchy_roots.len()
        ));
    }
    let logical_name = hierarchy_roots[0]
        .get("name")
        .and_then(JsonValue::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| "exact logical-model root has no true m_Name".to_string())?
        .to_string();
    suppress_exact_nif_parse_warning_for_serialized_game_object(&mut result, &normalized, &matched);
    suppress_exact_kfm_parse_warning_for_serialized_hierarchy(&mut result, &normalized, &matched);
    resolve_exact_unrelated_mesh_filter_warning(&mut result);
    let model_hierarchy = result
        .get("modelHierarchy")
        .cloned()
        .ok_or_else(|| "exact logical-model export has no modelHierarchy".to_string())?;
    let animations = result
        .get_mut("animations")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| "exact logical-model export has no animations array".to_string())?;
    npc_legacy_animation::mark_unbound_animation_bindings(&model_hierarchy, animations)?;
    npc_legacy_animation::classify_metadata_only_sound_event_pointers(animations)?;
    let animations = result
        .get("animations")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "exact logical-model export has no animations array".to_string())?;
    npc_legacy_animation::validate_exact_animation_source(&model_hierarchy, animations)?;
    if let Some(object) = result.as_object_mut() {
        object.insert(
            "matchedPaths".to_string(),
            json!([matched.first().cloned().unwrap_or(container_path.clone())]),
        );
        object.insert("matchedRouteOccurrences".to_string(), json!(matched.len()));
        object.insert("exactContainerRoute".to_string(), json!(container_path));
        object.insert("logicalName".to_string(), json!(logical_name));
    }
    Ok(result)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExactLogicalModelEnvironmentWarmup {
    pub bundle_path: String,
    pub requested_routes: usize,
    pub extract_directories: usize,
    pub unity_assets: usize,
    pub indexed_objects: usize,
    pub indexed_container_routes: usize,
}

/// Parses and indexes one physical bundle environment once before an exact
/// route batch. The following route exports reuse the last-environment cache
/// and perform only their direct serialized closure traversal.
pub(crate) fn prewarm_exact_logical_model_environment(
    bundle_path: &str,
    project_dir: &Path,
    routes: &[String],
) -> EditorResult<ExactLogicalModelEnvironmentWarmup> {
    let bundle = PathBuf::from(bundle_path);
    let extract_dir = if bundle.is_dir() {
        bundle.clone()
    } else {
        extract_bundle_cached(project_dir, &bundle)?
    };
    let wanted = routes
        .iter()
        .map(|path| normalized_asset_path(path))
        .filter(|path| !path.is_empty())
        .collect::<BTreeSet<_>>();
    let mut extract_dirs = vec![extract_dir.clone()];
    extract_dirs.extend(staged_preview_sibling_asset_dirs(project_dir, &bundle));
    extract_dirs.extend(explicit_dependency_extract_dirs(project_dir)?);
    let dependency_dirs =
        preview_dependency_extract_dirs(project_dir, &bundle, &extract_dirs, &wanted)?;
    extract_dirs.extend(dependency_dirs);
    let env = cached_unity_environment_from_extract_dirs(&extract_dirs);
    let container_entries = cached_preview_container_entries(&env);
    cache_exact_batch_environment(&bundle, project_dir, &extract_dir, Rc::clone(&env));
    Ok(ExactLogicalModelEnvironmentWarmup {
        bundle_path: bundle_path.to_string(),
        requested_routes: routes.len(),
        extract_directories: extract_dirs.len(),
        unity_assets: env.assets.len(),
        indexed_objects: env.assets.iter().map(|asset| asset.objects.len()).sum(),
        indexed_container_routes: container_entries.len(),
    })
}
