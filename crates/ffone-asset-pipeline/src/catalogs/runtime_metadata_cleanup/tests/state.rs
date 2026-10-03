use super::*;

#[test]
fn policy_keeps_runtime_registries_and_layouts_but_defers_world() {
    assert_eq!(
        classify_metadata("characters/npc/dexter/dexter.publish.json"),
        MetadataDisposition::Archive("logical-model publish evidence")
    );
    assert_eq!(
        classify_metadata("characters/registry.json"),
        MetadataDisposition::Keep
    );
    assert_eq!(
        classify_metadata("characters/_runtime/characters.json"),
        MetadataDisposition::Keep
    );
    assert_eq!(
        classify_metadata("characters/player/male/base/male_skeleton.json"),
        MetadataDisposition::Archive("player rig source conversion contract")
    );
    assert_eq!(
        classify_metadata("characters/player/shared/player_rig_contract.json"),
        MetadataDisposition::Keep,
        "the native client opens the shared player rig contract at runtime"
    );
    assert_eq!(
        classify_metadata("characters/player/female/base/female_skeleton.json"),
        MetadataDisposition::Keep,
        "arbitrary skeleton documents must not be wildcard-selected"
    );
    assert_eq!(
        classify_metadata("ui/gameplay/layout/gameplay_hud.json"),
        MetadataDisposition::Keep
    );
    for relative in ["ui/character/catalog.json", "ui/gameplay/catalog.json"] {
        assert_eq!(
            classify_metadata(relative),
            MetadataDisposition::Archive(
                "UI installer catalog; runtime uses manifest-owned layouts and images"
            )
        );
    }
    for relative in [
        "map/shared/effects/catalog.json",
        "map/shared/projectiles/catalog.json",
    ] {
        assert_eq!(classify_metadata(relative), MetadataDisposition::Keep);
    }
    for relative in ACTIVE_RUNTIME_CATALOGS {
        assert_eq!(classify_metadata(relative), MetadataDisposition::Keep);
    }
    assert_eq!(
        classify_metadata("data/catalog/content-index--fixture.json"),
        MetadataDisposition::Keep
    );
    assert!(matches!(
        classify_metadata("data/catalog/avatar/test_cook_report.json"),
        MetadataDisposition::Archive(_)
    ));
    assert!(matches!(
        classify_metadata("tutorial/models/catalog.json"),
        MetadataDisposition::Archive(_)
    ));
    assert_eq!(
        classify_metadata("tutorial/models/tree/tree.publish.json"),
        MetadataDisposition::Archive("tutorial logical-model publish evidence")
    );
    assert!(matches!(
        classify_metadata("world/tutorial/static/tiles/tile_00_00/hierarchy.json"),
        MetadataDisposition::TutorialStatic(_)
    ));
    assert!(matches!(
        classify_metadata("world/maps/map_00_00/terrain/scene-instance.json"),
        MetadataDisposition::DeferWorld(_)
    ));
    assert!(matches!(
        classify_metadata("world/catalog.json"),
        MetadataDisposition::DeferWorld(_)
    ));
}

#[test]
fn tutorial_static_metadata_requires_runtime_pin_and_keeps_payloads_and_scene() {
    let (_temp, project_root) = fixture();
    add_static_world_fixture(&project_root, false);
    let blocked = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap();
    assert!(!blocked.apply_ready);
    assert_eq!(
        blocked
            .deferred_world
            .iter()
            .filter(|entry| entry.source_path.starts_with("world/tutorial/static/"))
            .count(),
        4
    );
    assert!(
        blocked
            .blockers
            .iter()
            .any(|blocker| blocker.contains(RUNTIME_WORLD_REGISTRY_PATH))
    );

    let scene_relative = "world/tutorial/terrain/tiles/tile_00_00/scene.json";
    let scene_bytes = fs::read(project_root.join("assets/game").join(scene_relative)).unwrap();
    let registry = RuntimeWorldRegistry {
        schema: RUNTIME_WORLD_REGISTRY_SCHEMA.to_owned(),
        entries: vec![RuntimeWorldRegistryEntry {
            id: "tile_00_00".to_owned(),
            scope: "tutorial".to_owned(),
            tile: [0, 0],
            scene: RuntimeWorldContentReference {
                path: scene_relative.to_owned(),
                blake3: blake3::hash(&scene_bytes).to_hex().to_string(),
            },
            terrain: RuntimeWorldContentReference {
                path: "world/tutorial/terrain/tiles/tile_00_00/terrain.json".to_owned(),
                blake3: "0".repeat(64),
            },
            environment: None,
        }],
    };
    add_fixture_asset(
        &project_root,
        RUNTIME_WORLD_REGISTRY_PATH,
        &serde_json::to_vec(&registry).unwrap(),
    );
    add_fixture_asset(
        &project_root,
        "tutorial/models/tree/tree.publish.json",
        br#"{"schema":"tutorial-publish"}"#,
    );
    add_fixture_asset(
        &project_root,
        "tutorial/models/tree/tree.glb",
        b"tutorial-model-glb",
    );

    let applied = clean_runtime_metadata(
        &CleanRuntimeMetadataOptions::new(&project_root, "retrobution-test").with_apply(true),
    )
    .unwrap();
    assert!(applied.apply_ready);
    for relative in [
        TUTORIAL_STATIC_INSTALL_MANIFEST,
        "world/tutorial/static/tiles/tile_00_00/catalog.json",
        "world/tutorial/static/tiles/tile_00_00/hierarchy.json",
        "world/tutorial/static/tiles/tile_00_00/materials.json",
        "tutorial/models/tree/tree.publish.json",
    ] {
        assert!(!project_root.join("assets/game").join(relative).exists());
    }
    for relative in [
        "models/world/tutorial/tile_00_00/tree.glb",
        "models/world/tutorial/tile_00_00/tree.png",
        scene_relative,
        "tutorial/models/tree/tree.glb",
    ] {
        assert!(
            project_root.join("assets/game").join(relative).is_file(),
            "{relative} must remain a runtime payload"
        );
    }
    let repeat = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap();
    assert_eq!(repeat.counts.archived_files, 0);
}
