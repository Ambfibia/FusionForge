use super::*;

fn catalog(source: &str) -> LogicalModelCatalog {
    catalog_logical_models_from_json(source, "fixture/bundle-index.json").unwrap()
}

#[test]
fn groups_duplicate_route_case_and_separator_without_changing_exact_routes() {
    let result = catalog(
        r#"{
            "bundles": [{
                "path": "C:/build/Character.resourceFile",
                "name": "Character.resourceFile",
                "assets": [{
                    "name": "CustomAssetBundle",
                    "containerPaths": [
                        "Mob/NPC_Dexter.KFM",
                        "mob\\npc_dexter.kfm"
                    ]
                }]
            }]
        }"#,
    );

    assert_eq!(result.counts.occurrence_count, 2);
    assert_eq!(result.counts.normalized_route_count, 1);
    assert_eq!(result.counts.case_or_separator_collision_group_count, 1);
    let group = &result.route_groups[0];
    assert_eq!(group.normalized_route, "mob/npc_dexter.kfm");
    assert_eq!(
        group.exact_routes,
        vec!["Mob/NPC_Dexter.KFM", "mob\\npc_dexter.kfm"]
    );
    assert!(group.case_or_separator_collision);
    assert!(!group.ambiguous_owner);
    assert_eq!(group.preferred_owner, Some(group.owners[0].clone()));
}

#[test]
fn ordering_is_stable_and_independent_of_container_path_order() {
    let source_a = r#"{
        "bundles": [
            {
                "path": "C:/build/Z.resourceFile",
                "name": "Z.resourceFile",
                "assets": [{
                    "name": "B",
                    "containerPaths": ["wear/Z.nif", "mob/B.kfm"]
                }]
            },
            {
                "path": "C:/build/A.resourceFile",
                "name": "A.resourceFile",
                "assets": [{"name": "A", "containerPaths": ["mob/A.kfm"]}]
            }
        ]
    }"#;
    let source_b = r#"{
        "bundles": [
            {
                "path": "C:/build/A.resourceFile",
                "name": "A.resourceFile",
                "assets": [{"name": "A", "containerPaths": ["mob/A.kfm"]}]
            },
            {
                "path": "C:/build/Z.resourceFile",
                "name": "Z.resourceFile",
                "assets": [{
                    "name": "B",
                    "containerPaths": ["mob/B.kfm", "wear/Z.nif"]
                }]
            }
        ]
    }"#;

    let first = catalog(source_a);
    let second = catalog(source_b);
    assert_eq!(first, second);
    assert_eq!(
        first
            .occurrences
            .iter()
            .map(|occurrence| occurrence.exact_route.as_str())
            .collect::<Vec<_>>(),
        vec!["mob/A.kfm", "mob/B.kfm", "wear/Z.nif"]
    );
}

#[test]
fn multiple_distinct_owners_are_ambiguous_and_never_preferred() {
    let result = catalog(
        r#"{
            "bundles": [
                {
                    "path": "C:/build/A.resourceFile",
                    "name": "A.resourceFile",
                    "assets": [{"name": "AssetA", "containerPaths": ["mob/shared.kfm"]}]
                },
                {
                    "path": "C:/build/B.resourceFile",
                    "name": "B.resourceFile",
                    "assets": [{"name": "AssetB", "containerPaths": ["mob/shared.kfm"]}]
                }
            ]
        }"#,
    );

    assert_eq!(result.counts.ambiguous_owner_group_count, 1);
    assert_eq!(result.counts.preferred_owner_group_count, 0);
    let group = &result.route_groups[0];
    assert!(group.duplicate_route);
    assert!(group.ambiguous_owner);
    assert_eq!(group.owners.len(), 2);
    assert!(group.preferred_owner.is_none());
    assert!(group.preferred_owner_basis.is_none());
}
