use super::*;
use crate::{
    SourcePackIdentity,
    runtime_metadata_cleanup::{CleanRuntimeMetadataOptions, clean_runtime_metadata},
};
use serde_json::json;
use tempfile::TempDir;

const ENVIRONMENT_PATH: &str = "world/maps/map_00_00/terrain/environment/environment.json";
const TERRAIN_PATH: &str = "world/maps/map_00_00/terrain/terrain.json";
const SCENE_PATH: &str = "world/maps/map_00_00/scene.json";
const PROVENANCE_PATH: &str = "world/maps/map_00_00/provenance.json";
const CLEANUP_SEED_PATH: &str = "ui/character/catalog.json";
const CLEANUP_REVISION_PATH: &str = "ui/gameplay/catalog.json";
const TECHNICAL_PATHS: [&str; 8] = [
    LEGACY_WORLD_CATALOG_PATH,
    PROVENANCE_PATH,
    "world/maps/map_00_00/terrain/manifest.json",
    "world/maps/map_00_00/terrain/scene-instance.json",
    "world/maps/map_00_00/terrain/components/terrain.parsed.json",
    "world/maps/map_00_00/terrain/environment/source/environment.parsed.json",
    "world/maps/map_00_00/terrain/details/detail-database.raw.json",
    "world/maps/map_00_00/terrain/details/trees.raw.json",
];

struct Fixture {
    _temp: TempDir,
    project_root: PathBuf,
    original_terrain: Vec<u8>,
}

fn value_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap();
    bytes.push(b'\n');
    bytes
}

fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn add_asset(
    asset_root: &Path,
    relative: &str,
    bytes: &[u8],
    files: &mut Vec<ProjectAssetFile>,
) {
    let path = asset_root.join(native_path(relative).unwrap());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
    files.push(ProjectAssetFile {
        source_path: format!("fixture/{relative}"),
        path: relative.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: bytes.len() as u64,
        blake3: digest(bytes),
    });
}

fn upsert_manifest_asset(project_root: &Path, relative: &str, source_path: &str, bytes: &[u8]) {
    let asset_root = project_root.join("assets/game");
    let path = asset_root.join(native_path(relative).unwrap());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest.files.retain(|entry| entry.path != relative);
    manifest.files.push(ProjectAssetFile {
        source_path: source_path.to_owned(),
        path: relative.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: bytes.len() as u64,
        blake3: digest(bytes),
    });
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    fs::write(
        manifest_path,
        value_bytes(&serde_json::to_value(manifest).unwrap()),
    )
    .unwrap();
}

fn fixture_with_cleanup_revision() -> Fixture {
    let fixture = fixture();
    let asset_root = fixture.project_root.join("assets/game");
    let seed_path = asset_root.join(native_path(CLEANUP_SEED_PATH).unwrap());
    fs::create_dir_all(seed_path.parent().unwrap()).unwrap();
    fs::write(&seed_path, b"{\"seed\":true}\n").unwrap();
    clean_runtime_metadata(
        &CleanRuntimeMetadataOptions::new(&fixture.project_root, "retrobution-test")
            .with_apply(true),
    )
    .unwrap();
    assert!(!seed_path.exists());

    upsert_manifest_asset(
        &fixture.project_root,
        CLEANUP_REVISION_PATH,
        "fixture/original-gameplay-catalog",
        b"{\"version\":1}\n",
    );
    migrate_runtime_world(
        &RuntimeWorldMigrationOptions::new(&fixture.project_root, "retrobution-test")
            .with_apply(true),
    )
    .unwrap();
    let cleanup = clean_runtime_metadata(
        &CleanRuntimeMetadataOptions::new(&fixture.project_root, "retrobution-test")
            .with_apply(true),
    )
    .unwrap();
    assert!(
        cleanup
            .revision_archived
            .iter()
            .any(|entry| entry.source_path == CLEANUP_REVISION_PATH)
    );
    assert!(
        !asset_root
            .join(native_path(CLEANUP_REVISION_PATH).unwrap())
            .exists()
    );
    fixture
}

fn fixture() -> Fixture {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path().join("project");
    let asset_root = project_root.join("assets/game");
    fs::create_dir_all(&asset_root).unwrap();
    let mut files = Vec::new();

    let environment = value_bytes(&json!({
        "schema": "ffone.native-terrain-environment.v1",
        "mapScene": {"path": "Retrobution/map-scene"},
        "placementAudit": {"status": "complete"},
        "sourceCodeEvidence": [{"workspacePath": "Retrobution/source.cs"}],
        "ambience": {
            "sourceObject": {"pathId": 10},
            "sunColor": [1.0, 0.9, 0.8, 1.0],
            "sunIntensity": 0.75
        },
        "terrainDetail": {
            "sourceObject": {"pathId": 20},
            "terrainRendererSourceObject": {"pathId": 21},
            "drawTreesAndFoliage": true,
            "detailObjectDistance": 80.0
        },
        "runtimeFormulas": {
            "ambient": "sunColor * sunIntensity"
        }
    }));
    let environment_hash = digest(&environment);
    add_asset(&asset_root, ENVIRONMENT_PATH, &environment, &mut files);

    let terrain = value_bytes(&json!({
        "schema": "ffone.native-terrain.v1",
        "source": {
            "sourceBuild": "retrobution-test",
            "path": "Retrobution.resourceFile"
        },
        "sceneInstance": {
            "path": "scene-instance.json"
        },
        "environment": {
            "path": "environment/environment.json",
            "blake3": format!("blake3:{environment_hash}")
        },
        "gameplayAttributes": {
            "rawParsedDocument": {"path": "gameplay/attributes.raw.json"},
            "source": {"pointerPathId": 39},
            "rawPath": "gameplay/attributes.bin",
            "rawBlake3": format!("blake3:{}", "1".repeat(64))
        },
        "detailAndTrees": {
            "assetClosure": {"status": "complete"},
            "rawDocument": {"path": "details/detail-database.raw.json"},
            "preloadTextureAtlasData": {
                "entries": [{
                    "sourcePointer": {"fileId": 0, "pathId": 283},
                    "status": "coveredByPrototypeTextureExport"
                }]
            },
            "trees": {
                "rawDocument": {"path": "details/trees.raw.json"},
                "instances": []
            },
            "wavingGrass": {"amount": 0.5}
        },
        "splat": {
            "weightMaps": [{
                "source": {"pointerPathId": 1},
                "mips": [{
                    "sourceEncoded": {"path": "weights/mip.source.bin"},
                    "width": 1
                }]
            }],
            "layers": [{
                "mode": 0,
                "modeSource": "serializedField",
                "modeEvidence": {"field": "m_Splats[].mode"},
                "albedo": {
                    "source": {"pointerPathId": 2},
                    "mips": [{
                        "sourceEncoded": {"path": "layers/mip.source.bin"},
                        "width": 1
                    }]
                }
            }]
        },
        "lightmap": {
            "sourcePointer": {"pathId": 3},
            "source": {"resolvedAssetName": "Lightmap"},
            "runtimeSelection": "source terrain component flag",
            "mips": [{
                "sourceEncoded": {"path": "lightmap/mip.source.bin"},
                "width": 1
            }]
        },
        "heightmap": {
            "path": "heightmap.png",
            "rawPath": "heightmap.bin",
            "rawBlake3": format!("blake3:{}", "2".repeat(64))
        }
    }));
    let terrain_hash = digest(&terrain);
    add_asset(&asset_root, TERRAIN_PATH, &terrain, &mut files);

    let scene = value_bytes(&json!({
        "schema": "ffone.world-scene.v1",
        "name": "map_00_00",
        "provenance": {
            "sourceBuild": "retrobution-test",
            "path": PROVENANCE_PATH
        },
        "nativeTerrain": {
            "path": TERRAIN_PATH,
            "blake3": terrain_hash,
            "sceneInstancePath":
                "world/maps/map_00_00/terrain/scene-instance.json",
            "sceneInstanceBlake3": "3".repeat(64),
            "environment": {
                "path": ENVIRONMENT_PATH,
                "blake3": environment_hash
            }
        },
        "models": [],
        "colliders": []
    }));
    add_asset(&asset_root, SCENE_PATH, &scene, &mut files);

    add_asset(
        &asset_root,
        PROVENANCE_PATH,
        br#"{"schema":"fixture.provenance.v1"}"#,
        &mut files,
    );
    for relative in TECHNICAL_PATHS.iter().copied().skip(2) {
        add_asset(&asset_root, relative, b"{}\n", &mut files);
    }

    let catalog = value_bytes(&json!({
        "schema": LEGACY_WORLD_CATALOG_SCHEMA,
        "sourceBuild": "retrobution-test",
        "blocked": [],
        "entries": [{
            "instanceId": "map_00_00",
            "placementStatus": "linked",
            "provenance": PROVENANCE_PATH,
            "scene": SCENE_PATH,
            "sceneBlake3": "9".repeat(64),
            "scope": "worldMap",
            "terrainDescriptor": TERRAIN_PATH,
            "terrainDescriptorBlake3": terrain_hash,
            "tile": [0, 0],
            "environment": {
                "path": ENVIRONMENT_PATH,
                "blake3": environment_hash,
                "schema": "ffone.native-terrain-environment.v1",
                "status": "complete"
            }
        }]
    }));
    add_asset(&asset_root, LEGACY_WORLD_CATALOG_PATH, &catalog, &mut files);
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = ProjectAssetManifest {
        schema: PROJECT_ASSET_SCHEMA.to_owned(),
        protocol: 104,
        locale: "en-US".to_owned(),
        source_pack: SourcePackIdentity {
            schema: "fixture".to_owned(),
            manifest_blake3: "0".repeat(64),
        },
        files,
    };
    write_json_new(&asset_root.join(ASSET_MANIFEST_FILE), &manifest).unwrap();

    Fixture {
        _temp: temp,
        project_root,
        original_terrain: terrain,
    }
}

#[test]
fn dry_run_is_read_only_and_records_stale_catalog_hashes() {
    let fixture = fixture();
    let report = migrate_runtime_world(&RuntimeWorldMigrationOptions::new(
        &fixture.project_root,
        "retrobution-test",
    ))
    .unwrap();

    assert_eq!(report.mode, RuntimeWorldMigrationMode::DryRun);
    assert!(report.apply_ready);
    assert!(report.blockers.is_empty());
    assert_eq!(report.counts.registry_entries, 1);
    assert_eq!(report.counts.excluded_blocked_entries, 0);
    assert_eq!(report.counts.legacy_catalog_hash_drifts, 1);
    assert_eq!(report.counts.archived_technical_files, 8);
    assert_eq!(report.counts.rewritten_runtime_files, 3);
    assert_eq!(report.counts.removed_manifest_entries, 8);
    assert_eq!(report.counts.updated_manifest_entries, 3);
    assert_eq!(report.counts.added_manifest_entries, 1);
    assert_eq!(report.counts.manifest_files_before, 11);
    assert_eq!(report.counts.manifest_files_after, 4);
    assert!(report.counts.removed_evidence_fields >= 20);
    let [drift] = report.legacy_catalog_hash_drifts.as_slice() else {
        panic!("expected exactly one legacy catalog hash drift");
    };
    assert_eq!(drift.instance_id, "map_00_00");
    assert_eq!(drift.payload, "scene");
    assert_eq!(drift.path, SCENE_PATH);
    assert_eq!(drift.catalog_blake3, "9".repeat(64));
    assert!(
        !fixture
            .project_root
            .join("../FusionForge/work/ffone/migration-archive/retrobution-test/world-conversion-metadata")
            .exists()
    );
    assert!(
        !fixture
            .project_root
            .join("assets/game/_runtime/world.json")
            .exists()
    );
    for relative in TECHNICAL_PATHS {
        assert!(
            fixture
                .project_root
                .join("assets/game")
                .join(native_path(relative).unwrap())
                .is_file()
        );
    }
}

#[test]
fn apply_preserves_originals_and_publishes_a_manifest_owned_runtime_graph() {
    let fixture = fixture();
    let report = migrate_runtime_world(
        &RuntimeWorldMigrationOptions::new(&fixture.project_root, "retrobution-test")
            .with_apply(true),
    )
    .unwrap();

    assert_eq!(report.mode, RuntimeWorldMigrationMode::Apply);
    let asset_root = fixture.project_root.join("assets/game");
    let archive = fixture
        .project_root
        .join("../FusionForge/work/ffone/migration-archive/retrobution-test/world-conversion-metadata");
    assert!(archive.join("index.json").is_file());
    assert!(archive.join("report.json").is_file());
    assert!(archive.join("files/world/catalog.json").is_file());
    assert_eq!(
        fs::read(archive.join("runtime-originals").join(TERRAIN_PATH)).unwrap(),
        fixture.original_terrain
    );
    assert!(archive.join("runtime-next/_runtime/world.json").is_file());

    for relative in TECHNICAL_PATHS {
        assert!(!asset_root.join(native_path(relative).unwrap()).exists());
    }
    for relative in [ENVIRONMENT_PATH, TERRAIN_PATH, SCENE_PATH] {
        let bytes = fs::read(asset_root.join(native_path(relative).unwrap())).unwrap();
        audit_sanitized_json(relative, &bytes).unwrap();
    }
    let terrain: Value = serde_json::from_slice(
        &fs::read(asset_root.join(native_path(TERRAIN_PATH).unwrap())).unwrap(),
    )
    .unwrap();
    assert!(
        terrain["lightmap"]
            .as_object()
            .unwrap()
            .get("sourcePointer")
            .is_none()
    );
    assert!(
        terrain["detailAndTrees"]["preloadTextureAtlasData"]["entries"][0]
            .as_object()
            .unwrap()
            .contains_key("sourcePointer")
    );

    let registry_bytes =
        fs::read(asset_root.join(native_path(RUNTIME_WORLD_REGISTRY_PATH).unwrap())).unwrap();
    audit_sanitized_json(RUNTIME_WORLD_REGISTRY_PATH, &registry_bytes).unwrap();
    let registry: RuntimeWorldRegistry = serde_json::from_slice(&registry_bytes).unwrap();
    assert_eq!(registry.schema, RUNTIME_WORLD_REGISTRY_SCHEMA);
    let [entry] = registry.entries.as_slice() else {
        panic!("expected exactly one registry entry");
    };
    assert_eq!(entry.id, "map_00_00");
    assert_eq!(entry.scope, "worldMap");
    assert_eq!(entry.tile, [0, 0]);
    for reference in std::iter::once(&entry.scene)
        .chain(std::iter::once(&entry.terrain))
        .chain(entry.environment.iter())
    {
        let bytes = fs::read(asset_root.join(native_path(&reference.path).unwrap())).unwrap();
        assert_eq!(reference.blake3, digest(&bytes));
    }

    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap())
            .unwrap();
    assert_eq!(manifest.files.len(), 4);
    assert_eq!(
        manifest
            .files
            .iter()
            .filter(|entry| entry.path == RUNTIME_WORLD_REGISTRY_PATH)
            .count(),
        1
    );
    for entry in &manifest.files {
        let bytes = fs::read(asset_root.join(native_path(&entry.path).unwrap())).unwrap();
        assert_eq!(entry.bytes, bytes.len() as u64);
        assert_eq!(entry.blake3, digest(&bytes));
    }
    assert!(fs::read_dir(&asset_root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".runtime-world-")
    }));

    let repeat = migrate_runtime_world(&RuntimeWorldMigrationOptions::new(
        &fixture.project_root,
        "retrobution-test",
    ))
    .unwrap();
    assert_eq!(repeat.mode, RuntimeWorldMigrationMode::DryRun);
    assert!(repeat.apply_ready);
    assert_eq!(repeat.counts, report.counts);
    assert_eq!(repeat.registry, report.registry);
    assert_eq!(
        repeat.legacy_catalog_hash_drifts,
        report.legacy_catalog_hash_drifts
    );
}

#[test]
fn completed_archive_accepts_a_manifest_and_disk_verified_cleanup_republication() {
    let fixture = fixture_with_cleanup_revision();
    upsert_manifest_asset(
        &fixture.project_root,
        CLEANUP_REVISION_PATH,
        "fixture/republished-gameplay-catalog",
        b"{\"version\":2}\n",
    );

    let report = migrate_runtime_world(&RuntimeWorldMigrationOptions::new(
        &fixture.project_root,
        "retrobution-test",
    ))
    .unwrap();
    assert_eq!(report.mode, RuntimeWorldMigrationMode::DryRun);
    assert!(report.apply_ready);
}

#[test]
fn completed_archive_rejects_cleanup_republication_disk_drift() {
    let fixture = fixture_with_cleanup_revision();
    upsert_manifest_asset(
        &fixture.project_root,
        CLEANUP_REVISION_PATH,
        "fixture/republished-gameplay-catalog",
        b"{\"version\":2}\n",
    );
    let path = fixture
        .project_root
        .join("assets/game")
        .join(native_path(CLEANUP_REVISION_PATH).unwrap());
    fs::write(path, b"{\"version\":3}\n").unwrap();

    let error = migrate_runtime_world(&RuntimeWorldMigrationOptions::new(
        &fixture.project_root,
        "retrobution-test",
    ))
    .unwrap_err()
    .to_string();
    assert!(error.contains("differs from current manifest"));
}

#[test]
fn completed_archive_rejects_duplicate_cleanup_revision_paths() {
    let fixture = fixture_with_cleanup_revision();
    upsert_manifest_asset(
        &fixture.project_root,
        CLEANUP_REVISION_PATH,
        "fixture/second-gameplay-catalog",
        b"{\"version\":2}\n",
    );
    clean_runtime_metadata(
        &CleanRuntimeMetadataOptions::new(&fixture.project_root, "retrobution-test")
            .with_apply(true),
    )
    .unwrap();

    let error = migrate_runtime_world(&RuntimeWorldMigrationOptions::new(
        &fixture.project_root,
        "retrobution-test",
    ))
    .unwrap_err()
    .to_string();
    assert!(error.contains("repeat manifested path"));
}

#[test]
fn completed_archive_tampering_blocks_idempotent_dry_run() {
    let fixture = fixture();
    migrate_runtime_world(
        &RuntimeWorldMigrationOptions::new(&fixture.project_root, "retrobution-test")
            .with_apply(true),
    )
    .unwrap();
    let archived_catalog = fixture.project_root.join(
        "../FusionForge/work/ffone/migration-archive/retrobution-test/world-conversion-metadata/files/world/catalog.json",
    );
    fs::write(&archived_catalog, b"{}\n").unwrap();

    let error = migrate_runtime_world(&RuntimeWorldMigrationOptions::new(
        &fixture.project_root,
        "retrobution-test",
    ))
    .unwrap_err()
    .to_string();
    assert!(error.contains("identity mismatch"));
}

#[test]
fn manifest_identity_mismatch_blocks_before_any_write() {
    let fixture = fixture();
    let catalog_path = fixture
        .project_root
        .join("assets/game")
        .join(native_path(LEGACY_WORLD_CATALOG_PATH).unwrap());
    let mut tampered = fs::read(&catalog_path).unwrap();
    tampered.push(b' ');
    fs::write(&catalog_path, tampered).unwrap();

    let error = migrate_runtime_world(&RuntimeWorldMigrationOptions::new(
        &fixture.project_root,
        "retrobution-test",
    ))
    .unwrap_err()
    .to_string();
    assert!(error.contains("world JSON manifest identity mismatch"));
    assert!(
        !fixture
            .project_root
            .join("../FusionForge/work/ffone/migration-archive/retrobution-test/world-conversion-metadata")
            .exists()
    );
    assert!(
        !fixture
            .project_root
            .join("assets/game/_runtime/world.json")
            .exists()
    );
}
