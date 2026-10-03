use super::*;

pub(super) fn blocker_for_provenance(
    subject_kind: &str,
    provenance: &LogicalModelRouteProvenance,
    code: impl Into<String>,
    detail: impl Into<String>,
) -> LogicalModelPlanBlocker {
    LogicalModelPlanBlocker {
        subject_kind: subject_kind.to_string(),
        normalized_route: Some(provenance.normalized_route.clone()),
        exact_routes: vec![provenance.exact_route.clone()],
        owners: vec![provenance.owner.clone()],
        code: code.into(),
        detail: detail.into(),
        kfm_pointer_traversal: None,
        kfm_preload_ownership: None,
    }
}

pub(super) fn physical_fingerprint_for_pointer(
    env: &super::super::UnityEnvironment,
    pointer: &super::super::Pointer,
    exact_route: &str,
    serialized_asset_hashes: &mut BTreeMap<usize, String>,
) -> Result<LogicalModelPhysicalFingerprint, String> {
    let target = env.resolve_pointer(pointer).map_err(|error| {
        format!("exact container PPtr for {exact_route:?} could not be resolved: {error}")
    })?;
    let target_asset = env
        .assets
        .get(target.asset)
        .ok_or_else(|| "resolved target serialized asset is absent".to_string())?;
    let target_info = target_asset
        .objects
        .get(&target.path_id)
        .ok_or_else(|| "resolved target object is absent".to_string())?;
    let serialized_asset_sha256 = serialized_asset_hashes
        .entry(target.asset)
        .or_insert_with(|| format!("{:x}", Sha256::digest(&target_asset.data)))
        .clone();
    Ok(LogicalModelPhysicalFingerprint {
        serialized_asset_sha256,
        path_id: target.path_id,
        object_type: target_asset.object_type_name(target_info),
    })
}

pub(super) fn physical_fingerprint_for_occurrence(
    index: &PhysicalEnvironmentRouteIndex,
    occurrence: &LogicalModelOccurrence,
) -> Result<LogicalModelPhysicalFingerprint, PhysicalOccurrenceResolutionError> {
    let Some(owner) = index.owners.get(&occurrence.owner.asset_name) else {
        return Err(PhysicalOccurrenceResolutionError::new(
            "PhysicalTargetUnresolved",
            format!(
                "exact owner asset {:?} resolved to 0 serialized assets",
                occurrence.owner.asset_name
            ),
        ));
    };
    if owner.serialized_asset_match_count != 1 {
        return Err(PhysicalOccurrenceResolutionError::new(
            "PhysicalTargetUnresolved",
            format!(
                "exact owner asset {:?} resolved to {} serialized assets",
                occurrence.owner.asset_name, owner.serialized_asset_match_count
            ),
        ));
    }
    let routes = owner.routes.as_ref().map_err(|detail| {
        PhysicalOccurrenceResolutionError::new("PhysicalTargetUnresolved", detail.clone())
    })?;
    let route = routes.get(&occurrence.normalized_route);
    let normalized_spelling_count = route.map_or(0, |route| route.exact_spellings.len());
    if normalized_spelling_count != 1 {
        return Err(PhysicalOccurrenceResolutionError::new(
            "PhysicalRouteSpellingConflict",
            format!(
                "exact route {:?} resolved to {} normalized spellings in owner {:?}",
                occurrence.exact_route, normalized_spelling_count, occurrence.owner
            ),
        ));
    }
    let exact_occurrences = route
        .and_then(|route| route.exact_occurrences.get(&occurrence.exact_route))
        .map(Vec::as_slice)
        .unwrap_or_default();
    if exact_occurrences.is_empty() {
        return Err(PhysicalOccurrenceResolutionError::new(
            "PhysicalTargetUnresolved",
            format!(
                "exact route {:?} resolved to no exact container occurrence in owner {:?}",
                occurrence.exact_route, occurrence.owner
            ),
        ));
    }

    let duplicate = exact_occurrences.len() > 1;
    let mut fingerprints = BTreeSet::new();
    for (index, result) in exact_occurrences.iter().enumerate() {
        match result {
            Ok(fingerprint) => {
                fingerprints.insert(fingerprint.clone());
            }
            Err(detail) => {
                return Err(PhysicalOccurrenceResolutionError::new(
                    if duplicate {
                        "PhysicalDuplicateTargetUnresolved"
                    } else {
                        "PhysicalTargetUnresolved"
                    },
                    format!(
                        "exact route {:?} occurrence {}/{} in owner {:?} could not resolve its physical target: {}",
                        occurrence.exact_route,
                        index + 1,
                        exact_occurrences.len(),
                        occurrence.owner,
                        detail
                    ),
                ));
            }
        }
    }
    if fingerprints.len() != 1 {
        return Err(PhysicalOccurrenceResolutionError::new(
            if duplicate {
                "PhysicalDuplicateTargetConflict"
            } else {
                "PhysicalTargetConflict"
            },
            format!(
                "exact route {:?} has {} exact container occurrences resolving to {} distinct serialized-asset SHA-256/PathID/object-type fingerprints in owner {:?}",
                occurrence.exact_route,
                exact_occurrences.len(),
                fingerprints.len(),
                occurrence.owner
            ),
        ));
    }
    Ok(fingerprints
        .into_iter()
        .next()
        .expect("one physical fingerprint was proven"))
}

pub(super) fn exact_nonnegative_usize_field(
    metadata: &super::super::UnityValue,
    field: &str,
) -> Result<usize, KfmResolveError> {
    let value = metadata.get(field).ok_or_else(|| {
        KfmResolveError::new(
            "kfmPreloadMetadataFieldMissing",
            format!("exact KFM container metadata has no '{field}' field"),
        )
    })?;
    match value {
        super::super::UnityValue::Int(value) => usize::try_from(*value),
        super::super::UnityValue::UInt(value) => usize::try_from(*value),
        _ => {
            return Err(KfmResolveError::new(
                "kfmPreloadMetadataFieldInvalid",
                format!("exact KFM container metadata field '{field}' is not an integer"),
            ));
        }
    }
    .map_err(|_| {
        KfmResolveError::new(
            "kfmPreloadMetadataFieldInvalid",
            format!("exact KFM container metadata field '{field}' is outside usize range"),
        )
    })
}

pub(super) fn exact_required_pointer_key(
    env: &super::super::UnityEnvironment,
    value: Option<&super::super::UnityValue>,
    context: &str,
) -> Result<(usize, i64), String> {
    let pointer = value
        .and_then(super::super::UnityValue::as_pointer)
        .ok_or_else(|| format!("{context} is missing, null, or not a pointer"))?;
    let key = env
        .resolve_pointer(pointer)
        .map_err(|err| format!("{context} cannot be resolved: {err}"))?;
    exact_object_body(env, (key.asset, key.path_id))?;
    Ok((key.asset, key.path_id))
}

pub(super) fn exact_nullable_pointer_key(
    env: &super::super::UnityEnvironment,
    value: Option<&super::super::UnityValue>,
    context: &str,
) -> Result<Option<(usize, i64)>, String> {
    let value = value.ok_or_else(|| format!("{context} is missing"))?;
    let super::super::UnityValue::Pointer(pointer) = value else {
        return Err(format!("{context} is not a pointer"));
    };
    if pointer.is_null() {
        return Ok(None);
    }
    let key = env
        .resolve_pointer(pointer)
        .map_err(|err| format!("{context} cannot be resolved: {err}"))?;
    exact_object_body(env, (key.asset, key.path_id))?;
    Ok(Some((key.asset, key.path_id)))
}

pub(super) fn exact_component_class_and_pointer(value: &super::super::UnityValue) -> Option<(i64, &super::super::Pointer)> {
    match value {
        super::super::UnityValue::Array(items) if items.len() == 2 => {
            Some((items[0].as_i64()?, items[1].as_pointer()?))
        }
        super::super::UnityValue::Pair(left, right) => Some((left.as_i64()?, right.as_pointer()?)),
        _ => None,
    }
}

pub(super) fn exact_pointer_closure(
    env: &super::super::UnityEnvironment,
    root: (usize, i64),
) -> Result<BTreeSet<(usize, i64)>, String> {
    let mut closure = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(key) = pending.pop() {
        if !closure.insert(key) {
            continue;
        }
        let object = exact_object_body(env, key)?;
        let mut pointers = Vec::new();
        crate::collect_value_pointers(&object.body, &mut pointers);
        let mut children = Vec::with_capacity(pointers.len());
        for pointer in pointers {
            let child = env.resolve_pointer(&pointer).map_err(|err| {
                format!(
                    "{}#{} contains an unresolved non-null pointer: {err}",
                    key.0, key.1
                )
            })?;
            let child = (child.asset, child.path_id);
            exact_object_body(env, child)?;
            children.push(child);
        }
        children.sort();
        children.dedup();
        for child in children.into_iter().rev() {
            if !closure.contains(&child) {
                pending.push(child);
            }
        }
    }
    Ok(closure)
}

pub(super) fn exact_typed_dependency(
    env: &super::super::UnityEnvironment,
    value: Option<&super::super::UnityValue>,
    expected_type: &str,
    context: &str,
) -> Result<(usize, i64), String> {
    let key = exact_required_pointer_key(env, value, context)?;
    let object = exact_object_body(env, key)?;
    if object.object_type != expected_type {
        return Err(format!(
            "{context} expected {expected_type}, got {} at {}#{}",
            object.object_type, key.0, key.1
        ));
    }
    Ok(key)
}

pub(super) fn unresolved_pointer_evidence(
    env: &super::super::UnityEnvironment,
    pointer: &super::super::Pointer,
    resolution_error: impl Into<String>,
) -> UnresolvedPointerEvidence {
    let source_asset = env.assets.get(pointer.source_asset);
    let asset_ref = usize::try_from(pointer.file_id)
        .ok()
        .and_then(|index| {
            source_asset?
                .asset_refs
                .get(index)
                .map(|asset_ref| (index, asset_ref))
        })
        .map(|(asset_ref_index, asset_ref)| PointerAssetRefEvidence {
            asset_ref_index,
            asset_path: asset_ref.asset_path.clone(),
            file_path: asset_ref.file_path.clone(),
            archive_name: super::super::unity::archive_name_from_ref(&asset_ref.file_path)
                .or_else(|| super::super::unity::archive_name_from_ref(&asset_ref.asset_path)),
            referenced_asset_name: super::super::unity::archive_ref_asset_name(&asset_ref.file_path)
                .or_else(|| super::super::unity::archive_ref_asset_name(&asset_ref.asset_path)),
        });
    let loaded_path_id_candidates = env
        .assets
        .iter()
        .enumerate()
        .filter(|(_, asset)| asset.objects.contains_key(&pointer.path_id))
        .filter_map(|(asset_index, _)| {
            resolved_object_key_evidence(env, (asset_index, pointer.path_id)).ok()
        })
        .collect();
    UnresolvedPointerEvidence {
        source_asset_index: pointer.source_asset,
        source_asset_name: source_asset.map(|asset| asset.name.clone()),
        source_asset_format: source_asset.map(|asset| asset.format),
        file_id: pointer.file_id,
        path_id: pointer.path_id,
        asset_ref,
        loaded_path_id_candidates,
        resolution_error: resolution_error.into(),
    }
}
