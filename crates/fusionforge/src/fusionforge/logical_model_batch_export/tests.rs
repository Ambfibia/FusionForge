use super::*;

fn test_candidate(route: &str, true_name: &str) -> PreparedCandidate {
    let owner = LogicalModelOwner {
        bundle_path: "D:/build/DongResources.resourceFile".to_string(),
        bundle_name: "DongResources.resourceFile".to_string(),
        asset_name: "CustomAssetBundle".to_string(),
    };
    let key = ResolvedObjectKeyEvidence {
        asset_index: 2,
        asset_name: owner.asset_name.clone(),
        path_id: 41,
        object_type: "GameObject".to_string(),
    };
    let route_stem = route
        .strip_prefix("mob/")
        .and_then(|value| value.strip_suffix(".kfm"))
        .unwrap();
    PreparedCandidate {
        family: "mob".to_string(),
        semantic_directories: vec![route_stem.to_string()],
        true_name: true_name.to_string(),
        relative_source_path: PathBuf::from(format!(
            "mob/{route_stem}/{true_name}.source.json"
        )),
        exact_route: route.to_string(),
        normalized_route: route.to_string(),
        bundle_path: owner.bundle_path.clone(),
        ownership: LogicalModelSourceOwnershipEvidence {
            ownership_evidence: "exactGameObjectSubtree".to_string(),
            self_contained_proof: "oneParentlessTransformRoot".to_string(),
            canonical_owner: owner.clone(),
            physical_alias_owners: vec![LogicalModelOwner {
                bundle_path: "D:/build/alias.resourceFile".to_string(),
                bundle_name: "alias.resourceFile".to_string(),
                asset_name: "AliasAsset".to_string(),
            }],
            physical_fingerprint: None,
            root_game_object: key.clone(),
            root_transform: ResolvedObjectKeyEvidence {
                object_type: "Transform".to_string(),
                ..key
            },
        },
    }
}

fn valid_test_source(candidate: &PreparedCandidate) -> JsonValue {
    serde_json::json!({
        "schema": "ffone.logical-model-source.v1",
        "selectionMode": "exact-container-route",
        "status": "ready",
        "logicalName": candidate.true_name,
        "exactContainerRoute": candidate.exact_route,
        "modelHierarchy": {
            "roots": [{
                "name": candidate.true_name
            }]
        },
        "matchedPaths": [candidate.exact_route],
        "warnings": []
    })
}

#[test]
fn root_selection_never_exports_blocked_unresolved_or_unproven_roots() {
    assert_eq!(
        classify_root(LogicalModelRootStatus::Ready, true, true),
        RootEligibility::Candidate
    );
    assert_eq!(
        classify_root(LogicalModelRootStatus::Blocked, true, true),
        RootEligibility::Blocked
    );
    assert_eq!(
        classify_root(LogicalModelRootStatus::Ready, true, false),
        RootEligibility::Blocked
    );
    assert_eq!(
        classify_root(LogicalModelRootStatus::Ready, false, true),
        RootEligibility::SkippedNotSelfContained
    );
}

#[test]
fn family_and_true_name_path_uses_native_minimal_windows_contract() {
    assert_eq!(source_family("mob/npc_simon.kfm").unwrap(), "mob");
    assert_eq!(
        source_semantic_directories("mob", "mob/npc_iceking.kfm").unwrap(),
        vec!["npc_iceking"]
    );
    assert_eq!(
        slash_path(&source_relative_path("mob", &["npc_iceking".into()], "NPC:Simon").unwrap()),
        "mob/npc_iceking/NPC_Simon.source.json"
    );
    assert_eq!(
        source_semantic_directories("mob", "mob/variant/npc_iceking.kfm").unwrap(),
        vec!["variant", "npc_iceking"]
    );
}

#[test]
fn case_insensitive_minimal_name_collision_fails_before_writes() {
    let first =
        slash_path(&source_relative_path("mob", &["npc_iceking".into()], "NPC:Simon").unwrap());
    let second =
        slash_path(&source_relative_path("MOB", &["NPC_ICEKING".into()], "npc_simon").unwrap());
    let error = preflight_output_paths(&[
        (first, "mob/fusion_iceking.kfm".to_string()),
        (second, "mob/npc_iceking.kfm".to_string()),
    ])
    .unwrap_err();
    assert!(error.contains("case-insensitive"));
    assert!(error.contains("fusion_iceking.kfm"));
    assert!(error.contains("npc_iceking.kfm"));
}

#[test]
fn unicode_compatibility_equivalent_paths_collide_in_preflight() {
    let error = preflight_output_paths(&[
        (
            "mob/npc_simon/\u{212A}ing.source.json".to_string(),
            "mob/first.kfm".to_string(),
        ),
        (
            "mob/npc_simon/king.source.json".to_string(),
            "mob/second.kfm".to_string(),
        ),
    ])
    .unwrap_err();
    assert!(error.contains("first.kfm"));
    assert!(error.contains("second.kfm"));
}

#[test]
fn source_stage_ambiguity_blocks_only_exact_root_and_preserves_siblings() {
    let blocked_candidate = test_candidate("mob/mob_lightbulb.kfm", "mob_lightbulb");
    let exported_candidate = test_candidate("mob/mob_bshell.kfm", "mob_bshell");
    let candidates = vec![blocked_candidate.clone(), exported_candidate.clone()];
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let staging_root = std::env::temp_dir().join(format!(
        "ffclient-logical-model-source-stage-test-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&staging_root).unwrap();

    let staged = stage_sources_with(&candidates, &staging_root, |candidate| {
        if candidate.exact_route == "mob/mob_lightbulb.kfm" {
            Err(
                "AnimationClip Asset#238 (nif-default) animation path \"mob_lightbulb\" is ambiguous under the true root"
                    .to_string(),
            )
        } else {
            Ok(valid_test_source(candidate))
        }
    })
    .unwrap();

    assert_eq!(staged.exported.len(), 1);
    assert_eq!(
        staged.exported[0].exact_route,
        exported_candidate.exact_route
    );
    assert_eq!(staged.blocked.len(), 1);
    let blocked = &staged.blocked[0];
    assert_eq!(blocked.code, "sourceStageExportFailed");
    assert_eq!(
        blocked.exact_route.as_deref(),
        Some(blocked_candidate.exact_route.as_str())
    );
    assert_eq!(
        blocked.normalized_route.as_deref(),
        Some(blocked_candidate.normalized_route.as_str())
    );
    assert!(blocked.detail.contains("ambiguous under the true root"));
    assert_eq!(blocked.owners.len(), 2);
    assert!(!staging_root
        .join(&blocked_candidate.relative_source_path)
        .exists());
    assert!(staging_root
        .join(&exported_candidate.relative_source_path)
        .is_file());

    fs::remove_dir_all(staging_root).unwrap();
}

#[test]
fn distinct_true_name_paths_are_sorted_without_hash_or_path_id_suffixes() {
    let mut paths = vec![
        (
            slash_path(
                &source_relative_path("nano", &["nano_grim".into()], "nano_grim").unwrap(),
            ),
            "nano/nano_grim.kfm".to_string(),
        ),
        (
            slash_path(
                &source_relative_path("mob", &["npc_iceking".into()], "npc_simon").unwrap(),
            ),
            "mob/npc_iceking.kfm".to_string(),
        ),
    ];
    paths.sort();
    preflight_output_paths(&paths).unwrap();
    assert_eq!(paths[0].0, "mob/npc_iceking/npc_simon.source.json");
    assert_eq!(paths[1].0, "nano/nano_grim/nano_grim.source.json");
    assert!(!paths.iter().any(|(path, _)| path.contains("PathID")));
    assert!(!paths.iter().any(|(path, _)| path.contains("--")));
}

#[test]
fn manifest_is_a_sibling_and_binds_exact_route_to_hashed_source_and_ownership() {
    let output_root = Path::new("target/logical-model-sources");
    assert_eq!(
        batch_manifest_path(output_root).unwrap(),
        PathBuf::from("target/logical-model-sources.manifest.json")
    );

    let owner = LogicalModelOwner {
        bundle_path: "D:/build/DongResources.resourceFile".to_string(),
        bundle_name: "DongResources.resourceFile".to_string(),
        asset_name: "CustomAssetBundle".to_string(),
    };
    let key = ResolvedObjectKeyEvidence {
        asset_index: 2,
        asset_name: owner.asset_name.clone(),
        path_id: 41,
        object_type: "GameObject".to_string(),
    };
    let manifest = LogicalModelSourceBatchManifest {
        schema: LOGICAL_MODEL_SOURCE_BATCH_SCHEMA,
        status: "complete",
        source_index_path: "project/cache/bundle-index.json".to_string(),
        source_root: output_root.to_string_lossy().to_string(),
        manifest_path: "target/logical-model-sources.manifest.json".to_string(),
        full_plan_ownership_scan_complete: false,
        counts: LogicalModelSourceBatchCounts {
            planned_logical_root_count: 3,
            ready_self_contained_root_count: 1,
            exported_count: 1,
            skipped_root_count: 1,
            blocked_entry_count: 1,
            unresolved_reference_count: 1,
            full_plan_blocker_count: 1,
        },
        exported: vec![LogicalModelSourceBatchExported {
            family: "mob".to_string(),
            semantic_directories: vec!["npc_iceking".to_string()],
            true_name: "npc_simon".to_string(),
            source_relative_path: "mob/npc_iceking/npc_simon.source.json".to_string(),
            source_byte_length: 123,
            source_sha256: "ab".repeat(32),
            exact_route: "mob/npc_iceking.kfm".to_string(),
            normalized_route: "mob/npc_iceking.kfm".to_string(),
            source_ownership: LogicalModelSourceOwnershipEvidence {
                ownership_evidence: "exactGameObjectSubtree".to_string(),
                self_contained_proof: "oneParentlessTransformRoot".to_string(),
                canonical_owner: owner.clone(),
                physical_alias_owners: vec![owner.clone()],
                physical_fingerprint: Some(LogicalModelPhysicalFingerprint {
                    serialized_asset_sha256: "cd".repeat(32),
                    path_id: 41,
                    object_type: "GameObject".to_string(),
                }),
                root_game_object: key.clone(),
                root_transform: ResolvedObjectKeyEvidence {
                    object_type: "Transform".to_string(),
                    ..key
                },
            },
        }],
        skipped: vec![LogicalModelSourceBatchSkipped {
            exact_route: "mob/raw.kfm".to_string(),
            normalized_route: "mob/raw.kfm".to_string(),
            code: "readyRootNotSelfContained".to_string(),
            detail: "not self-contained".to_string(),
            ownership_evidence: "rawKfmPayload".to_string(),
        }],
        blocked: vec![LogicalModelSourceBatchBlocked {
            subject_kind: "logicalRoot".to_string(),
            exact_route: Some("mob/blocked.kfm".to_string()),
            normalized_route: Some("mob/blocked.kfm".to_string()),
            code: "logicalRootBlockedOrUnresolved".to_string(),
            detail: "unresolved".to_string(),
            owners: vec![owner],
            ownership_evidence: None,
        }],
    };
    let value = serde_json::to_value(manifest).unwrap();
    assert_eq!(
        value
            .pointer("/exported/0/exactRoute")
            .and_then(JsonValue::as_str),
        Some("mob/npc_iceking.kfm")
    );
    assert_eq!(
        value
            .pointer("/exported/0/sourceRelativePath")
            .and_then(JsonValue::as_str),
        Some("mob/npc_iceking/npc_simon.source.json")
    );
    assert_eq!(
        value
            .pointer("/exported/0/sourceOwnership/ownershipEvidence")
            .and_then(JsonValue::as_str),
        Some("exactGameObjectSubtree")
    );
    assert_eq!(
        value.pointer("/skipped/0/code").and_then(JsonValue::as_str),
        Some("readyRootNotSelfContained")
    );
    assert_eq!(
        value.pointer("/blocked/0/code").and_then(JsonValue::as_str),
        Some("logicalRootBlockedOrUnresolved")
    );
}
