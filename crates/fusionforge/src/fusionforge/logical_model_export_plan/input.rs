use super::*;

pub(super) fn resolve_kfm_from_environment(
    env: &super::super::UnityEnvironment,
    provenance: &LogicalModelRouteProvenance,
    dependency_archives: &[DependencyArchiveResolutionEvidence],
) -> Result<KfmPayloadEvidence, KfmResolveError> {
    let matching_assets = env
        .assets
        .iter()
        .enumerate()
        .filter(|(_, asset)| asset.name == provenance.owner.asset_name)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if matching_assets.len() != 1 {
        return Err(KfmResolveError::new(
            "kfmPayloadAssetCountMismatch",
            format!(
                "expected one serialized asset named '{}', found {}",
                provenance.owner.asset_name,
                matching_assets.len()
            ),
        ));
    }
    let asset_index = matching_assets[0];
    let asset = &env.assets[asset_index];
    let mut exact_occurrences = Vec::new();
    let mut normalized_variants = BTreeSet::new();
    for info in asset.objects.values() {
        if asset.object_type_name(info) != "AssetBundle" {
            continue;
        }
        let body = asset
            .read_object(asset_index, info)
            .map_err(|err| KfmResolveError::new("kfmAssetBundleReadFailed", err.to_string()))?;
        let preload_table = super::super::value_array(body.get("m_PreloadTable")).to_vec();
        for entry in super::super::value_array(body.get("m_Container")) {
            let Some((path, metadata)) = super::super::pair_name_value(entry) else {
                continue;
            };
            if normalize_logical_model_route(path) != provenance.normalized_route {
                continue;
            }
            normalized_variants.insert(path.to_string());
            if path == provenance.exact_route {
                exact_occurrences.push(ExactContainerOccurrence {
                    metadata: metadata.clone(),
                    preload_table: preload_table.clone(),
                    asset_bundle_asset_name: asset.name.clone(),
                    asset_bundle_path_id: info.path_id,
                });
            }
        }
    }
    if normalized_variants.len() != 1 || exact_occurrences.len() != 1 {
        return Err(KfmResolveError::new(
            "kfmPayloadRouteCountMismatch",
            format!(
                "catalog route '{}' resolved to {} normalized variants and {} exact payload entries",
                provenance.exact_route,
                normalized_variants.len(),
                exact_occurrences.len()
            ),
        ));
    }
    let occurrence = &exact_occurrences[0];
    let pointer = occurrence
        .metadata
        .get("asset")
        .and_then(super::super::UnityValue::as_pointer)
        .ok_or_else(|| {
            KfmResolveError::new(
                "kfmPayloadPointerMissing",
                "the exact KFM container entry has no asset pointer",
            )
        })?;
    let key = env
        .resolve_pointer(pointer)
        .map_err(|err| KfmResolveError::new("kfmPayloadPointerUnresolved", err.to_string()))?;
    let target_asset = env.assets.get(key.asset).ok_or_else(|| {
        KfmResolveError::new("kfmPayloadTargetAssetMissing", "resolved asset is absent")
    })?;
    let target_info = target_asset.objects.get(&key.path_id).ok_or_else(|| {
        KfmResolveError::new("kfmPayloadTargetObjectMissing", "resolved object is absent")
    })?;
    let container_object_type = target_asset.object_type_name(target_info);
    let mut visited = BTreeSet::new();
    let payload_match = exact_kfm_text_asset_payload(env, key.asset, key.path_id, &mut visited);
    let mut pointer_traversal = KfmPointerTraversalEvidence {
        exact_container_occurrence_count: exact_occurrences.len(),
        container_asset_name: target_asset.name.clone(),
        container_path_id: key.path_id,
        container_object_type: container_object_type.clone(),
        payload_asset_name: None,
        payload_path_id: None,
        payload_object_type: None,
        pointer_nodes_visited: visited.len(),
        pointer_cycle_guard: "visitedAssetIndexAndPathIdSet".to_string(),
        pointer_safety_budget: None,
        pointer_budget_exceeded: false,
        pointer_graph_exhausted: payload_match.is_none(),
    };
    let Some((resolution, references)) = payload_match else {
        return resolve_kfm_from_preload_ownership(
            env,
            occurrence,
            (key.asset, key.path_id),
            pointer_traversal,
            dependency_archives,
        );
    };
    let payload_asset = env.assets.get(resolution.asset_index).ok_or_else(|| {
        KfmResolveError::new(
            "kfmTraversalPayloadAssetMissing",
            "pointer traversal returned an absent payload asset",
        )
        .with_pointer_traversal(pointer_traversal.clone())
    })?;
    pointer_traversal.payload_asset_name = Some(payload_asset.name.clone());
    pointer_traversal.payload_path_id = Some(resolution.path_id);
    pointer_traversal.payload_object_type = Some(resolution.object_type.clone());
    let nif_references = references
        .iter()
        .filter(|reference| normalize_logical_model_route(reference).ends_with(".nif"))
        .cloned()
        .collect::<Vec<_>>();
    debug_assert!(!nif_references.is_empty());
    Ok(KfmPayloadEvidence {
        ownership_evidence: "rawKfmPayload".to_string(),
        payload_asset_name: Some(payload_asset.name.clone()),
        payload_path_id: Some(resolution.path_id),
        payload_object_type: Some(resolution.object_type),
        byte_length: Some(resolution.bytes.len()),
        references,
        nif_references,
        pointer_traversal,
        preload_ownership: None,
        self_contained_game_object: None,
    })
}

pub(super) fn resolve_kfm_from_preload_ownership(
    env: &super::super::UnityEnvironment,
    occurrence: &ExactContainerOccurrence,
    container_key: (usize, i64),
    pointer_traversal: KfmPointerTraversalEvidence,
    dependency_archives: &[DependencyArchiveResolutionEvidence],
) -> Result<KfmPayloadEvidence, KfmResolveError> {
    let preload_index = exact_nonnegative_usize_field(&occurrence.metadata, "preloadIndex")
        .map_err(|err| err.with_pointer_traversal(pointer_traversal.clone()))?;
    let preload_size = exact_nonnegative_usize_field(&occurrence.metadata, "preloadSize")
        .map_err(|err| err.with_pointer_traversal(pointer_traversal.clone()))?;
    let preload_range_end = preload_index.checked_add(preload_size).ok_or_else(|| {
        KfmResolveError::new(
            "kfmPreloadRangeOverflow",
            format!("preload range {preload_index} + {preload_size} overflows usize"),
        )
        .with_pointer_traversal(pointer_traversal.clone())
    })?;
    if preload_range_end > occurrence.preload_table.len() {
        return Err(KfmResolveError::new(
            "kfmPreloadRangeOutOfBounds",
            format!(
                "exact preload range [{preload_index}, {preload_range_end}) exceeds m_PreloadTable length {}",
                occurrence.preload_table.len()
            ),
        )
        .with_pointer_traversal(pointer_traversal));
    }

    let container_pointer = resolved_object_key_evidence(env, container_key)
        .map_err(|err| err.with_pointer_traversal(pointer_traversal.clone()))?;
    let mut resolved_preload_pointers = Vec::new();
    let mut retained_external_preload_pointers = Vec::new();
    let mut unresolved_preload_table_indices = Vec::new();
    let mut unresolved_preload_pointers = Vec::new();
    let mut preload_indices_by_key = BTreeMap::<(usize, i64), Vec<usize>>::new();
    for preload_table_index in preload_index..preload_range_end {
        let value = &occurrence.preload_table[preload_table_index];
        let pointer = match value {
            super::super::UnityValue::Pointer(pointer) if pointer.is_null() => continue,
            super::super::UnityValue::Pointer(pointer) => pointer,
            _ => {
                unresolved_preload_table_indices.push(preload_table_index);
                unresolved_preload_pointers.push(PreloadUnresolvedPointerEvidence {
                    preload_table_index,
                    value_kind: unity_value_kind(value).to_string(),
                    pointer: None,
                });
                continue;
            }
        };
        let key = match env.resolve_pointer(pointer) {
            Ok(key) => key,
            Err(err) => {
                let pointer_evidence = unresolved_pointer_evidence(env, pointer, err);
                if is_retained_unity_system_pointer(env, pointer) {
                    retained_external_preload_pointers.push(
                        PreloadRetainedExternalPointerEvidence {
                            preload_table_index,
                            pointer: pointer_evidence,
                        },
                    );
                    continue;
                }
                unresolved_preload_table_indices.push(preload_table_index);
                unresolved_preload_pointers.push(PreloadUnresolvedPointerEvidence {
                    preload_table_index,
                    value_kind: "pointer".to_string(),
                    pointer: Some(pointer_evidence),
                });
                continue;
            }
        };
        let key = (key.asset, key.path_id);
        let key_evidence = match resolved_object_key_evidence(env, key) {
            Ok(evidence) => evidence,
            Err(err) => {
                unresolved_preload_table_indices.push(preload_table_index);
                unresolved_preload_pointers.push(PreloadUnresolvedPointerEvidence {
                    preload_table_index,
                    value_kind: "pointer".to_string(),
                    pointer: Some(unresolved_pointer_evidence(
                        env,
                        pointer,
                        format!("{}: {}", err.code, err.detail),
                    )),
                });
                continue;
            }
        };
        preload_indices_by_key
            .entry(key)
            .or_default()
            .push(preload_table_index);
        resolved_preload_pointers.push(PreloadResolvedPointerEvidence {
            preload_table_index,
            key: key_evidence,
        });
    }

    let mut roots = preload_indices_by_key
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    roots.insert(container_key);
    let mut pending = roots.iter().rev().copied().collect::<Vec<_>>();
    let mut closure = BTreeSet::new();
    let mut retained_external_closure_pointers = Vec::new();
    let mut closure_unresolved_pointers = Vec::new();
    let mut closure_unreadable_object_count = 0usize;
    while let Some(key) = pending.pop() {
        if !closure.insert(key) {
            continue;
        }
        let Some(asset) = env.assets.get(key.0) else {
            closure_unreadable_object_count += 1;
            continue;
        };
        let Some(info) = asset.objects.get(&key.1) else {
            closure_unreadable_object_count += 1;
            continue;
        };
        let Ok(body) = asset.read_object(key.0, info) else {
            closure_unreadable_object_count += 1;
            continue;
        };
        let mut pointers = Vec::new();
        crate::collect_value_pointers(&body, &mut pointers);
        let source_object = resolved_object_key_evidence(env, key)
            .expect("readable closure object disappeared from its Unity environment");
        let mut children = Vec::new();
        for (pointer_ordinal, pointer) in pointers.into_iter().enumerate() {
            let child = match env.resolve_pointer(&pointer) {
                Ok(child) => child,
                Err(err) => {
                    let pointer_evidence = unresolved_pointer_evidence(env, &pointer, err);
                    if is_retained_unity_system_pointer(env, &pointer) {
                        retained_external_closure_pointers.push(
                            ClosureRetainedExternalPointerEvidence {
                                source_object: source_object.clone(),
                                pointer_ordinal,
                                pointer: pointer_evidence,
                            },
                        );
                        continue;
                    }
                    closure_unresolved_pointers.push(ClosureUnresolvedPointerEvidence {
                        source_object: source_object.clone(),
                        pointer_ordinal,
                        pointer: pointer_evidence,
                    });
                    continue;
                }
            };
            let child = (child.asset, child.path_id);
            if let Err(err) = resolved_object_key_evidence(env, child) {
                closure_unresolved_pointers.push(ClosureUnresolvedPointerEvidence {
                    source_object: source_object.clone(),
                    pointer_ordinal,
                    pointer: unresolved_pointer_evidence(
                        env,
                        &pointer,
                        format!("{}: {}", err.code, err.detail),
                    ),
                });
                continue;
            }
            children.push(child);
        }
        for child in children.into_iter().rev() {
            if !closure.contains(&child) {
                pending.push(child);
            }
        }
    }

    let closure_keys = closure
        .iter()
        .filter_map(|key| resolved_object_key_evidence(env, *key).ok())
        .collect::<Vec<_>>();
    let mut preload_ownership = KfmPreloadOwnershipEvidence {
        exact_container_occurrence_count: 1,
        asset_bundle_asset_name: occurrence.asset_bundle_asset_name.clone(),
        asset_bundle_path_id: occurrence.asset_bundle_path_id,
        preload_index,
        preload_size,
        preload_range_start: preload_index,
        preload_range_end,
        preload_table_length: occurrence.preload_table.len(),
        dependency_archives: dependency_archives.to_vec(),
        container_pointer,
        resolved_preload_pointers,
        retained_external_preload_pointers,
        unresolved_preload_table_indices,
        unresolved_preload_pointers,
        closure_keys,
        retained_external_closure_pointers,
        closure_unresolved_pointer_count: closure_unresolved_pointers.len(),
        closure_unresolved_pointers,
        closure_unreadable_object_count,
        scanned_nif_route_group_count: 0,
        scanned_nif_container_occurrence_count: 0,
        resolved_nif_container_pointer_count: 0,
        nif_pointer_identity_match_count: 0,
        matched_nif_routes: Vec::new(),
    };

    if !preload_ownership
        .unresolved_preload_table_indices
        .is_empty()
    {
        return Err(KfmResolveError::new(
            "kfmPreloadPointersUnresolved",
            format!(
                "{} entries in the exact KFM preload range could not be resolved",
                preload_ownership.unresolved_preload_table_indices.len()
            ),
        )
        .with_pointer_traversal(pointer_traversal)
        .with_preload_ownership(preload_ownership));
    }
    if preload_ownership.closure_unreadable_object_count != 0 {
        return Err(KfmResolveError::new(
            "kfmPreloadClosureObjectsUnreadable",
            format!(
                "{} resolved objects in the preload closure could not be read",
                preload_ownership.closure_unreadable_object_count
            ),
        )
        .with_pointer_traversal(pointer_traversal)
        .with_preload_ownership(preload_ownership));
    }
    if preload_ownership.closure_unresolved_pointer_count != 0 {
        return Err(KfmResolveError::new(
            "kfmPreloadClosurePointersUnresolved",
            format!(
                "{} non-null pointers in the preload closure could not be resolved",
                preload_ownership.closure_unresolved_pointer_count
            ),
        )
        .with_pointer_traversal(pointer_traversal)
        .with_preload_ownership(preload_ownership));
    }

    let preload_keys = preload_indices_by_key
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let self_contained_proof = if pointer_traversal.container_object_type == "GameObject" {
        Some(
            prove_self_contained_game_object(env, container_key, &preload_keys).map_err(
                |detail| {
                    KfmResolveError::new("kfmSelfContainedGameObjectProofFailed", detail)
                        .with_pointer_traversal(pointer_traversal.clone())
                        .with_preload_ownership(preload_ownership.clone())
                },
            )?,
        )
    } else {
        None
    };
    let ownership_closure = self_contained_proof
        .as_ref()
        .map(|proof| &proof.root_closure)
        .unwrap_or(&closure);

    let mut nif_groups = BTreeMap::<String, NifContainerGroup>::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let body = asset.read_object(asset_index, info).map_err(|err| {
                KfmResolveError::new(
                    "kfmNifContainerScanFailed",
                    format!(
                        "failed to read {} AssetBundle #{}: {err}",
                        asset.name, info.path_id
                    ),
                )
                .with_pointer_traversal(pointer_traversal.clone())
                .with_preload_ownership(preload_ownership.clone())
            })?;
            for entry in super::super::value_array(body.get("m_Container")) {
                let Some((path, metadata)) = super::super::pair_name_value(entry) else {
                    continue;
                };
                let normalized_route = normalize_logical_model_route(path);
                if !normalized_route.ends_with(".nif") {
                    continue;
                }
                let group = nif_groups.entry(normalized_route).or_default();
                group.exact_routes.insert(path.to_string());
                group.occurrence_count += 1;
                let Some(pointer) = metadata
                    .get("asset")
                    .and_then(super::super::UnityValue::as_pointer)
                else {
                    group.unresolved_occurrence_count += 1;
                    continue;
                };
                let Ok(key) = env.resolve_pointer(pointer) else {
                    group.unresolved_occurrence_count += 1;
                    continue;
                };
                let key = (key.asset, key.path_id);
                if resolved_object_key_evidence(env, key).is_err() {
                    group.unresolved_occurrence_count += 1;
                    continue;
                }
                group.resolved.push(NifContainerOccurrence {
                    exact_route: path.to_string(),
                    container_asset_name: asset.name.clone(),
                    asset_bundle_path_id: info.path_id,
                    key,
                });
            }
        }
    }

    preload_ownership.scanned_nif_route_group_count = nif_groups.len();
    preload_ownership.scanned_nif_container_occurrence_count = nif_groups
        .values()
        .map(|group| group.occurrence_count)
        .sum();
    preload_ownership.resolved_nif_container_pointer_count =
        nif_groups.values().map(|group| group.resolved.len()).sum();
    let mut nif_ambiguity = None;
    let mut matched_nif_routes = Vec::new();
    for (normalized_route, group) in nif_groups {
        let matching = group
            .resolved
            .iter()
            .filter(|occurrence| ownership_closure.contains(&occurrence.key))
            .collect::<Vec<_>>();
        if matching.is_empty() {
            continue;
        }
        let distinct_keys = group
            .resolved
            .iter()
            .map(|occurrence| occurrence.key)
            .collect::<BTreeSet<_>>();
        if group.exact_routes.len() != 1
            || group.occurrence_count != 1
            || group.unresolved_occurrence_count != 0
            || distinct_keys.len() != 1
        {
            nif_ambiguity.get_or_insert_with(|| {
                (
                    "kfmPreloadNifPointerAmbiguous",
                    format!(
                        "NIF route '{normalized_route}' has {} exact spellings, {} container occurrences, {} unresolved occurrences, and {} distinct resolved pointer identities",
                        group.exact_routes.len(),
                        group.occurrence_count,
                        group.unresolved_occurrence_count,
                        distinct_keys.len()
                    ),
                )
            });
            continue;
        }
        let occurrence = matching[0];
        let matching_preload_table_indices = preload_indices_by_key
            .get(&occurrence.key)
            .cloned()
            .unwrap_or_default();
        let matched_by = if self_contained_proof.is_some() {
            if occurrence.key == container_key {
                "exactGameObjectRootPointerIdentity"
            } else {
                "exactGameObjectPointerClosureIdentity"
            }
        } else if occurrence.key == container_key {
            "exactContainerAssetPointerIdentity"
        } else if !matching_preload_table_indices.is_empty() {
            "assetBundlePreloadPointerIdentity"
        } else {
            "assetBundlePreloadClosurePointerIdentity"
        };
        matched_nif_routes.push(NifPointerOwnershipEvidence {
            exact_route: occurrence.exact_route.clone(),
            normalized_route,
            container_asset_name: occurrence.container_asset_name.clone(),
            asset_bundle_path_id: occurrence.asset_bundle_path_id,
            pointer: resolved_object_key_evidence(env, occurrence.key)
                .expect("validated NIF container pointer disappeared"),
            matched_by: matched_by.to_string(),
            matching_preload_table_indices,
        });
    }
    matched_nif_routes.sort_by(|left, right| {
        left.normalized_route
            .cmp(&right.normalized_route)
            .then_with(|| left.exact_route.cmp(&right.exact_route))
            .then_with(|| left.pointer.cmp(&right.pointer))
    });
    preload_ownership.nif_pointer_identity_match_count = matched_nif_routes.len();
    preload_ownership.matched_nif_routes = matched_nif_routes;

    if let Some((code, detail)) = nif_ambiguity {
        return Err(KfmResolveError::new(code, detail)
            .with_pointer_traversal(pointer_traversal)
            .with_preload_ownership(preload_ownership));
    }
    if preload_ownership.matched_nif_routes.is_empty() && self_contained_proof.is_none() {
        return Err(KfmResolveError::new(
            "kfmPreloadClosureHasNoNifPointerMatches",
            "the exact AssetBundle preload closure contains no NIF container pointer identity",
        )
        .with_pointer_traversal(pointer_traversal)
        .with_preload_ownership(preload_ownership));
    }

    let references = preload_ownership
        .matched_nif_routes
        .iter()
        .map(|route| route.exact_route.clone())
        .collect::<Vec<_>>();
    let self_contained_game_object = self_contained_proof.map(|proof| proof.evidence);
    Ok(KfmPayloadEvidence {
        ownership_evidence: if self_contained_game_object.is_some() {
            "exactGameObjectSubtree"
        } else {
            "assetBundlePreloadPointerIdentity"
        }
        .to_string(),
        payload_asset_name: None,
        payload_path_id: None,
        payload_object_type: None,
        byte_length: None,
        references: references.clone(),
        nif_references: references,
        pointer_traversal,
        preload_ownership: Some(preload_ownership),
        self_contained_game_object,
    })
}
