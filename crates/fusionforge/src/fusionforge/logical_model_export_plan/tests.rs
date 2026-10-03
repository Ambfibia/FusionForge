use super::*;
use crate::fusionforge::logical_model_catalog::catalog_logical_models_from_json;

fn catalog(source: &str) -> LogicalModelCatalog {
    catalog_logical_models_from_json(source, "fixture/cache/bundle-index.json").unwrap()
}

#[test]
fn unity_system_asset_refs_are_retained_without_weakening_bundle_resolution() {
    assert!(is_unity_system_asset_ref("library/unity default resources"));
    assert!(is_unity_system_asset_ref(r"Resources\unity_builtin_extra"));
    assert!(!is_unity_system_asset_ref(
        "CustomAssetBundle-unity default resources"
    ));
    assert!(!is_unity_system_asset_ref("archive:/CAB-123/CAB-123"));
}

fn evidence(references: &[&str]) -> KfmPayloadEvidence {
    KfmPayloadEvidence {
        ownership_evidence: "rawKfmPayload".to_string(),
        payload_asset_name: Some("PayloadAsset".to_string()),
        payload_path_id: Some(7),
        payload_object_type: Some("TextAsset".to_string()),
        byte_length: Some(64),
        references: references.iter().map(|value| value.to_string()).collect(),
        nif_references: Vec::new(),
        pointer_traversal: KfmPointerTraversalEvidence {
            exact_container_occurrence_count: 1,
            container_asset_name: "SceneAsset".to_string(),
            container_path_id: 11,
            container_object_type: "GameObject".to_string(),
            payload_asset_name: Some("PayloadAsset".to_string()),
            payload_path_id: Some(7),
            payload_object_type: Some("TextAsset".to_string()),
            pointer_nodes_visited: 3,
            pointer_cycle_guard: "visitedAssetIndexAndPathIdSet".to_string(),
            pointer_safety_budget: None,
            pointer_budget_exceeded: false,
            pointer_graph_exhausted: false,
        },
        preload_ownership: None,
        self_contained_game_object: None,
    }
}

fn preload_evidence(reference: &str) -> KfmPayloadEvidence {
    let key = ResolvedObjectKeyEvidence {
        asset_index: 0,
        asset_name: "AssetA".to_string(),
        path_id: 11,
        object_type: "GameObject".to_string(),
    };
    KfmPayloadEvidence {
        ownership_evidence: "assetBundlePreloadPointerIdentity".to_string(),
        payload_asset_name: None,
        payload_path_id: None,
        payload_object_type: None,
        byte_length: None,
        references: vec![reference.to_string()],
        nif_references: vec![reference.to_string()],
        pointer_traversal: KfmPointerTraversalEvidence {
            exact_container_occurrence_count: 1,
            container_asset_name: "AssetA".to_string(),
            container_path_id: 11,
            container_object_type: "GameObject".to_string(),
            payload_asset_name: None,
            payload_path_id: None,
            payload_object_type: None,
            pointer_nodes_visited: 1,
            pointer_cycle_guard: "visitedAssetIndexAndPathIdSet".to_string(),
            pointer_safety_budget: None,
            pointer_budget_exceeded: false,
            pointer_graph_exhausted: true,
        },
        preload_ownership: Some(KfmPreloadOwnershipEvidence {
            exact_container_occurrence_count: 1,
            asset_bundle_asset_name: "AssetA".to_string(),
            asset_bundle_path_id: 1,
            preload_index: 0,
            preload_size: 1,
            preload_range_start: 0,
            preload_range_end: 1,
            preload_table_length: 1,
            dependency_archives: Vec::new(),
            container_pointer: key.clone(),
            resolved_preload_pointers: vec![PreloadResolvedPointerEvidence {
                preload_table_index: 0,
                key: key.clone(),
            }],
            retained_external_preload_pointers: Vec::new(),
            unresolved_preload_table_indices: Vec::new(),
            unresolved_preload_pointers: Vec::new(),
            closure_keys: vec![key.clone()],
            retained_external_closure_pointers: Vec::new(),
            closure_unresolved_pointer_count: 0,
            closure_unresolved_pointers: Vec::new(),
            closure_unreadable_object_count: 0,
            scanned_nif_route_group_count: 1,
            scanned_nif_container_occurrence_count: 1,
            resolved_nif_container_pointer_count: 1,
            nif_pointer_identity_match_count: 1,
            matched_nif_routes: vec![NifPointerOwnershipEvidence {
                exact_route: reference.to_string(),
                normalized_route: normalize_logical_model_route(reference),
                container_asset_name: "AssetA".to_string(),
                asset_bundle_path_id: 1,
                pointer: key,
                matched_by: "assetBundlePreloadPointerIdentity".to_string(),
                matching_preload_table_indices: vec![0],
            }],
        }),
        self_contained_game_object: None,
    }
}

fn self_contained_evidence() -> KfmPayloadEvidence {
    let root = ResolvedObjectKeyEvidence {
        asset_index: 0,
        asset_name: "AssetA".to_string(),
        path_id: 11,
        object_type: "GameObject".to_string(),
    };
    let transform = ResolvedObjectKeyEvidence {
        asset_index: 0,
        asset_name: "AssetA".to_string(),
        path_id: 12,
        object_type: "Transform".to_string(),
    };
    let mut payload = evidence(&[]);
    payload.ownership_evidence = "exactGameObjectSubtree".to_string();
    payload.payload_asset_name = None;
    payload.payload_path_id = None;
    payload.payload_object_type = None;
    payload.byte_length = None;
    payload.pointer_traversal.payload_asset_name = None;
    payload.pointer_traversal.payload_path_id = None;
    payload.pointer_traversal.payload_object_type = None;
    payload.pointer_traversal.pointer_graph_exhausted = true;
    payload.self_contained_game_object = Some(SelfContainedGameObjectEvidence {
        proof: "exactGameObjectTransformSubtreeAndRootPointerClosureContainment".to_string(),
        root_game_object: root.clone(),
        root_transform: transform.clone(),
        root_name: "npc_fixture".to_string(),
        root_closure_key_count: 8,
        preload_core_key_count: 8,
        game_object_count: 1,
        transform_count: 1,
        hierarchy_edge_count: 0,
        component_count: 3,
        renderer_count: 1,
        skinned_mesh_renderer_count: 1,
        mesh_count: 1,
        material_count: 1,
        bone_count: 1,
        animation_component_count: 1,
        animation_clip_count: 1,
        null_parent_transform_count: 1,
        core_keys: vec![root, transform],
    });
    payload
}

fn fingerprint(marker: char, path_id: i64) -> LogicalModelPhysicalFingerprint {
    LogicalModelPhysicalFingerprint {
        serialized_asset_sha256: marker.to_string().repeat(64),
        path_id,
        object_type: "GameObject".to_string(),
    }
}

fn physical_lookup_fixture(
    exact_occurrences: Vec<Result<LogicalModelPhysicalFingerprint, String>>,
) -> (PhysicalEnvironmentRouteIndex, LogicalModelOccurrence) {
    let exact_route = "mob/body.nif".to_string();
    let normalized_route = normalize_logical_model_route(&exact_route);
    let owner = LogicalModelOwner {
        bundle_path: "C:/A.resourceFile".to_string(),
        bundle_name: "A.resourceFile".to_string(),
        asset_name: "AssetA".to_string(),
    };
    let route = PhysicalNormalizedRouteIndex {
        exact_spellings: BTreeSet::from([exact_route.clone()]),
        exact_occurrences: BTreeMap::from([(exact_route.clone(), exact_occurrences)]),
    };
    let index = PhysicalEnvironmentRouteIndex {
        owners: BTreeMap::from([(
            owner.asset_name.clone(),
            PhysicalOwnerAssetRouteIndex {
                serialized_asset_match_count: 1,
                routes: Ok(BTreeMap::from([(normalized_route.clone(), route)])),
            },
        )]),
        stats: PhysicalRouteIndexBuildStats::default(),
    };
    let occurrence = LogicalModelOccurrence {
        exact_route,
        normalized_route,
        kind: LogicalModelKind::Nif,
        owner,
    };
    (index, occurrence)
}

fn physical_resolutions<F>(
    catalog: &LogicalModelCatalog,
    mut fingerprint_for: F,
) -> BTreeMap<String, Result<LogicalModelResolvedRoute, LogicalModelPlanBlocker>>
where
    F: FnMut(&LogicalModelOccurrence) -> LogicalModelPhysicalFingerprint,
{
    catalog
        .route_groups
        .iter()
        .map(|group| {
            let proofs = catalog
                .occurrences
                .iter()
                .filter(|occurrence| occurrence.normalized_route == group.normalized_route)
                .map(|occurrence| LogicalModelOccurrenceFingerprint {
                    exact_route: occurrence.exact_route.clone(),
                    owner: occurrence.owner.clone(),
                    fingerprint: fingerprint_for(occurrence),
                })
                .collect::<Vec<_>>();
            (
                group.normalized_route.clone(),
                resolve_physical_route_group(group, &proofs),
            )
        })
        .collect()
}

#[test]
fn physical_route_index_parses_each_container_once_and_preserves_lookup_results() {
    let root_fingerprint = fingerprint('a', 41);
    let body_fingerprint = fingerprint('b', 42);
    let required_routes =
        BTreeSet::from(["mob/root.kfm".to_string(), "mob/body.nif".to_string()]);
    let mut stats = PhysicalRouteIndexBuildStats::default();
    let mut parse_calls = 0;
    let routes = build_physical_owner_route_index(
        [7_i64],
        &required_routes,
        &mut stats,
        |asset_bundle_path_id| {
            parse_calls += 1;
            assert_eq!(asset_bundle_path_id, 7);
            Ok(vec![
                PhysicalIndexedContainerOccurrence {
                    exact_route: "mob/root.kfm".to_string(),
                    fingerprint: Ok(root_fingerprint.clone()),
                },
                PhysicalIndexedContainerOccurrence {
                    exact_route: "mob/body.nif".to_string(),
                    fingerprint: Ok(body_fingerprint.clone()),
                },
                PhysicalIndexedContainerOccurrence {
                    exact_route: "texture/not-a-model.png".to_string(),
                    fingerprint: Err("irrelevant entry must not affect lookup".to_string()),
                },
            ])
        },
    )
    .expect("fixture container index should build");
    assert_eq!(parse_calls, 1);
    assert_eq!(stats.asset_bundle_container_parse_count, 1);

    let index = PhysicalEnvironmentRouteIndex {
        owners: BTreeMap::from([(
            "AssetA".to_string(),
            PhysicalOwnerAssetRouteIndex {
                serialized_asset_match_count: 1,
                routes: Ok(routes),
            },
        )]),
        stats,
    };
    let owner = LogicalModelOwner {
        bundle_path: "C:/A.resourceFile".to_string(),
        bundle_name: "A.resourceFile".to_string(),
        asset_name: "AssetA".to_string(),
    };
    let root = LogicalModelOccurrence {
        exact_route: "mob/root.kfm".to_string(),
        normalized_route: "mob/root.kfm".to_string(),
        kind: LogicalModelKind::Kfm,
        owner: owner.clone(),
    };
    let body = LogicalModelOccurrence {
        exact_route: "mob/body.nif".to_string(),
        normalized_route: "mob/body.nif".to_string(),
        kind: LogicalModelKind::Nif,
        owner,
    };

    assert_eq!(
        physical_fingerprint_for_occurrence(&index, &root),
        Ok(root_fingerprint.clone())
    );
    assert_eq!(
        physical_fingerprint_for_occurrence(&index, &body),
        Ok(body_fingerprint)
    );
    assert_eq!(
        physical_fingerprint_for_occurrence(&index, &root),
        Ok(root_fingerprint)
    );
    assert_eq!(
        index.stats.asset_bundle_container_parse_count, 1,
        "all indexed occurrence lookups must reuse the single container parse"
    );
}

#[test]
fn duplicate_identical_exact_container_targets_are_accepted() {
    let expected = fingerprint('a', 41);
    let (index, occurrence) =
        physical_lookup_fixture(vec![Ok(expected.clone()), Ok(expected.clone())]);

    assert_eq!(
        physical_fingerprint_for_occurrence(&index, &occurrence),
        Ok(expected)
    );
}

#[test]
fn duplicate_distinct_exact_container_targets_are_blocked() {
    let (index, occurrence) =
        physical_lookup_fixture(vec![Ok(fingerprint('a', 41)), Ok(fingerprint('b', 42))]);

    let error = physical_fingerprint_for_occurrence(&index, &occurrence)
        .expect_err("different complete fingerprints must stay fail-closed");
    assert_eq!(error.code_suffix, "PhysicalDuplicateTargetConflict");
    assert!(error.detail.contains("2 exact container occurrences"));
    assert!(error.detail.contains("2 distinct"));

    let catalog = catalog(
        r#"{"bundles":[{"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["mob/body.nif"]}]}]}"#,
    );
    let blocker =
        route_group_blocker(&catalog.route_groups[0], error.code_suffix, error.detail);
    assert_eq!(blocker.code, "nifPhysicalDuplicateTargetConflict");
}

#[test]
fn one_failed_duplicate_pointer_blocks_entire_route() {
    let (index, occurrence) = physical_lookup_fixture(vec![
        Ok(fingerprint('a', 41)),
        Err("fixture PPtr failure".to_string()),
    ]);

    let error = physical_fingerprint_for_occurrence(&index, &occurrence)
        .expect_err("one unresolved duplicate target must reject the whole route");
    assert_eq!(error.code_suffix, "PhysicalDuplicateTargetUnresolved");
    assert!(error.detail.contains("occurrence 2/2"));
    assert!(error.detail.contains("fixture PPtr failure"));

    let catalog = catalog(
        r#"{"bundles":[{"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["mob/body.nif"]}]}]}"#,
    );
    let blocker =
        route_group_blocker(&catalog.route_groups[0], error.code_suffix, error.detail);
    assert_eq!(blocker.code, "nifPhysicalDuplicateTargetUnresolved");
}

#[test]
fn kfm_referenced_nif_is_owned_and_never_standalone() {
    let catalog = catalog(
        r#"{"bundles":[{"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["mob/root.kfm","mob/body.nif","wear/hat.nif"]}]}]}"#,
    );
    let plan = build_logical_model_export_plan(
        &catalog,
        LogicalModelExportPlanOptions::default(),
        |_| Ok(evidence(&["mob/body.nif", "anim/idle.kf"])),
    );

    assert!(plan.ownership_scan_complete);
    assert_eq!(plan.logical_roots.len(), 1);
    assert_eq!(plan.logical_roots[0].status, LogicalModelRootStatus::Ready);
    assert_eq!(plan.owned_parts.len(), 1);
    assert_eq!(plan.owned_parts[0].nif.exact_route, "mob/body.nif");
    assert_eq!(plan.standalone_nifs.len(), 1);
    assert_eq!(plan.standalone_nifs[0].nif.exact_route, "wear/hat.nif");
    assert!(!plan
        .standalone_nifs
        .iter()
        .any(|entry| entry.nif.exact_route == "mob/body.nif"));
}

#[test]
fn identical_physical_multi_owner_kfm_alias_is_accepted_with_canonical_owner() {
    let catalog = catalog(
        r#"{"bundles":[
            {"path":"C:/B.resourceFile","name":"B.resourceFile","assets":[{"name":"AssetB","containerPaths":["mob/root.kfm"]}]},
            {"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["mob/root.kfm"]}]}
        ]}"#,
    );
    let resolutions = physical_resolutions(&catalog, |_| fingerprint('a', 41));
    let plan = build_logical_model_export_plan_with_resolved_routes(
        &catalog,
        LogicalModelExportPlanOptions::default(),
        &resolutions,
        |_| Ok(self_contained_evidence()),
    );

    assert!(plan.ownership_scan_complete);
    assert_eq!(plan.logical_roots.len(), 1);
    assert_eq!(
        plan.logical_roots[0].kfm.owner.bundle_path,
        "C:/A.resourceFile"
    );
    assert_eq!(plan.counts.physical_alias_route_count, 1);
    assert_eq!(plan.counts.physical_alias_owner_count, 1);
    assert_eq!(plan.physical_aliases.len(), 1);
    assert_eq!(
        plan.physical_aliases[0].alias_owners[0].bundle_path,
        "C:/B.resourceFile"
    );
    assert_eq!(
        plan.physical_aliases[0].basis,
        "serializedAssetSha256PathIdAndObjectType"
    );
}

#[test]
fn differing_physical_multi_owner_fingerprints_remain_blocked() {
    let catalog = catalog(
        r#"{"bundles":[
            {"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["mob/root.kfm"]}]},
            {"path":"C:/B.resourceFile","name":"B.resourceFile","assets":[{"name":"AssetB","containerPaths":["mob/root.kfm"]}]}
        ]}"#,
    );
    let resolutions = physical_resolutions(&catalog, |occurrence| {
        if occurrence.owner.bundle_path == "C:/A.resourceFile" {
            fingerprint('a', 41)
        } else {
            fingerprint('b', 41)
        }
    });
    let mut kfm_reads = 0;
    let plan = build_logical_model_export_plan_with_resolved_routes(
        &catalog,
        LogicalModelExportPlanOptions::default(),
        &resolutions,
        |_| {
            kfm_reads += 1;
            Ok(self_contained_evidence())
        },
    );

    assert_eq!(kfm_reads, 0, "a physical conflict must never read a KFM");
    assert!(!plan.ownership_scan_complete);
    assert!(plan.logical_roots.is_empty());
    assert!(plan.physical_aliases.is_empty());
    assert!(plan
        .blockers
        .iter()
        .any(|blocker| blocker.code == "kfmPhysicalTargetConflict"));
}

#[test]
fn physically_aliased_owned_nif_is_never_scattered_as_standalone() {
    let catalog = catalog(
        r#"{"bundles":[
            {"path":"C:/Root.resourceFile","name":"Root.resourceFile","assets":[{"name":"RootAsset","containerPaths":["mob/root.kfm","wear/hat.nif"]}]},
            {"path":"C:/B.resourceFile","name":"B.resourceFile","assets":[{"name":"AssetB","containerPaths":["mob/body.nif"]}]},
            {"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["mob/body.nif"]}]}
        ]}"#,
    );
    let resolutions = physical_resolutions(&catalog, |occurrence| {
        if occurrence.normalized_route == "mob/body.nif" {
            fingerprint('c', 73)
        } else {
            fingerprint('d', 11)
        }
    });
    let plan = build_logical_model_export_plan_with_resolved_routes(
        &catalog,
        LogicalModelExportPlanOptions::default(),
        &resolutions,
        |_| Ok(evidence(&["mob/body.nif"])),
    );

    assert_eq!(plan.counts.catalog_kfm_route_count, 1);
    assert_eq!(plan.counts.catalog_nif_route_count, 2);
    assert_eq!(plan.counts.owned_nif_route_count, 1);
    assert_eq!(plan.counts.standalone_nif_count, 1);
    assert_eq!(plan.counts.physical_alias_route_count, 1);
    assert_eq!(plan.counts.physical_alias_owner_count, 1);
    assert_eq!(plan.owned_parts.len(), 1);
    assert_eq!(
        plan.owned_parts[0].nif.owner.bundle_path,
        "C:/A.resourceFile"
    );
    assert!(!plan
        .standalone_nifs
        .iter()
        .any(|entry| entry.nif.normalized_route == "mob/body.nif"));
    assert_eq!(plan.standalone_nifs[0].nif.normalized_route, "wear/hat.nif");
}

#[test]
fn ambiguous_kfm_owner_is_not_read_and_withholds_all_standalone_nifs() {
    let catalog = catalog(
        r#"{"bundles":[
            {"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["mob/root.kfm","wear/hat.nif"]}]},
            {"path":"C:/B.resourceFile","name":"B.resourceFile","assets":[{"name":"AssetB","containerPaths":["mob/root.kfm"]}]}
        ]}"#,
    );
    let mut calls = 0;
    let plan = build_logical_model_export_plan(
        &catalog,
        LogicalModelExportPlanOptions::default(),
        |_| {
            calls += 1;
            Ok(evidence(&["mob/body.nif"]))
        },
    );

    assert_eq!(calls, 0, "ambiguous KFM payload must never be read");
    assert!(!plan.ownership_scan_complete);
    assert!(plan.logical_roots.is_empty());
    assert!(plan.standalone_nifs.is_empty());
    assert!(plan
        .blockers
        .iter()
        .any(|blocker| blocker.code == "kfmOwnerAmbiguous"));
    assert!(plan
        .blockers
        .iter()
        .any(|blocker| blocker.code == "standaloneNifProofIncomplete"));
}

#[test]
fn parse_failure_and_route_collision_fail_closed() {
    let collision_catalog = catalog(
        r#"{"bundles":[{"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["Mob/Root.KFM","mob\\root.kfm","wear/hat.nif"]}]}]}"#,
    );
    let mut calls = 0;
    let collision_plan = build_logical_model_export_plan(
        &collision_catalog,
        LogicalModelExportPlanOptions::default(),
        |_| {
            calls += 1;
            Ok(evidence(&["mob/body.nif"]))
        },
    );
    assert_eq!(calls, 0, "colliding KFM route must never be read");
    assert!(collision_plan.standalone_nifs.is_empty());
    assert!(collision_plan
        .blockers
        .iter()
        .any(|blocker| blocker.code == "kfmRouteCollision"));

    let catalog = catalog(
        r#"{"bundles":[{"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["mob/root.kfm","wear/hat.nif"]}]}]}"#,
    );
    let failed_plan = build_logical_model_export_plan(
        &catalog,
        LogicalModelExportPlanOptions::default(),
        |_| Err(KfmResolveError::new("kfmPayloadBytesMissing", "fixture")),
    );
    assert!(!failed_plan.ownership_scan_complete);
    assert!(failed_plan.logical_roots.is_empty());
    assert!(failed_plan.standalone_nifs.is_empty());
}

#[test]
fn ambiguous_referenced_nif_blocks_root_and_is_never_scattered() {
    let catalog = catalog(
        r#"{"bundles":[
            {"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["mob/root.kfm","mob/body.nif","wear/hat.nif"]}]},
            {"path":"C:/B.resourceFile","name":"B.resourceFile","assets":[{"name":"AssetB","containerPaths":["mob/body.nif"]}]}
        ]}"#,
    );
    let plan = build_logical_model_export_plan(
        &catalog,
        LogicalModelExportPlanOptions::default(),
        |_| Ok(evidence(&["mob/body.nif"])),
    );

    assert!(plan.ownership_scan_complete, "the KFM itself was parsed");
    assert_eq!(
        plan.logical_roots[0].status,
        LogicalModelRootStatus::Blocked
    );
    assert!(plan.owned_parts.is_empty());
    assert!(plan
        .unresolved
        .iter()
        .any(|entry| entry.code == "referencedNifOwnerAmbiguous"));
    assert!(!plan
        .standalone_nifs
        .iter()
        .any(|entry| entry.nif.normalized_route == "mob/body.nif"));
    assert!(plan
        .standalone_nifs
        .iter()
        .any(|entry| entry.nif.normalized_route == "wear/hat.nif"));
}

#[test]
fn preload_pointer_identity_groups_nif_under_kfm_without_raw_payload() {
    let catalog = catalog(
        r#"{"bundles":[{"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["mob/root.kfm","mob/body.nif"]}]}]}"#,
    );
    let plan = build_logical_model_export_plan(
        &catalog,
        LogicalModelExportPlanOptions::default(),
        |_| Ok(preload_evidence("mob/body.nif")),
    );

    assert_eq!(plan.logical_roots.len(), 1);
    assert_eq!(plan.owned_parts.len(), 1);
    assert!(plan.standalone_nifs.is_empty());
    assert_eq!(
        plan.logical_roots[0].payload.ownership_evidence,
        "assetBundlePreloadPointerIdentity"
    );
    assert!(plan.logical_roots[0].payload.payload_asset_name.is_none());
    assert_eq!(plan.owned_parts[0].reference_exact_route, "mob/body.nif");
}

#[test]
fn sparse_preload_hierarchy_membership_is_accepted() {
    let hierarchy_game_objects = BTreeSet::from([(0, 1), (0, 2), (0, 3)]);
    let hierarchy_transforms = BTreeSet::from([(0, 11), (0, 12), (0, 13)]);
    let preload_game_objects = BTreeSet::from([(0, 1)]);
    let preload_transforms = BTreeSet::from([(0, 11)]);

    assert!(validate_sparse_preload_hierarchy_membership(
        &preload_game_objects,
        &hierarchy_game_objects,
        &preload_transforms,
        &hierarchy_transforms,
    )
    .is_ok());
}

#[test]
fn sparse_preload_rejects_a_foreign_game_object() {
    let error = validate_sparse_preload_hierarchy_membership(
        &BTreeSet::from([(0, 1), (9, 99)]),
        &BTreeSet::from([(0, 1), (0, 2)]),
        &BTreeSet::from([(0, 11)]),
        &BTreeSet::from([(0, 11), (0, 12)]),
    )
    .unwrap_err();

    assert!(error.contains("GameObjects outside the root hierarchy"));
    assert!(error.contains("(9, 99)"));
}

#[test]
fn sparse_preload_rejects_a_foreign_transform() {
    let error = validate_sparse_preload_hierarchy_membership(
        &BTreeSet::from([(0, 1)]),
        &BTreeSet::from([(0, 1), (0, 2)]),
        &BTreeSet::from([(0, 11), (9, 99)]),
        &BTreeSet::from([(0, 11), (0, 12)]),
    )
    .unwrap_err();

    assert!(error.contains("Transforms outside the root hierarchy"));
    assert!(error.contains("(9, 99)"));
}

#[test]
fn exact_self_contained_game_object_is_ready_without_inventing_a_nif_part() {
    let catalog = catalog(
        r#"{"bundles":[{"path":"C:/A.resourceFile","name":"A.resourceFile","assets":[{"name":"AssetA","containerPaths":["mob/npc_fixture.kfm","wear/hat.nif"]}]}]}"#,
    );
    let plan = build_logical_model_export_plan(
        &catalog,
        LogicalModelExportPlanOptions::default(),
        |_| Ok(self_contained_evidence()),
    );

    assert!(plan.ownership_scan_complete);
    assert_eq!(plan.logical_roots.len(), 1);
    assert_eq!(plan.logical_roots[0].status, LogicalModelRootStatus::Ready);
    assert_eq!(plan.logical_roots[0].owned_nif_routes, Vec::<String>::new());
    assert!(plan.owned_parts.is_empty());
    assert_eq!(plan.standalone_nifs.len(), 1);
    assert_eq!(plan.standalone_nifs[0].nif.exact_route, "wear/hat.nif");
    assert_eq!(
        plan.logical_roots[0].payload.ownership_evidence,
        "exactGameObjectSubtree"
    );
    assert!(plan.logical_roots[0]
        .payload
        .self_contained_game_object
        .is_some());
    assert!(!plan
        .blockers
        .iter()
        .any(|blocker| blocker.code == "kfmNoNifReferences"));
}

#[test]
fn exact_kfm_reference_parser_does_not_drop_parts_after_preview_limit() {
    let mut bytes = Vec::new();
    for index in 0..140 {
        bytes.extend_from_slice(format!("mob/part_{index:03}.nif").as_bytes());
        bytes.push(0);
    }
    let references = crate::kfm_reference_paths_exact(&bytes);
    assert_eq!(references.len(), 140);
    assert_eq!(
        references.first().map(String::as_str),
        Some("mob/part_000.nif")
    );
    assert_eq!(
        references.last().map(String::as_str),
        Some("mob/part_139.nif")
    );
}
