use super::*;

pub(super) fn fixture() -> (TempDir, PathBuf) {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path().join("project");
    let asset_root = project_root.join("assets/game");
    fs::create_dir_all(&asset_root).unwrap();
    let mut files = Vec::new();
    add_asset(
        &asset_root,
        "characters/registry.json",
        br#"{"schema":"runtime"}"#,
        &mut files,
    );
    add_asset(
        &asset_root,
        "characters/npc/test/test.publish.json",
        br#"{"schema":"publish"}"#,
        &mut files,
    );
    add_asset(
        &asset_root,
        "icons/catalog.json",
        br#"{"schema":"icon-import"}"#,
        &mut files,
    );
    add_asset(
        &asset_root,
        "ui/gameplay/catalog.json",
        br#"{"schema":"runtime-gameplay-ui"}"#,
        &mut files,
    );
    add_asset(
        &asset_root,
        "ui/character/catalog.json",
        br#"{"schema":"runtime-character-ui"}"#,
        &mut files,
    );
    add_asset(
        &asset_root,
        "ui/gameplay/layout/gameplay_hud.json",
        br#"{"schema":"runtime-layout"}"#,
        &mut files,
    );
    add_asset(
        &asset_root,
        "ui/gameplay/images/gameplay_hud.png",
        b"fixture-runtime-ui-image",
        &mut files,
    );
    for relative in [
        "map/shared/effects/catalog.json",
        "map/shared/projectiles/catalog.json",
    ] {
        add_asset(
            &asset_root,
            relative,
            br#"{"schema":"runtime-tutorial-catalog"}"#,
            &mut files,
        );
    }
    for relative in [
        "data/character_creation/appearance.json",
        "data/character_creation/avatar_items.json",
        "data/character_creation/runtime_textures.json",
    ] {
        add_asset(
            &asset_root,
            relative,
            br#"{"schema":"runtime-character-creation"}"#,
            &mut files,
        );
    }
    add_asset(
        &asset_root,
        "data/catalog/content-index.json",
        br#"{"schema":"source-index"}"#,
        &mut files,
    );
    add_asset(
        &asset_root,
        "data/catalog/avatar/test_cook_report.json",
        br#"{"schema":"conversion-report"}"#,
        &mut files,
    );
    add_asset(
        &asset_root,
        LEGACY_LOCALIZATION_CATALOG,
        br#"{"schema":"ffone.localization-catalog.v1"}"#,
        &mut files,
    );
    add_asset(
        &asset_root,
        "tutorial/models/catalog.json",
        br#"{"schema":"tutorial-model-import"}"#,
        &mut files,
    );
    add_asset(
        &asset_root,
        "world/maps/map_00_00/terrain/scene-instance.json",
        br#"{"schema":"conversion-evidence"}"#,
        &mut files,
    );
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
    write_json_new(&asset_root.join(ASSET_MANIFEST_FILE), &manifest).unwrap();
    (temp, project_root)
}

pub(super) fn add_static_world_fixture(project_root: &Path, with_runtime_registry: bool) {
    let tile_id = "tile_00_00";
    let scene_relative = format!("world/tutorial/terrain/tiles/{tile_id}/scene.json");
    let scene_bytes = br#"{"schema":"ffone.native-world-scene.v2","static":{"models":[]}}"#;
    add_fixture_asset(project_root, &scene_relative, scene_bytes);
    add_fixture_asset(
        project_root,
        TUTORIAL_STATIC_INSTALL_MANIFEST,
        br#"{"schema":"ffone.tutorial-static-world-install.v2"}"#,
    );
    for file in ["catalog.json", "hierarchy.json", "materials.json"] {
        add_fixture_asset(
            project_root,
            &format!("world/tutorial/static/tiles/{tile_id}/{file}"),
            format!(r#"{{"schema":"fixture.{file}"}}"#).as_bytes(),
        );
    }
    add_fixture_asset(
        project_root,
        "models/world/tutorial/tile_00_00/tree.glb",
        b"fixture-glb",
    );
    add_fixture_asset(
        project_root,
        "models/world/tutorial/tile_00_00/tree.png",
        b"fixture-png",
    );
    if with_runtime_registry {
        let scene_blake3 = blake3::hash(scene_bytes).to_hex().to_string();
        let registry = RuntimeWorldRegistry {
            schema: RUNTIME_WORLD_REGISTRY_SCHEMA.to_owned(),
            entries: vec![RuntimeWorldRegistryEntry {
                id: tile_id.to_owned(),
                scope: "tutorial".to_owned(),
                tile: [0, 0],
                scene: RuntimeWorldContentReference {
                    path: scene_relative,
                    blake3: scene_blake3,
                },
                terrain: RuntimeWorldContentReference {
                    path: format!("world/tutorial/terrain/tiles/{tile_id}/terrain.json"),
                    blake3: "0".repeat(64),
                },
                environment: None,
            }],
        };
        add_fixture_asset(
            project_root,
            RUNTIME_WORLD_REGISTRY_PATH,
            &serde_json::to_vec(&registry).unwrap(),
        );
    }
}

#[test]
fn completed_archive_tampering_fails_the_idempotent_dry_run() {
    let (_temp, project_root) = fixture();
    clean_runtime_metadata(
        &CleanRuntimeMetadataOptions::new(&project_root, "retrobution-test").with_apply(true),
    )
    .unwrap();
    let archived_icon = project_root
        .join("../FusionForge/work/ffone/migration-archive/retrobution-test/conversion-metadata/files/icons/catalog.json");
    fs::write(&archived_icon, b"{}\n").unwrap();

    let error = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap_err()
    .to_string();
    assert!(error.contains("archived payload identity mismatch"));
}

#[test]
fn same_hash_reappearance_is_removal_only_and_does_not_mutate_primary_archive() {
    let (_temp, project_root) = fixture();
    let apply =
        CleanRuntimeMetadataOptions::new(&project_root, "retrobution-test").with_apply(true);
    clean_runtime_metadata(&apply).unwrap();
    let archive = project_root.join(
        "../FusionForge/work/ffone/migration-archive/retrobution-test/conversion-metadata",
    );
    let primary_index_before = fs::read(archive.join("index.json")).unwrap();
    let icon_bytes = br#"{"schema":"icon-import"}"#;
    add_fixture_asset(&project_root, "icons/catalog.json", icon_bytes);

    let dry = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap();
    assert_eq!(dry.counts.archived_files, 1);
    assert_eq!(dry.counts.removal_only_files, 1);
    assert_eq!(dry.counts.revision_archived_files, 0);
    assert!(dry.revision_plan_blake3.is_none());

    let applied = clean_runtime_metadata(&apply).unwrap();
    assert_eq!(applied.counts.removal_only_files, 1);
    assert!(!project_root.join("assets/game/icons/catalog.json").exists());
    assert_eq!(
        fs::read(archive.join("index.json")).unwrap(),
        primary_index_before
    );
    assert!(!archive.join(ARCHIVE_REVISIONS_DIRECTORY).exists());
    let repeat = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap();
    assert_eq!(repeat.counts.archived_files, 0);
}

#[test]
fn changed_reappearance_preserves_primary_and_creates_deterministic_revision() {
    let (_temp, project_root) = fixture();
    let apply =
        CleanRuntimeMetadataOptions::new(&project_root, "retrobution-test").with_apply(true);
    clean_runtime_metadata(&apply).unwrap();
    let archive = project_root.join(
        "../FusionForge/work/ffone/migration-archive/retrobution-test/conversion-metadata",
    );
    let primary_index_before = fs::read(archive.join("index.json")).unwrap();
    let primary_icon_before = fs::read(archive.join("files/icons/catalog.json")).unwrap();
    let changed = br#"{"schema":"icon-import","installerRevision":2}"#;
    add_fixture_asset(&project_root, "icons/catalog.json", changed);

    let dry = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap();
    assert_eq!(dry.counts.revision_archived_files, 1);
    assert_eq!(dry.counts.removal_only_files, 0);
    let plan_blake3 = dry.revision_plan_blake3.clone().unwrap();
    assert_eq!(plan_blake3.len(), 64);

    let applied = clean_runtime_metadata(&apply).unwrap();
    assert_eq!(
        applied.revision_plan_blake3.as_deref(),
        Some(plan_blake3.as_str())
    );
    let revision = archive.join(ARCHIVE_REVISIONS_DIRECTORY).join(&plan_blake3);
    assert!(revision.join("index.json").is_file());
    assert!(revision.join("report.json").is_file());
    assert_eq!(
        fs::read(revision.join("files/icons/catalog.json")).unwrap(),
        changed
    );
    assert_eq!(
        fs::read(archive.join("index.json")).unwrap(),
        primary_index_before
    );
    assert_eq!(
        fs::read(archive.join("files/icons/catalog.json")).unwrap(),
        primary_icon_before
    );
    let repeat = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap();
    assert_eq!(repeat.counts.archived_files, 0);
}

#[test]
fn supplemental_revision_tampering_fails_closed() {
    let (_temp, project_root) = fixture();
    let apply =
        CleanRuntimeMetadataOptions::new(&project_root, "retrobution-test").with_apply(true);
    clean_runtime_metadata(&apply).unwrap();
    add_fixture_asset(
        &project_root,
        "icons/catalog.json",
        br#"{"schema":"icon-import","installerRevision":2}"#,
    );
    let report = clean_runtime_metadata(&apply).unwrap();
    let revision_icon = project_root
        .join("../FusionForge/work/ffone/migration-archive/retrobution-test/conversion-metadata")
        .join(ARCHIVE_REVISIONS_DIRECTORY)
        .join(report.revision_plan_blake3.unwrap())
        .join("files/icons/catalog.json");
    fs::write(&revision_icon, b"{}\n").unwrap();

    let error = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap_err()
    .to_string();
    assert!(error.contains("archived payload identity mismatch"));
}

pub(super) fn orphan_world_fixture() -> (TempDir, PathBuf, ProvenOrphanWorldPayload) {
    let (temp, project_root) = fixture();
    let registry = RuntimeWorldRegistry {
        schema: RUNTIME_WORLD_REGISTRY_SCHEMA.to_owned(),
        entries: Vec::new(),
    };
    add_fixture_asset(
        &project_root,
        RUNTIME_WORLD_REGISTRY_PATH,
        &serde_json::to_vec(&registry).unwrap(),
    );
    let id = "map_99_99";
    let root = format!("world/maps/{id}");
    let terrain = br#"{"schema":"ffone.native-terrain.v1"}"#;
    let environment = br#"{"schema":"ffone.native-terrain-environment.v1"}"#;
    let payload = b"fixture-heightmap";
    add_fixture_asset(
        &project_root,
        &format!("{root}/terrain/terrain.json"),
        terrain,
    );
    add_fixture_asset(
        &project_root,
        &format!("{root}/terrain/environment/environment.json"),
        environment,
    );
    add_fixture_asset(
        &project_root,
        &format!("{root}/terrain/heightmap.bin"),
        payload,
    );
    let proof = ProvenOrphanWorldPayload {
        id: id.to_owned(),
        root,
        files: 3,
        json_files: 2,
        bytes: (terrain.len() + environment.len() + payload.len()) as u64,
        terrain_blake3: blake3::hash(terrain).to_hex().to_string(),
        environment_blake3: blake3::hash(environment).to_hex().to_string(),
    };
    (temp, project_root, proof)
}

#[test]
fn orphan_world_policy_is_exact_fail_closed_and_archive_idempotent() {
    let (_temp, project_root, proof) = orphan_world_fixture();
    let asset_root = project_root.join("assets/game");
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap())
            .unwrap();
    let mut by_path = BTreeMap::<&str, Vec<&ProjectAssetFile>>::new();
    for entry in &manifest.files {
        by_path.entry(&entry.path).or_default().push(entry);
    }
    let archive = ArchiveHistory::default();
    let planned = plan_proven_orphan_world_payloads(
        &asset_root,
        &manifest,
        &by_path,
        &archive,
        std::slice::from_ref(&proof),
    )
    .unwrap();
    assert!(planned.blockers.is_empty());
    assert_eq!(planned.payload_roots, [proof.root.clone()]);
    assert_eq!(planned.archived.len(), 3);
    assert_eq!(
        planned
            .archived
            .iter()
            .filter(|entry| entry.source_path.ends_with(".json"))
            .count(),
        2
    );
    assert!(
        planned
            .archived
            .iter()
            .all(|entry| entry.reason == ORPHAN_WORLD_PAYLOAD_REASON && entry.manifested)
    );

    let unknown =
        plan_proven_orphan_world_payloads(&asset_root, &manifest, &by_path, &archive, &[])
            .unwrap();
    assert!(!unknown.blockers.is_empty());
    assert!(unknown.archived.is_empty());

    let mut history = ArchiveHistory::default();
    history.add(&planned.archived);
    fs::remove_dir_all(asset_root.join(native_path(&proof.root).unwrap())).unwrap();
    fs::create_dir_all(
        asset_root
            .join(native_path(&proof.root).unwrap())
            .join("empty/nested"),
    )
    .unwrap();
    let mut next_manifest = manifest.clone();
    next_manifest
        .files
        .retain(|entry| !entry.path.starts_with(&format!("{}/", proof.root)));
    let mut next_by_path = BTreeMap::<&str, Vec<&ProjectAssetFile>>::new();
    for entry in &next_manifest.files {
        next_by_path.entry(&entry.path).or_default().push(entry);
    }
    let repeat = plan_proven_orphan_world_payloads(
        &asset_root,
        &next_manifest,
        &next_by_path,
        &history,
        std::slice::from_ref(&proof),
    )
    .unwrap();
    assert!(repeat.blockers.is_empty());
    assert!(repeat.archived.is_empty());
    assert_eq!(repeat.already_archived_roots, [proof.root.clone()]);

    fs::write(
        asset_root
            .join(native_path(&proof.root).unwrap())
            .join("unexpected.bin"),
        b"residual",
    )
    .unwrap();
    let residual = plan_proven_orphan_world_payloads(
        &asset_root,
        &next_manifest,
        &next_by_path,
        &history,
        std::slice::from_ref(&proof),
    )
    .unwrap();
    assert!(
        residual
            .blockers
            .iter()
            .any(|blocker| blocker.contains("unregistered residual files"))
    );
}

#[test]
fn orphan_world_policy_rejects_external_reference_and_anchor_change() {
    let (_temp, project_root, mut proof) = orphan_world_fixture();
    add_fixture_asset(
        &project_root,
        "data/catalog/orphan-reference.json",
        format!(r#"{{"path":"{}/terrain/terrain.json"}}"#, proof.root).as_bytes(),
    );
    let asset_root = project_root.join("assets/game");
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap())
            .unwrap();
    let mut by_path = BTreeMap::<&str, Vec<&ProjectAssetFile>>::new();
    for entry in &manifest.files {
        by_path.entry(&entry.path).or_default().push(entry);
    }
    let blocked = plan_proven_orphan_world_payloads(
        &asset_root,
        &manifest,
        &by_path,
        &ArchiveHistory::default(),
        std::slice::from_ref(&proof),
    )
    .unwrap();
    assert!(
        blocked
            .blockers
            .iter()
            .any(|blocker| blocker.contains("still references orphan world root"))
    );

    proof.terrain_blake3 = "0".repeat(64);
    let error = plan_proven_orphan_world_payloads(
        &asset_root,
        &manifest,
        &by_path,
        &ArchiveHistory::default(),
        std::slice::from_ref(&proof),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("proof anchor mismatch"));
}

#[test]
fn revision_publication_failure_preserves_primary_and_restores_live_source() {
    let (_temp, project_root) = fixture();
    let apply =
        CleanRuntimeMetadataOptions::new(&project_root, "retrobution-test").with_apply(true);
    clean_runtime_metadata(&apply).unwrap();
    let archive_root = project_root.join(
        "../FusionForge/work/ffone/migration-archive/retrobution-test/conversion-metadata",
    );
    let primary_index_before = fs::read(archive_root.join("index.json")).unwrap();
    let changed = br#"{"schema":"icon-import","installerRevision":2}"#;
    add_fixture_asset(&project_root, "icons/catalog.json", changed);

    let asset_root = fs::canonicalize(project_root.join("assets/game")).unwrap();
    let history = validate_completed_archive(&archive_root, "retrobution-test").unwrap();
    let plan = build_plan(
        asset_root.clone(),
        archive_root.clone(),
        "retrobution-test",
        &history,
    )
    .unwrap();
    assert_eq!(plan.revision_archived.len(), 1);
    assert!(plan.removal_only.is_empty());
    let revision_target = archive_root
        .join(ARCHIVE_REVISIONS_DIRECTORY)
        .join(plan.revision_plan_blake3.as_ref().unwrap());
    let manifest_before = plan.manifest.clone();
    let mut fail_before_publish = || invalid("injected revision publication failure");

    let error = apply_plan_inner(&plan, "retrobution-test", &mut fail_before_publish)
        .unwrap_err()
        .to_string();
    assert!(error.contains("injected revision publication failure"));
    assert!(!revision_target.exists());
    assert_eq!(
        fs::read(archive_root.join("index.json")).unwrap(),
        primary_index_before
    );
    assert_eq!(
        fs::read(asset_root.join("icons/catalog.json")).unwrap(),
        changed
    );
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap())
            .unwrap();
    assert_eq!(manifest, manifest_before);
    assert!(fs::read_dir(&asset_root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".clean-runtime-metadata-")
    }));
    assert!(
        fs::read_dir(archive_root.parent().unwrap())
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".conversion-metadata-revision-stage-"))
    );
}

#[test]
fn incomplete_rollback_preserves_backup_stage_and_journal() {
    let (_temp, project_root) = fixture();
    let asset_root = fs::canonicalize(project_root.join("assets/game")).unwrap();
    let archive_root = project_root.join(
        "../FusionForge/work/ffone/migration-archive/retrobution-test/conversion-metadata",
    );
    let plan = build_plan(
        asset_root.clone(),
        archive_root.clone(),
        "retrobution-test",
        &ArchiveHistory::default(),
    )
    .unwrap();
    let entry = plan.archived.first().unwrap().clone();
    let relative = native_path(&entry.source_path).unwrap();
    let live = asset_root.join(&relative);
    let transaction_root = asset_root.join(".clean-runtime-metadata-transaction-test");
    let backup = transaction_root.join("backup");
    let backup_file = backup.join(&relative);
    fs::create_dir_all(backup_file.parent().unwrap()).unwrap();
    fs::rename(&live, &backup_file).unwrap();
    fs::write(&live, b"conflicting concurrent content").unwrap();
    fs::write(transaction_root.join("plan.json"), b"journal").unwrap();
    let archive_stage = archive_root
        .parent()
        .unwrap()
        .join(".conversion-metadata-stage-test");
    fs::create_dir_all(&archive_stage).unwrap();
    fs::write(archive_stage.join("sentinel"), b"staged archive").unwrap();

    let error = rollback(
        &plan,
        &transaction_root,
        &backup,
        &transaction_root.join("manifest-next.json"),
        &transaction_root.join("manifest-backup.json"),
        Some(&archive_stage),
        std::slice::from_ref(&entry),
        &[],
        false,
        false,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("rollback is incomplete"));
    assert_eq!(fs::read(&live).unwrap(), b"conflicting concurrent content");
    validate_file_identity(
        &backup_file,
        entry.bytes,
        &entry.blake3,
        "preserved rollback backup",
    )
    .unwrap();
    assert!(transaction_root.join("plan.json").is_file());
    assert!(archive_stage.join("sentinel").is_file());
}
