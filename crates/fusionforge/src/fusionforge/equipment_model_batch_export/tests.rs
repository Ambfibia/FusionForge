use super::*;

fn semantic_fixture() -> SemanticPlan {
    SemanticPlan {
        schema: SEMANTIC_PLAN_SCHEMA.to_string(),
        entities: vec![
            SemanticEntity {
                id: "equipment:hat:row:1:number:1".to_string(),
                category: "equipment_hat".to_string(),
                semantic_directory: "characters/player/equipment/hat".to_string(),
                table_owner: json!({"table": "hat", "rowIndex": 1, "meshIndex": 26}),
                model_routes: vec!["Wear/Hat_TrueName.nif".to_string()],
            },
            SemanticEntity {
                id: "equipment:hat:row:2:number:2".to_string(),
                category: "equipment_hat".to_string(),
                semantic_directory: "characters/player/equipment/hat".to_string(),
                table_owner: json!({"table": "hat", "rowIndex": 2, "meshIndex": 27}),
                model_routes: vec!["wear/hat_truename.NIF".to_string()],
            },
            SemanticEntity {
                id: "npc:ignored".to_string(),
                category: "npc".to_string(),
                semantic_directory: "characters/npc".to_string(),
                table_owner: json!({}),
                model_routes: vec!["mob/npc.kfm".to_string()],
            },
        ],
    }
}

#[test]
fn semantic_routes_are_exactly_deduplicated_and_keep_every_table_row() {
    let (routes, entities, with_routes, categories) =
        collect_route_plans(&semantic_fixture()).unwrap();
    assert_eq!(entities, 2);
    assert_eq!(with_routes, 2);
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].category, "hat");
    assert_eq!(routes[0].table_rows.len(), 2);
    assert_eq!(categories.get("hat"), Some(&2));
}

#[test]
fn missing_and_ambiguous_routes_are_typed_without_precedence() {
    let (routes, ..) = collect_route_plans(&semantic_fixture()).unwrap();
    let (candidates, blockers) = resolve_routes(&routes, &BTreeMap::new());
    assert!(candidates.is_empty());
    assert_eq!(blockers[0].code, "equipmentRouteMissingFromBundleIndex");

    let owner = EquipmentContainerOwner {
        bundle_path: "D:/build/a.resourceFile".to_string(),
        bundle_name: "a.resourceFile".to_string(),
        asset_name: "CustomAssetBundle".to_string(),
        exact_container_route: "wear/hat_truename.nif".to_string(),
    };
    let occurrences = BTreeMap::from([(
        routes[0].normalized_route.clone(),
        vec![
            owner.clone(),
            EquipmentContainerOwner {
                bundle_path: "D:/build/b.resourceFile".to_string(),
                bundle_name: "b.resourceFile".to_string(),
                ..owner
            },
        ],
    )]);
    let (candidates, blockers) = resolve_routes(&routes, &occurrences);
    assert!(candidates.is_empty());
    assert_eq!(blockers[0].code, "equipmentRouteAmbiguousInBundleIndex");
    assert_eq!(blockers[0].disposition, "blocked-no-owner-precedence-guess");
}

#[test]
fn source_taxonomy_contains_only_semantics_and_true_name() {
    let route = StagedRoute {
        candidate: RouteCandidate {
            route: RoutePlan {
                category: "hat".to_string(),
                exact_route: "wear/hat_apacheshield.nif".to_string(),
                normalized_route: "wear/hat_apacheshield.nif".to_string(),
                table_rows: Vec::new(),
            },
            owner: EquipmentContainerOwner {
                bundle_path: "D:/build/CharacterSelection.resourceFile".to_string(),
                bundle_name: "CharacterSelection.resourceFile".to_string(),
                asset_name: "CustomAssetBundle".to_string(),
                exact_container_route: "wear/hat_apacheshield.nif".to_string(),
            },
        },
        true_name: "hat_apacheshield".to_string(),
        safe_true_name: "hat_apacheshield".to_string(),
        route_qualifier: None,
        temporary_path: PathBuf::from("unused"),
        source_byte_length: 1,
        source_sha256: "0".repeat(64),
        physical_target: EquipmentPhysicalTarget {
            asset_index: 1,
            asset_name: "CustomAssetBundle".to_string(),
            path_id: 42,
            object_type: "GameObject".to_string(),
        },
        facts: EquipmentSourceFacts::default(),
    };
    assert_eq!(
        slash_path(&source_relative_path(&route)),
        "characters/player/equipment/hat/hat_apacheshield/hat_apacheshield.source.json"
    );
    assert!(!destination_key(&route).contains("42"));
    assert!(!destination_key(&route).contains("--"));
}

#[test]
fn route_qualified_collision_taxonomy_keeps_route_and_true_root_semantic() {
    let mut route = StagedRoute {
        candidate: RouteCandidate {
            route: RoutePlan {
                category: "vehicle".to_string(),
                exact_route: "wear/vehicle_policecar.nif".to_string(),
                normalized_route: "wear/vehicle_policecar.nif".to_string(),
                table_rows: Vec::new(),
            },
            owner: EquipmentContainerOwner {
                bundle_path: "D:/build/Retro_shared.resourceFile".to_string(),
                bundle_name: "Retro_shared.resourceFile".to_string(),
                asset_name: "CustomAssetBundle".to_string(),
                exact_container_route: "wear/vehicle_policecar.nif".to_string(),
            },
        },
        true_name: "vehicle_hovercarA".to_string(),
        safe_true_name: "vehicle_hovercarA".to_string(),
        route_qualifier: None,
        temporary_path: PathBuf::from("unused"),
        source_byte_length: 1,
        source_sha256: "0".repeat(64),
        physical_target: EquipmentPhysicalTarget {
            asset_index: 1,
            asset_name: "CustomAssetBundle".to_string(),
            path_id: 42,
            object_type: "GameObject".to_string(),
        },
        facts: EquipmentSourceFacts::default(),
    };
    route.route_qualifier =
        Some(route_stem_qualifier(&route.candidate.route.exact_route).unwrap());
    assert_eq!(
        slash_path(&source_relative_path(&route)),
        "characters/player/equipment/vehicle/vehicle_policecar/vehicle_hovercarA/vehicle_hovercarA.source.json"
    );
}

#[test]
fn lone_route_alias_is_qualified_before_it_can_overwrite_an_existing_true_root() {
    let route_stem = route_stem_qualifier("wear/helmet_noodies_fusion.nif").unwrap();
    let true_root = "helmet_noodies";
    assert_ne!(portable_key(&route_stem), portable_key(true_root));
}
