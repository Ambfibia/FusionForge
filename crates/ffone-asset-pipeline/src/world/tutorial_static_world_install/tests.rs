use super::*;
use crate::{
    SourcePackIdentity,
    runtime_metadata_cleanup::{
        CleanRuntimeMetadataMode, CleanRuntimeMetadataOptions, clean_runtime_metadata,
    },
};
use serde_json::json;
use tempfile::TempDir;

const FIXTURE_ARCHIVE: &str =
    "1111111111111111111111111111111111111111111111111111111111111111";

struct Fixture {
    _root: TempDir,
    project: PathBuf,
    export: PathBuf,
    assets: PathBuf,
    contracts: Vec<TileContract>,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        let export = root.path().join("export");
        let assets = project.join("assets").join("game");
        fs::create_dir(&export).unwrap();
        fs::create_dir_all(&assets).unwrap();
        let contracts = vec![
            TileContract {
                id: Cow::Borrowed("tile_00_00"),
                source_archive_blake3: Cow::Borrowed(FIXTURE_ARCHIVE),
                scene_nodes: 1,
                exported_visuals: 1,
                runtime_visuals: 1,
                exported_colliders: 1,
                runtime_colliders: 1,
                exported_models: 1,
                vertices: 3,
                indices: 3,
            },
            TileContract {
                id: Cow::Borrowed("tile_00_01"),
                source_archive_blake3: Cow::Borrowed(FIXTURE_ARCHIVE),
                scene_nodes: 1,
                exported_visuals: 1,
                runtime_visuals: 1,
                exported_colliders: 1,
                runtime_colliders: 1,
                exported_models: 1,
                vertices: 3,
                indices: 3,
            },
        ];
        let mut manifest_files = Vec::new();
        let mut scene_references = Vec::new();
        for contract in &contracts {
            let scene = base_scene(&contract.id);
            let bytes = pretty_json(&scene).unwrap();
            let path = tile_scene_path(&contract.id);
            write_new(&safe_join(&assets, &path).unwrap(), &bytes).unwrap();
            let blake3 = hash_bytes(&bytes);
            scene_references.push((contract.id.as_ref(), path.clone(), blake3.clone()));
            manifest_files.push(ProjectAssetFile {
                source_path: format!("native-terrain-export/{path}"),
                path: path.clone(),
                kind: ProjectAssetKind::Data,
                bytes: bytes.len() as u64,
                blake3,
            });
        }
        let catalog = json!({
            "schema": WORLD_CATALOG_SCHEMA,
            "sourceBuild": TUTORIAL_STATIC_WORLD_SOURCE_BUILD,
            "status": "complete",
            "terrainGlbAllowed": false,
            "entries": scene_references.iter().map(|(tile_id, scene, blake3)| json!({
                "scope": "tutorial",
                "instanceId": tile_id,
                "tile": tile_coordinates(tile_id).unwrap(),
                "placementStatus": "linked",
                "scene": scene,
                "sceneBlake3": blake3,
                "terrainDescriptor": format!("world/tutorial/terrain/tiles/{tile_id}/terrain.json"),
                "terrainDescriptorBlake3": "2".repeat(64),
                "provenance": format!("world/tutorial/terrain/tiles/{tile_id}/provenance.json")
            })).collect::<Vec<_>>(),
            "blocked": []
        });
        let runtime_registry = json!({
            "schema": RUNTIME_WORLD_REGISTRY_SCHEMA,
            "entries": scene_references.iter().map(|(tile_id, scene, blake3)| json!({
                "id": tile_id,
                "scope": "tutorial",
                "tile": tile_coordinates(tile_id).unwrap(),
                "scene": {"path": scene, "blake3": blake3},
                "terrain": {
                    "path": format!("world/tutorial/terrain/tiles/{tile_id}/terrain.json"),
                    "blake3": "2".repeat(64)
                }
            })).collect::<Vec<_>>()
        });
        for (path, value) in [
            (WORLD_CATALOG_PATH, catalog),
            (RUNTIME_WORLD_REGISTRY_PATH, runtime_registry),
        ] {
            let bytes = pretty_json(&value).unwrap();
            write_new(&safe_join(&assets, path).unwrap(), &bytes).unwrap();
            manifest_files.push(ProjectAssetFile {
                source_path: format!("native-terrain-export/{path}"),
                path: path.to_owned(),
                kind: ProjectAssetKind::Data,
                bytes: bytes.len() as u64,
                blake3: hash_bytes(&bytes),
            });
        }
        let unrelated = b"keep";
        write_new(&assets.join("unrelated.bin"), unrelated).unwrap();
        manifest_files.push(ProjectAssetFile {
            source_path: "fixture/unrelated.bin".to_owned(),
            path: "unrelated.bin".to_owned(),
            kind: ProjectAssetKind::Data,
            bytes: unrelated.len() as u64,
            blake3: hash_bytes(unrelated),
        });
        manifest_files.sort_by(|left, right| left.path.cmp(&right.path));
        write_project_manifest(&assets, manifest_files);
        let fixture = Self {
            _root: root,
            project,
            export,
            assets,
            contracts,
        };
        fixture.write_exports("a");
        fixture
    }

    fn options(&self) -> TutorialStaticWorldInstallOptions {
        TutorialStaticWorldInstallOptions::new(&self.export, &self.assets)
    }

    fn write_exports(&self, version: &str) {
        for contract in &self.contracts {
            let tile_root = self.export.join(contract.id.as_ref());
            if tile_root.exists() {
                fs::remove_dir_all(&tile_root).unwrap();
            }
            fs::create_dir(&tile_root).unwrap();
            write_fixture_export(&tile_root, contract, version);
        }
    }

    fn assert_reference_closure(&self, expected_paths: &[&str]) {
        let manifest: ProjectAssetManifest = read_json(
            &self.assets.join(ASSET_MANIFEST_FILE),
            "fixture project manifest",
        )
        .unwrap();
        let ownership: TutorialStaticWorldOwnership = read_json(
            &safe_join(&self.assets, TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH).unwrap(),
            "fixture tutorial ownership",
        )
        .unwrap();
        assert_eq!(ownership.schema, TUTORIAL_STATIC_WORLD_OWNERSHIP_SCHEMA);
        assert_eq!(ownership.references.len(), expected_paths.len());
        for expected in expected_paths {
            assert!(
                ownership
                    .references
                    .iter()
                    .any(|reference| reference.path == *expected)
            );
        }
        verify_reference_closure(
            &self.assets,
            &manifest,
            &ownership.scenes,
            &ownership.references,
        )
        .unwrap();
    }

    fn remove_reference_document(&self, relative: &str) {
        fs::remove_file(safe_join(&self.assets, relative).unwrap()).unwrap();
        let manifest_path = self.assets.join(ASSET_MANIFEST_FILE);
        let mut manifest: ProjectAssetManifest =
            read_json(&manifest_path, "fixture project manifest").unwrap();
        manifest.files.retain(|entry| entry.path != relative);
        fs::write(&manifest_path, pretty_json(&manifest).unwrap()).unwrap();
    }

    fn rewrite_json_document(&self, relative: &str, mutate: impl FnOnce(&mut JsonValue)) {
        let path = safe_join(&self.assets, relative).unwrap();
        let mut value: JsonValue = read_json(&path, "fixture JSON document").unwrap();
        mutate(&mut value);
        let bytes = pretty_json(&value).unwrap();
        fs::write(&path, &bytes).unwrap();

        let manifest_path = self.assets.join(ASSET_MANIFEST_FILE);
        let mut manifest: ProjectAssetManifest =
            read_json(&manifest_path, "fixture project manifest").unwrap();
        let entry = manifest
            .files
            .iter_mut()
            .find(|entry| entry.path == relative)
            .unwrap();
        entry.bytes = bytes.len() as u64;
        entry.blake3 = hash_bytes(&bytes);
        fs::write(&manifest_path, pretty_json(&manifest).unwrap()).unwrap();
    }
}

#[test]
fn install_is_idempotent_and_removes_stale_owned_files() {
    let fixture = Fixture::new();
    let first = install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    assert!(!first.replaced_previous_install);
    assert_eq!(first.tile_count, 2);
    assert_eq!(first.model_count, 2);
    fixture.assert_reference_closure(&[WORLD_CATALOG_PATH, RUNTIME_WORLD_REGISTRY_PATH]);
    assert_eq!(
        fs::read(fixture.assets.join("unrelated.bin")).unwrap(),
        b"keep"
    );
    let first_manifest = fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap();
    let first_catalog =
        fs::read(safe_join(&fixture.assets, WORLD_CATALOG_PATH).unwrap()).unwrap();
    let first_registry =
        fs::read(safe_join(&fixture.assets, RUNTIME_WORLD_REGISTRY_PATH).unwrap()).unwrap();

    let second = install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    assert!(second.replaced_previous_install);
    fixture.assert_reference_closure(&[WORLD_CATALOG_PATH, RUNTIME_WORLD_REGISTRY_PATH]);
    assert_eq!(
        fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap(),
        first_manifest
    );
    assert_eq!(
        fs::read(safe_join(&fixture.assets, WORLD_CATALOG_PATH).unwrap()).unwrap(),
        first_catalog
    );
    assert_eq!(
        fs::read(safe_join(&fixture.assets, RUNTIME_WORLD_REGISTRY_PATH).unwrap()).unwrap(),
        first_registry
    );

    fixture.write_exports("b");
    let third = install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    assert!(third.replaced_previous_install);
    fixture.assert_reference_closure(&[WORLD_CATALOG_PATH, RUNTIME_WORLD_REGISTRY_PATH]);
    for contract in &fixture.contracts {
        let root = fixture
            .assets
            .join("models/world/tutorial")
            .join(contract.id.as_ref());
        assert!(!root.join("a.glb").exists());
        assert!(root.join("b.glb").is_file());
        let scene: JsonValue = read_json(
            &safe_join(&fixture.assets, &tile_scene_path(&contract.id)).unwrap(),
            "scene",
        )
        .unwrap();
        assert_eq!(scene["coverage"], STATIC_COVERAGE);
        assert_eq!(scene["customUnrelatedField"], true);
    }
    assert_eq!(
        fs::read(fixture.assets.join("unrelated.bin")).unwrap(),
        b"keep"
    );
}

#[test]
fn runtime_only_reference_closure_is_idempotent() {
    let fixture = Fixture::new();
    fixture.remove_reference_document(WORLD_CATALOG_PATH);

    let first = install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    assert!(!first.replaced_previous_install);
    fixture.assert_reference_closure(&[RUNTIME_WORLD_REGISTRY_PATH]);
    let manifest = fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap();
    let registry =
        fs::read(safe_join(&fixture.assets, RUNTIME_WORLD_REGISTRY_PATH).unwrap()).unwrap();

    let second = install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    assert!(second.replaced_previous_install);
    fixture.assert_reference_closure(&[RUNTIME_WORLD_REGISTRY_PATH]);
    assert_eq!(
        fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap(),
        manifest
    );
    assert_eq!(
        fs::read(safe_join(&fixture.assets, RUNTIME_WORLD_REGISTRY_PATH).unwrap()).unwrap(),
        registry
    );
}

#[test]
fn world_map_domain_republishes_without_a_global_asset_manifest() {
    let root = tempfile::tempdir().unwrap();
    let export = root.path().join("export");
    let assets = root.path().join("assets").join("game");
    fs::create_dir(&export).unwrap();
    fs::create_dir_all(&assets).unwrap();
    let contract = TileContract {
        id: Cow::Borrowed("map_03_04"),
        source_archive_blake3: Cow::Borrowed(FIXTURE_ARCHIVE),
        scene_nodes: 1,
        exported_visuals: 1,
        runtime_visuals: 1,
        exported_colliders: 1,
        runtime_colliders: 1,
        exported_models: 1,
        vertices: 3,
        indices: 3,
    };
    let contracts = vec![contract.clone()];
    let scene_path = tile_scene_path(&contract.id);
    let scene_bytes = pretty_json(&base_scene(&contract.id)).unwrap();
    write_new(&safe_join(&assets, &scene_path).unwrap(), &scene_bytes).unwrap();
    let scene_hash = hash_bytes(&scene_bytes);
    let registry_bytes = pretty_json(&json!({
        "schema": RUNTIME_WORLD_REGISTRY_SCHEMA,
        "entries": [{
            "id": contract.id,
            "scope": "worldMap",
            "tile": [3, 4],
            "scene": {"path": scene_path, "blake3": scene_hash},
            "terrain": {
                "path": "world/maps/map_03_04/terrain/terrain.json",
                "blake3": "2".repeat(64)
            }
        }]
    }))
    .unwrap();
    write_new(
        &safe_join(&assets, RUNTIME_WORLD_REGISTRY_PATH).unwrap(),
        &registry_bytes,
    )
    .unwrap();
    write_new(&assets.join("unrelated.bin"), b"keep").unwrap();
    write_project_manifest(
        &assets,
        vec![
            ProjectAssetFile {
                source_path: format!("native-terrain-export/{scene_path}"),
                path: scene_path.clone(),
                kind: ProjectAssetKind::Data,
                bytes: scene_bytes.len() as u64,
                blake3: scene_hash,
            },
            ProjectAssetFile {
                source_path: format!("native-terrain-export/{RUNTIME_WORLD_REGISTRY_PATH}"),
                path: RUNTIME_WORLD_REGISTRY_PATH.to_owned(),
                kind: ProjectAssetKind::Data,
                bytes: registry_bytes.len() as u64,
                blake3: hash_bytes(&registry_bytes),
            },
            ProjectAssetFile {
                source_path: "fixture/unrelated.bin".to_owned(),
                path: "unrelated.bin".to_owned(),
                kind: ProjectAssetKind::Data,
                bytes: 4,
                blake3: hash_bytes(b"keep"),
            },
        ],
    );
    let tile_export = export.join(contract.id.as_ref());
    fs::create_dir(&tile_export).unwrap();
    write_fixture_export(&tile_export, &contract, "a");
    let options = TutorialStaticWorldInstallOptions::new(&export, &assets);
    install_with_contract(&options, &contracts).unwrap();

    fs::remove_file(assets.join(ASSET_MANIFEST_FILE)).unwrap();
    fs::remove_dir_all(&tile_export).unwrap();
    fs::create_dir(&tile_export).unwrap();
    write_fixture_export(&tile_export, &contract, "b");
    let report = install_with_contract(&options, &contracts).unwrap();
    assert!(report.replaced_previous_install);
    assert_eq!(report.manifest_files, 0);
    assert!(!assets.join(ASSET_MANIFEST_FILE).exists());
    assert_eq!(fs::read(assets.join("unrelated.bin")).unwrap(), b"keep");
    let model_root = assets.join(model_tile_root(&contract.id));
    assert!(!model_root.join("a.glb").exists());
    assert!(model_root.join("b.glb").is_file());
    let installed_scene = fs::read(safe_join(&assets, &scene_path).unwrap()).unwrap();
    let registry: JsonValue = read_json(
        &safe_join(&assets, RUNTIME_WORLD_REGISTRY_PATH).unwrap(),
        "runtime registry",
    )
    .unwrap();
    assert_eq!(
        registry["entries"][0]["scene"]["blake3"],
        hash_bytes(&installed_scene)
    );
    let ownership: TutorialStaticWorldOwnership = read_json(
        &safe_join(&assets, WORLD_MAP_STATIC_WORLD_OWNERSHIP_PATH).unwrap(),
        "world-map ownership",
    )
    .unwrap();
    assert_eq!(ownership.references.len(), 1);
    assert_eq!(ownership.references[0].path, RUNTIME_WORLD_REGISTRY_PATH);
}

#[test]
fn cleanup_archived_install_noops_identical_source_and_reactivates_an_upgrade() {
    let fixture = Fixture::new();
    fixture.remove_reference_document(WORLD_CATALOG_PATH);
    install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    let cleanup = clean_runtime_metadata(
        &CleanRuntimeMetadataOptions::new(&fixture.project, TUTORIAL_STATIC_WORLD_SOURCE_BUILD)
            .with_apply(true),
    )
    .unwrap();
    assert_eq!(cleanup.mode, CleanRuntimeMetadataMode::Apply);
    assert!(
        cleanup
            .archived
            .iter()
            .any(|entry| entry.source_path == TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH)
    );
    assert!(
        !safe_join(&fixture.assets, TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH)
            .unwrap()
            .exists()
    );
    let manifest_before = fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap();
    let registry_before =
        fs::read(safe_join(&fixture.assets, RUNTIME_WORLD_REGISTRY_PATH).unwrap()).unwrap();

    for _ in 0..2 {
        let report = install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
        assert!(report.replaced_previous_install);
        assert_eq!(report.installed_files, 0);
        assert_eq!(report.installed_bytes, 0);
        assert_eq!(
            fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap(),
            manifest_before
        );
        assert_eq!(
            fs::read(safe_join(&fixture.assets, RUNTIME_WORLD_REGISTRY_PATH).unwrap()).unwrap(),
            registry_before
        );
        assert!(
            !safe_join(&fixture.assets, TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH)
                .unwrap()
                .exists()
        );
    }

    fixture.write_exports("b");
    let upgraded = install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    assert!(upgraded.replaced_previous_install);
    assert!(upgraded.installed_files > 0);
    assert!(
        safe_join(&fixture.assets, TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH)
            .unwrap()
            .is_file()
    );
    assert_ne!(
        fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap(),
        manifest_before
    );
    let upgraded_cleanup = clean_runtime_metadata(
        &CleanRuntimeMetadataOptions::new(&fixture.project, TUTORIAL_STATIC_WORLD_SOURCE_BUILD)
            .with_apply(true),
    )
    .unwrap();
    assert_eq!(upgraded_cleanup.mode, CleanRuntimeMetadataMode::Apply);
    assert!(
        !safe_join(&fixture.assets, TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH)
            .unwrap()
            .exists()
    );
}

#[test]
fn legacy_v1_both_documents_repair_stale_scene_hashes() {
    let fixture = Fixture::new();
    install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    fixture.rewrite_json_document(TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH, |ownership| {
        ownership["schema"] =
            JsonValue::String(LEGACY_TUTORIAL_STATIC_WORLD_OWNERSHIP_SCHEMA.to_owned());
        ownership.as_object_mut().unwrap().remove("references");
    });
    fixture.rewrite_json_document(WORLD_CATALOG_PATH, |catalog| {
        for entry in catalog["entries"].as_array_mut().unwrap() {
            entry["sceneBlake3"] = JsonValue::String("f".repeat(64));
        }
    });
    fixture.rewrite_json_document(RUNTIME_WORLD_REGISTRY_PATH, |registry| {
        for entry in registry["entries"].as_array_mut().unwrap() {
            entry["scene"]["blake3"] = JsonValue::String("f".repeat(64));
        }
    });

    let repaired = install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    assert!(repaired.replaced_previous_install);
    fixture.assert_reference_closure(&[WORLD_CATALOG_PATH, RUNTIME_WORLD_REGISTRY_PATH]);
    let manifest = fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap();

    let repeated = install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    assert!(repeated.replaced_previous_install);
    fixture.assert_reference_closure(&[WORLD_CATALOG_PATH, RUNTIME_WORLD_REGISTRY_PATH]);
    assert_eq!(
        fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap(),
        manifest
    );
}

#[test]
fn legacy_scene_drift_without_completed_migration_is_blocked_before_staging() {
    let fixture = Fixture::new();
    install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    fixture.rewrite_json_document(TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH, |ownership| {
        ownership["schema"] =
            JsonValue::String(LEGACY_TUTORIAL_STATIC_WORLD_OWNERSHIP_SCHEMA.to_owned());
        ownership.as_object_mut().unwrap().remove("references");
    });

    let scene_path = tile_scene_path("tile_00_00");
    fixture.rewrite_json_document(&scene_path, |scene| {
        scene.as_object_mut().unwrap().remove("provenance");
    });
    let manifest: ProjectAssetManifest = read_json(
        &fixture.assets.join(ASSET_MANIFEST_FILE),
        "fixture project manifest",
    )
    .unwrap();
    let current_scene_blake3 = project_entry(&manifest, &scene_path)
        .unwrap()
        .blake3
        .clone();
    fixture.rewrite_json_document(WORLD_CATALOG_PATH, |catalog| {
        let entry = catalog["entries"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entry| entry["scene"] == scene_path)
            .unwrap();
        entry["sceneBlake3"] = JsonValue::String(current_scene_blake3.clone());
    });
    fixture.rewrite_json_document(RUNTIME_WORLD_REGISTRY_PATH, |registry| {
        let entry = registry["entries"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entry| entry["scene"]["path"] == scene_path)
            .unwrap();
        entry["scene"]["blake3"] = JsonValue::String(current_scene_blake3);
    });

    let tracked = [
        ASSET_MANIFEST_FILE,
        TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH,
        scene_path.as_str(),
        WORLD_CATALOG_PATH,
        RUNTIME_WORLD_REGISTRY_PATH,
    ]
    .into_iter()
    .map(|relative| {
        (
            relative.to_owned(),
            fs::read(safe_join(&fixture.assets, relative).unwrap()).unwrap(),
        )
    })
    .collect::<BTreeMap<_, _>>();
    let error = install_with_contract(&fixture.options(), &fixture.contracts).unwrap_err();
    assert!(error.to_string().contains("world-conversion-metadata"));
    for (relative, before) in tracked {
        assert_eq!(
            fs::read(safe_join(&fixture.assets, &relative).unwrap()).unwrap(),
            before,
            "installer mutated {relative:?} before proving the migration"
        );
    }
}

#[test]
fn source_hash_failure_does_not_mutate_installed_assets() {
    let fixture = Fixture::new();
    install_with_contract(&fixture.options(), &fixture.contracts).unwrap();
    let manifest_before = fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap();
    let scene_before = fs::read(
        safe_join(&fixture.assets, &tile_scene_path(&fixture.contracts[0].id)).unwrap(),
    )
    .unwrap();
    let catalog_before =
        fs::read(safe_join(&fixture.assets, WORLD_CATALOG_PATH).unwrap()).unwrap();
    let registry_before =
        fs::read(safe_join(&fixture.assets, RUNTIME_WORLD_REGISTRY_PATH).unwrap()).unwrap();
    fs::write(
        fixture
            .export
            .join(fixture.contracts[0].id.as_ref())
            .join("models/world/tutorial")
            .join(fixture.contracts[0].id.as_ref())
            .join("a.glb"),
        b"tampered",
    )
    .unwrap();
    let error = install_with_contract(&fixture.options(), &fixture.contracts)
        .unwrap_err()
        .to_string();
    assert!(error.contains("proof mismatch"));
    assert_eq!(
        fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap(),
        manifest_before
    );
    assert_eq!(
        fs::read(
            safe_join(&fixture.assets, &tile_scene_path(&fixture.contracts[0].id)).unwrap()
        )
        .unwrap(),
        scene_before
    );
    assert_eq!(
        fs::read(safe_join(&fixture.assets, WORLD_CATALOG_PATH).unwrap()).unwrap(),
        catalog_before
    );
    assert_eq!(
        fs::read(safe_join(&fixture.assets, RUNTIME_WORLD_REGISTRY_PATH).unwrap()).unwrap(),
        registry_before
    );
}

#[test]
fn path_validation_rejects_escape_and_windows_aliases() {
    for path in [
        "../escape",
        "models\\escape.glb",
        "/absolute",
        "models//x.glb",
        "C:x",
        "models/CON/file.glb",
        "models/trailing./file.glb",
    ] {
        assert!(validate_relative(path).is_err(), "{path}");
    }
    assert!(validate_relative("models/world/tutorial/tile_00_00/model.glb").is_ok());
}

#[test]
fn scope_is_derived_from_the_tile_identity() {
    assert_eq!(
        StaticWorldScope::of_tile("tile_00_00").unwrap(),
        StaticWorldScope::Tutorial
    );
    assert_eq!(
        StaticWorldScope::of_tile("map_03_04").unwrap(),
        StaticWorldScope::WorldMap
    );
    assert!(StaticWorldScope::of_tile("dong_03_04").is_err());

    assert_eq!(
        tile_scene_path("tile_00_00"),
        "world/tutorial/terrain/tiles/tile_00_00/scene.json"
    );
    assert_eq!(
        tile_scene_path("map_03_04"),
        "world/maps/map_03_04/scene.json"
    );
    assert_eq!(
        static_metadata_path("map_03_04", "catalog.json"),
        "world/maps/static/tiles/map_03_04/catalog.json"
    );
    assert_eq!(model_tile_root("map_03_04"), "models/world/maps/map_03_04");
    assert_eq!(tile_coordinates("map_03_04").unwrap(), [3, 4]);
    assert!(tile_coordinates("map_3_4").is_err());
}

#[test]
fn a_contract_may_not_mix_scopes() {
    let tutorial = TileContract {
        id: Cow::Borrowed("tile_00_00"),
        source_archive_blake3: Cow::Borrowed(FIXTURE_ARCHIVE),
        scene_nodes: 1,
        exported_visuals: 1,
        runtime_visuals: 1,
        exported_colliders: 1,
        runtime_colliders: 1,
        exported_models: 1,
        vertices: 3,
        indices: 3,
    };
    let mut world_map = tutorial.clone();
    world_map.id = Cow::Borrowed("map_03_04");

    assert_eq!(
        StaticWorldScope::of_contracts(std::slice::from_ref(&tutorial)).unwrap(),
        StaticWorldScope::Tutorial
    );
    assert_eq!(
        StaticWorldScope::of_contracts(std::slice::from_ref(&world_map)).unwrap(),
        StaticWorldScope::WorldMap
    );
    assert!(StaticWorldScope::of_contracts(&[tutorial, world_map]).is_err());
    assert!(StaticWorldScope::of_contracts(&[]).is_err());
}

#[test]
fn world_map_contract_loading_is_fail_closed() {
    let root =
        std::env::temp_dir().join(format!("ffone-world-map-contract-{}", unique_token()));
    fs::create_dir_all(&root).unwrap();
    let tile = |id: &str| {
        serde_json::json!({
            "id": id,
            "sourceArchiveBlake3": FIXTURE_ARCHIVE,
            "sceneNodes": 4,
            "exportedVisuals": 2,
            "runtimeVisuals": 2,
            "exportedColliders": 1,
            "runtimeColliders": 1,
            "exportedModels": 3,
            "vertices": 9,
            "indices": 9,
        })
    };
    let write = |name: &str, value: JsonValue| {
        let path = root.join(name);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        path
    };

    let valid = write(
        "valid.json",
        serde_json::json!({
            "schema": WORLD_MAP_STATIC_WORLD_CONTRACT_SCHEMA,
            "sourceBuild": TUTORIAL_STATIC_WORLD_SOURCE_BUILD,
            "tiles": [tile("map_03_05"), tile("map_03_04")],
        }),
    );
    let contracts =
        load_world_map_contract(&valid, TUTORIAL_STATIC_WORLD_SOURCE_BUILD).unwrap();
    assert_eq!(
        contracts
            .iter()
            .map(|contract| contract.id.as_ref())
            .collect::<Vec<_>>(),
        ["map_03_04", "map_03_05"]
    );

    let duplicate = write(
        "duplicate.json",
        serde_json::json!({
            "schema": WORLD_MAP_STATIC_WORLD_CONTRACT_SCHEMA,
            "sourceBuild": TUTORIAL_STATIC_WORLD_SOURCE_BUILD,
            "tiles": [tile("map_03_04"), tile("map_03_04")],
        }),
    );
    assert!(load_world_map_contract(&duplicate, TUTORIAL_STATIC_WORLD_SOURCE_BUILD).is_err());

    let wrong_build = write(
        "wrong-build.json",
        serde_json::json!({
            "schema": WORLD_MAP_STATIC_WORLD_CONTRACT_SCHEMA,
            "sourceBuild": "some-other-build",
            "tiles": [tile("map_03_04")],
        }),
    );
    assert!(load_world_map_contract(&wrong_build, TUTORIAL_STATIC_WORLD_SOURCE_BUILD).is_err());

    let bad_id = write(
        "bad-id.json",
        serde_json::json!({
            "schema": WORLD_MAP_STATIC_WORLD_CONTRACT_SCHEMA,
            "sourceBuild": TUTORIAL_STATIC_WORLD_SOURCE_BUILD,
            "tiles": [tile("dong_03_04")],
        }),
    );
    assert!(load_world_map_contract(&bad_id, TUTORIAL_STATIC_WORLD_SOURCE_BUILD).is_err());

    fs::remove_dir_all(&root).unwrap();
}

fn write_fixture_export(tile_root: &Path, contract: &TileContract, version: &str) {
    let model_root = model_tile_root(&contract.id);
    let model_path = format!("{model_root}/{version}.glb");
    let texture_path = format!("{model_root}/textures/albedo.png");
    let glb = fixture_glb("textures/albedo.png");
    let texture = b"fixture-png";
    write_new(&safe_join(tile_root, &model_path).unwrap(), &glb).unwrap();
    write_new(&safe_join(tile_root, &texture_path).unwrap(), texture).unwrap();

    let model_id = format!("static-{}-00000", contract.id);
    let model_hash = hash_bytes(&glb);
    let mut scene = base_scene(&contract.id);
    scene
        .as_object_mut()
        .expect("fixture scene object")
        .remove("customUnrelatedField");
    scene["coverage"] = json!(STATIC_COVERAGE);
    scene["models"] = json!([{
        "blake3": model_hash,
        "id": model_id,
        "path": model_path,
        "rootName": "fixture"
    }]);
    scene["visuals"] = json!([{"model": model_id, "transform": {}}]);
    scene["colliders"] = json!([{"model": model_id, "transform": {}}]);
    let scene_path = tile_scene_path(&contract.id);
    write_json_at(tile_root, &scene_path, &scene);

    let catalog_path = static_metadata_path(&contract.id, "catalog.json");
    let hierarchy_path = static_metadata_path(&contract.id, "hierarchy.json");
    let materials_path = static_metadata_path(&contract.id, "materials.json");
    write_json_at(
        tile_root,
        &catalog_path,
        &json!({"schema": STATIC_CATALOG_SCHEMA, "sourceBuild": TUTORIAL_STATIC_WORLD_SOURCE_BUILD, "tileId": contract.id}),
    );
    write_json_at(
        tile_root,
        &hierarchy_path,
        &json!({"schema": STATIC_HIERARCHY_SCHEMA, "sourceBuild": TUTORIAL_STATIC_WORLD_SOURCE_BUILD, "tileId": contract.id}),
    );
    write_json_at(
        tile_root,
        &materials_path,
        &json!({"schema": STATIC_MATERIALS_SCHEMA, "sourceBuild": TUTORIAL_STATIC_WORLD_SOURCE_BUILD, "tileId": contract.id}),
    );

    let report_path = tile_root.join(EXPORT_REPORT_FILE);
    let report = json!({
        "schema": EXPORT_REPORT_SCHEMA,
        "status": EXPORT_STATUS,
        "sourceBuild": TUTORIAL_STATIC_WORLD_SOURCE_BUILD,
        "tileId": contract.id,
        "sourceArchiveBlake3": contract.source_archive_blake3,
        "scenePath": scene_path,
        "sceneBlake3": hash_file_at(tile_root, &scene_path),
        "hierarchyPath": hierarchy_path,
        "hierarchyBlake3": hash_file_at(tile_root, &hierarchy_path),
        "materialPath": materials_path,
        "materialBlake3": hash_file_at(tile_root, &materials_path),
        "catalogPath": catalog_path,
        "catalogBlake3": hash_file_at(tile_root, &catalog_path),
        "manifestPath": EXPORT_MANIFEST_FILE,
        "counts": {
            "sceneNodes": contract.scene_nodes,
            "exportedVisualPayloads": contract.exported_visuals,
            "runtimeVisuals": contract.runtime_visuals,
            "exportedColliderPayloads": contract.exported_colliders,
            "runtimeColliders": contract.runtime_colliders,
            "exportedModels": contract.exported_models,
            "vertices": contract.vertices,
            "indices": contract.indices,
            "outputFiles": 0,
            "outputBytes": 0
        }
    });
    let report_bytes = pretty_json(&report).unwrap();
    write_new(&report_path, &report_bytes).unwrap();

    let mut paths = BTreeSet::new();
    collect_files(tile_root, tile_root, &mut paths).unwrap();
    let files = paths
        .iter()
        .map(|path| {
            let absolute = safe_join(tile_root, path).unwrap();
            let bytes = fs::read(&absolute).unwrap();
            ExportManifestFileFixture {
                blake3: hash_bytes(&bytes),
                byte_length: bytes.len() as u64,
                kind: if path.ends_with(".glb") {
                    "model"
                } else if path.ends_with(".png") {
                    "texture"
                } else {
                    "data"
                },
                path,
            }
        })
        .collect::<Vec<_>>();
    let export_manifest = json!({
        "counts": {
            "bytes": files.iter().map(|file| file.byte_length).sum::<u64>(),
            "files": files.len() as u64
        },
        "files": files.iter().map(|file| json!({
            "blake3": file.blake3,
            "byteLength": file.byte_length,
            "kind": file.kind,
            "path": file.path
        })).collect::<Vec<_>>(),
        "schema": EXPORT_MANIFEST_SCHEMA,
        "selfExcluded": EXPORT_MANIFEST_FILE,
        "sourceBuild": TUTORIAL_STATIC_WORLD_SOURCE_BUILD,
        "tileId": contract.id
    });
    write_json_at(tile_root, EXPORT_MANIFEST_FILE, &export_manifest);
}

struct ExportManifestFileFixture<'a> {
    blake3: String,
    byte_length: u64,
    kind: &'static str,
    path: &'a str,
}

fn base_scene(tile_id: &str) -> JsonValue {
    let scope = StaticWorldScope::of_tile(tile_id).unwrap().scene_scope();
    json!({
        "colliders": [],
        "coordinateContract": {"schema": "fixture"},
        "coverage": BASE_COVERAGE,
        "customUnrelatedField": true,
        "models": [],
        "name": tile_id,
        "nativeTerrain": {"path": "terrain.json"},
        "provenance": {"sourceBuild": TUTORIAL_STATIC_WORLD_SOURCE_BUILD},
        "root": {"translation": [0, 0, 0]},
        "schema": WORLD_SCENE_SCHEMA,
        "scope": scope,
        "tile": tile_coordinates(tile_id).unwrap(),
        "visuals": []
    })
}

fn fixture_glb(image_uri: &str) -> Vec<u8> {
    let document = json!({
        "asset": {"version": "2.0"},
        "images": [{"uri": image_uri}],
        "scenes": [{"nodes": []}],
        "scene": 0
    });
    let mut json_bytes = serde_json::to_vec(&document).unwrap();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 20 + json_bytes.len();
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(b"glTF");
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&(total as u32).to_le_bytes());
    bytes.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
    bytes.extend_from_slice(&json_bytes);
    bytes
}

fn write_json_at(root: &Path, relative: &str, value: &impl Serialize) {
    let bytes = pretty_json(value).unwrap();
    write_new(&safe_join(root, relative).unwrap(), &bytes).unwrap();
}

fn hash_file_at(root: &Path, relative: &str) -> String {
    hash_bytes(&fs::read(safe_join(root, relative).unwrap()).unwrap())
}

fn write_project_manifest(asset_root: &Path, files: Vec<ProjectAssetFile>) {
    let manifest = ProjectAssetManifest {
        schema: PROJECT_ASSET_SCHEMA.to_owned(),
        protocol: 104,
        locale: "ru-RU".to_owned(),
        source_pack: SourcePackIdentity {
            schema: "fixture".to_owned(),
            manifest_blake3: "0".repeat(64),
        },
        files,
    };
    let bytes = pretty_json(&manifest).unwrap();
    write_new(&asset_root.join(ASSET_MANIFEST_FILE), &bytes).unwrap();
}
