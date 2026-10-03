use super::*;

pub(super) fn add_asset(
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
        blake3: blake3::hash(bytes).to_hex().to_string(),
    });
}

pub(super) fn tiny_offline_content_index_fixture() -> (
    TempDir,
    PathBuf,
    ProjectAssetManifest,
    ProvenOfflineContentIndexProof,
) {
    let temp = TempDir::new().unwrap();
    let asset_root = temp.path().join("assets/game");
    fs::create_dir_all(&asset_root).unwrap();
    let relative = "data/catalog/content-index--fixture.json";
    let bytes = serde_json::to_vec(&serde_json::json!({
        "assets": [],
        "complete": false,
        "nativeOnly": true,
        "profile": "core-v1",
        "schema": "ffone.asset-index.v1"
    }))
    .unwrap();
    let mut files = Vec::new();
    add_asset(&asset_root, relative, &bytes, &mut files);
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
    let proof = ProvenOfflineContentIndexProof {
        path: relative.to_owned(),
        source_path: format!("fixture/{relative}"),
        bytes: bytes.len() as u64,
        blake3: blake3::hash(&bytes).to_hex().to_string(),
        schema: PROVEN_OFFLINE_CONTENT_INDEX_SCHEMA.to_owned(),
        profile: PROVEN_OFFLINE_CONTENT_INDEX_PROFILE.to_owned(),
    };
    (temp, asset_root, manifest, proof)
}

#[test]
fn proven_offline_content_index_is_exact_and_archive_idempotent() {
    let (_temp, asset_root, mut manifest, proof) = tiny_offline_content_index_fixture();
    assert_eq!(
        classify_metadata("data/catalog/content-index--new-sibling.json"),
        MetadataDisposition::Keep
    );

    let mut wrong_source = manifest.clone();
    wrong_source.files[0].source_path = "wrong/source.json".to_owned();
    let error = plan_proven_offline_content_index(
        &asset_root,
        &wrong_source,
        &ArchiveHistory::default(),
        &proof,
        &BTreeSet::new(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("manifest/disk identity drift"));

    let initial = plan_proven_offline_content_index(
        &asset_root,
        &manifest,
        &ArchiveHistory::default(),
        &proof,
        &BTreeSet::new(),
    )
    .unwrap();
    assert_eq!(initial.archived.len(), 1);
    assert_eq!(initial.archived[0].bytes, proof.bytes);
    assert_eq!(
        initial.archived[0].manifest_source_path.as_deref(),
        Some(proof.source_path.as_str())
    );
    let mut history = ArchiveHistory::default();
    history.add(&initial.archived);
    fs::remove_file(asset_root.join(native_path(&proof.path).unwrap())).unwrap();
    manifest.files.clear();
    let idempotent = plan_proven_offline_content_index(
        &asset_root,
        &manifest,
        &history,
        &proof,
        &BTreeSet::new(),
    )
    .unwrap();
    assert!(idempotent.archived.is_empty());
    assert_eq!(idempotent.already_archived_files, 1);
}

#[test]
fn proven_offline_content_index_rejects_drift_and_active_references() {
    let (_temp, asset_root, mut manifest, proof) = tiny_offline_content_index_fixture();
    let reference = format!(r#"{{"contentIndex":"{}"}}"#, proof.path);
    add_asset(
        &asset_root,
        "data/semantic-plan.json",
        reference.as_bytes(),
        &mut manifest.files,
    );
    let target_paths = [proof.path.clone()].into_iter().collect::<BTreeSet<_>>();
    for reference in [
        proof.path.clone(),
        format!("assets\\game\\{}", proof.path.replace('/', "\\")),
    ] {
        let value = serde_json::json!({"contentIndex": reference});
        assert_eq!(
            find_archived_asset_reference("data/semantic-plan.json", &value, &target_paths)
                .map(|(_, target)| target),
            Some(proof.path.clone())
        );
    }
    let error = plan_proven_offline_content_index(
        &asset_root,
        &manifest,
        &ArchiveHistory::default(),
        &proof,
        &BTreeSet::new(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("to archived asset"));

    manifest
        .files
        .retain(|entry| entry.path != "data/semantic-plan.json");
    fs::remove_file(asset_root.join("data/semantic-plan.json")).unwrap();
    let drift = br#"{"assets":[],"complete":true,"nativeOnly":true,"profile":"core-v1","schema":"ffone.asset-index.v1"}"#;
    fs::write(asset_root.join(native_path(&proof.path).unwrap()), drift).unwrap();
    let entry = manifest
        .files
        .iter_mut()
        .find(|entry| entry.path == proof.path)
        .unwrap();
    entry.bytes = drift.len() as u64;
    entry.blake3 = blake3::hash(drift).to_hex().to_string();
    let error = plan_proven_offline_content_index(
        &asset_root,
        &manifest,
        &ArchiveHistory::default(),
        &proof,
        &BTreeSet::new(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("identity drift"));
}

pub(super) fn add_fixture_asset(project_root: &Path, relative: &str, bytes: &[u8]) {
    let asset_root = project_root.join("assets/game");
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    add_asset(&asset_root, relative, bytes, &mut manifest.files);
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    fs::remove_file(&manifest_path).unwrap();
    write_json_new(&manifest_path, &manifest).unwrap();
}

#[test]
fn apply_archives_non_world_files_and_updates_manifest_transactionally() {
    let (_temp, project_root) = fixture();
    let options =
        CleanRuntimeMetadataOptions::new(&project_root, "retrobution-test").with_apply(true);
    let report = clean_runtime_metadata(&options).unwrap();
    assert_eq!(report.mode, CleanRuntimeMetadataMode::Apply);
    let archive = project_root.join(
        "../FusionForge/work/ffone/migration-archive/retrobution-test/conversion-metadata",
    );
    assert!(archive.join("index.json").is_file());
    assert!(archive.join("report.json").is_file());
    assert!(archive.join("files/icons/catalog.json").is_file());
    assert!(!archive.join("files/localization/catalog.json").exists());
    assert!(archive.join("files/tutorial/models/catalog.json").is_file());
    assert!(archive.join("files/ui/character/catalog.json").is_file());
    assert!(archive.join("files/ui/gameplay/catalog.json").is_file());
    assert!(!project_root.join("assets/game/icons/catalog.json").exists());
    assert!(
        project_root
            .join("assets/game/localization/catalog.json")
            .is_file()
    );
    assert!(
        !project_root
            .join("assets/game/_runtime/localization.json")
            .exists()
    );
    assert!(
        project_root
            .join("assets/game/ui/gameplay/layout/gameplay_hud.json")
            .is_file()
    );
    assert!(
        project_root
            .join("assets/game/ui/gameplay/images/gameplay_hud.png")
            .is_file()
    );
    for relative in ["ui/character/catalog.json", "ui/gameplay/catalog.json"] {
        assert!(
            !project_root
                .join("assets/game")
                .join(native_path(relative).unwrap())
                .exists(),
            "{relative} is conversion-only installer metadata"
        );
    }
    for relative in ACTIVE_RUNTIME_CATALOGS {
        assert!(
            project_root
                .join("assets/game")
                .join(native_path(relative).unwrap())
                .is_file(),
            "{relative} must remain a runtime asset"
        );
    }
    assert!(
        project_root
            .join("assets/game/data/catalog/content-index.json")
            .is_file()
    );
    assert!(
        project_root
            .join("assets/game/world/maps/map_00_00/terrain/scene-instance.json")
            .is_file(),
        "world conversion metadata must remain fail-closed"
    );
    let manifest_bytes =
        fs::read(project_root.join("assets/game/asset-manifest.json")).unwrap();
    let manifest: ProjectAssetManifest = serde_json::from_slice(&manifest_bytes).unwrap();
    assert!(
        manifest
            .files
            .iter()
            .all(|entry| entry.path != "icons/catalog.json")
    );
    assert!(
        manifest
            .files
            .iter()
            .any(|entry| entry.path == LEGACY_LOCALIZATION_CATALOG)
    );
    assert!(
        manifest
            .files
            .iter()
            .all(|entry| entry.path != RUNTIME_LOCALIZATION_CATALOG)
    );
    for relative in ACTIVE_RUNTIME_CATALOGS {
        assert!(manifest.files.iter().any(|entry| entry.path == *relative));
    }
    for relative in ["ui/character/catalog.json", "ui/gameplay/catalog.json"] {
        assert!(manifest.files.iter().all(|entry| entry.path != relative));
    }
    for relative in [
        "ui/gameplay/layout/gameplay_hud.json",
        "ui/gameplay/images/gameplay_hud.png",
        "map/shared/effects/catalog.json",
        "map/shared/projectiles/catalog.json",
    ] {
        assert!(
            manifest.files.iter().any(|entry| entry.path == relative),
            "{relative} must remain manifest-owned runtime content"
        );
    }
    assert!(
        manifest
            .files
            .iter()
            .any(|entry| { entry.path == "world/maps/map_00_00/terrain/scene-instance.json" })
    );

    let repeat = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-test",
    ))
    .unwrap();
    assert_eq!(repeat.mode, CleanRuntimeMetadataMode::DryRun);
    assert!(repeat.apply_ready);
    assert_eq!(repeat.counts.archived_files, 0);
    assert_eq!(repeat.counts.removed_manifest_entries, 0);
    assert_eq!(
        repeat.counts.manifest_files_before,
        manifest.files.len() as u64
    );
    assert_eq!(
        repeat.counts.manifest_files_after,
        manifest.files.len() as u64
    );
    assert_eq!(repeat.counts.deferred_world_files, 1);
}

#[test]
fn archive_publication_failure_restores_manifest_and_every_live_source() {
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
    let manifest_before = plan.manifest.clone();
    let archived = plan.archived.clone();
    let mut fail_before_publish = || invalid("injected archive publication failure");

    let error = apply_plan_inner(&plan, "retrobution-test", &mut fail_before_publish)
        .unwrap_err()
        .to_string();
    assert!(error.contains("injected archive publication failure"));
    assert!(!archive_root.exists());

    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap())
            .unwrap();
    assert_eq!(manifest, manifest_before);
    for entry in &archived {
        validate_file_identity(
            &asset_root.join(native_path(&entry.source_path).unwrap()),
            entry.bytes,
            &entry.blake3,
            "test rollback live source",
        )
        .unwrap();
    }
    assert!(!asset_root.join(RUNTIME_LOCALIZATION_CATALOG).exists());
    assert!(fs::read_dir(&asset_root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".clean-runtime-metadata-")
    }));
    let archive_parent = archive_root.parent().unwrap();
    assert!(
        !archive_parent.exists()
            || fs::read_dir(archive_parent).unwrap().all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".conversion-metadata-"))
    );
}
