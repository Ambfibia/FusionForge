use super::*;

pub(super) fn copy_file(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::copy(source, destination).unwrap_or_else(|error| {
        panic!(
            "failed to copy {} to {}: {error}",
            source.display(),
            destination.display()
        )
    });
}

pub(super) fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            copy_file(&source_path, &destination_path);
        }
    }
}

pub(super) fn native_ui_work_copy() -> (PathBuf, PathBuf) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let project = std::env::temp_dir().join(format!(
        "ffone-native-ui-install-{}-{stamp}",
        std::process::id()
    ));
    let source_project = workspace_root();
    let source_assets = source_project.join("assets").join("game");
    let asset_root = project.join("assets").join("game");
    fs::create_dir_all(&asset_root).unwrap();
    copy_file(
        &source_assets.join(ASSET_MANIFEST_FILE),
        &asset_root.join(ASSET_MANIFEST_FILE),
    );
    copy_tree(
        &source_assets.join("ui").join("gameplay"),
        &asset_root.join("ui").join("gameplay"),
    );
    for source in ROUTES
        .iter()
        .map(|route| route.source)
        .chain(VERIFIED_ICON_ROUTES.iter().map(|route| route.source))
    {
        let relative = source.replace('/', std::path::MAIN_SEPARATOR_STR);
        copy_file(&source_assets.join(&relative), &asset_root.join(&relative));
    }

    let revision_relative = Path::new("content")
        .join("imported")
        .join("retrobution-20260613")
        .join("conversion-metadata")
        .join("revisions")
        .join(ARCHIVED_GAMEPLAY_UI_REVISION);
    let source_revision = source_project.join(&revision_relative);
    let revision = project.join(&revision_relative);
    copy_file(
        &source_revision.join("index.json"),
        &revision.join("index.json"),
    );
    copy_file(
        &source_revision
            .join("files")
            .join("ui")
            .join("gameplay")
            .join("catalog.json"),
        &revision
            .join("files")
            .join("ui")
            .join("gameplay")
            .join("catalog.json"),
    );
    (project, asset_root)
}

#[test]
fn runtime_only_install_is_idempotent_and_rolls_back_stage_failure() {
    let (project, asset_root) = native_ui_work_copy();
    let options = NativeGameplayUiInstallOptions {
        asset_root: asset_root.clone(),
        source_build: "retrobution-20260613-test-copy".to_owned(),
    };
    let runtime = asset_root.join("ui").join("gameplay");
    let baseline_manifest = fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap();
    let baseline_manifest_value: ProjectAssetManifest =
        serde_json::from_slice(&baseline_manifest).unwrap();
    let baseline_manifest_sha256 = sha256_hex(&baseline_manifest);
    let baseline_manifest_blake3 = blake3::hash(&baseline_manifest).to_hex().to_string();
    let mut baseline_tree = BTreeMap::new();
    collect_runtime_files(&runtime, &runtime, &mut baseline_tree).unwrap();
    let baseline_tree_sha256 = runtime_tree_sha256(&baseline_tree);

    let first = install_native_gameplay_ui(&options).unwrap();
    assert_eq!(
        first.installed_files,
        (ROUTES.len() + VERIFIED_ICON_ROUTES.len() + 2) as u64
    );
    assert!(!asset_root.join("ui/gameplay/catalog.json").exists());
    let first_manifest = fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap();
    let first_manifest_value: ProjectAssetManifest =
        serde_json::from_slice(&first_manifest).unwrap();
    assert!(
        first_manifest_value
            .files
            .iter()
            .all(|entry| entry.path != GAMEPLAY_UI_CATALOG)
    );
    let mut first_tree = BTreeMap::new();
    collect_runtime_files(&runtime, &runtime, &mut first_tree).unwrap();
    let first_manifest_sha256 = sha256_hex(&first_manifest);
    let first_manifest_blake3 = blake3::hash(&first_manifest).to_hex().to_string();
    let first_tree_sha256 = runtime_tree_sha256(&first_tree);
    let added_routes = first_tree
        .iter()
        .filter(|(path, _)| !baseline_tree.contains_key(*path))
        .map(|(path, bytes)| {
            (
                path.clone(),
                bytes.len() as u64,
                blake3::hash(bytes).to_hex().to_string(),
            )
        })
        .collect::<Vec<_>>();
    assert!(added_routes.iter().all(|(path, _, _)| {
        desired_route_sources().contains_key(path) || path == GAMEPLAY_UI_LAYOUT
    }));
    assert_eq!(first_tree.len(), first.installed_files as usize);
    assert_eq!(
        first_manifest_value.files.len(),
        baseline_manifest_value.files.len() - baseline_tree.len() + first_tree.len()
    );

    let second = install_native_gameplay_ui(&options).unwrap();
    assert_eq!(first, second);
    let second_manifest = fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap();
    assert_eq!(first_manifest, second_manifest);
    let mut second_tree = BTreeMap::new();
    collect_runtime_files(&runtime, &runtime, &mut second_tree).unwrap();
    assert_eq!(first_tree, second_tree);
    let second_manifest_sha256 = sha256_hex(&second_manifest);
    let second_tree_sha256 = runtime_tree_sha256(&second_tree);

    for failpoint in [
        NativeGameplayUiInstallFailpoint::CreateStage,
        NativeGameplayUiInstallFailpoint::CreateUiParent,
    ] {
        assert!(install_native_gameplay_ui_with_failpoint(&options, failpoint).is_err());
        assert_eq!(
            first_manifest,
            fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap(),
            "manifest changed at {failpoint:?}"
        );
        let mut after_failpoint_tree = BTreeMap::new();
        collect_runtime_files(&runtime, &runtime, &mut after_failpoint_tree).unwrap();
        assert_eq!(
            first_tree, after_failpoint_tree,
            "runtime changed at {failpoint:?}"
        );
        assert!(
            !asset_root
                .join(format!(".gameplay-ui-backup-{}", std::process::id()))
                .exists()
        );
        assert!(
            !asset_root
                .join(format!(".gameplay-ui-stage-{}", std::process::id()))
                .exists()
        );
    }

    let stale_stage = asset_root.join(format!(".gameplay-ui-stage-{}", std::process::id()));
    fs::create_dir(&stale_stage).unwrap();
    assert!(install_native_gameplay_ui(&options).is_err());
    assert_eq!(
        first_manifest,
        fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap()
    );
    let mut after_stale_tree = BTreeMap::new();
    collect_runtime_files(&runtime, &runtime, &mut after_stale_tree).unwrap();
    assert_eq!(first_tree, after_stale_tree);
    assert!(
        !asset_root
            .join(format!(".gameplay-ui-backup-{}", std::process::id()))
            .exists()
    );
    fs::remove_dir(&stale_stage).unwrap();

    let strict_source = STRICT_ROUTE_PROOFS[0].source;
    let source_path =
        asset_root.join(strict_source.replace('/', std::path::MAIN_SEPARATOR_STR));
    let mut damaged_source = fs::read(&source_path).unwrap();
    damaged_source[0] ^= 0xff;
    fs::write(&source_path, damaged_source).unwrap();
    let before_failure_manifest = fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap();
    let before_failure_runtime =
        fs::read(runtime.join("mission/journal/acceptbut.png")).unwrap();

    assert!(install_native_gameplay_ui(&options).is_err());
    let after_failure_manifest = fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap();
    assert_eq!(before_failure_manifest, after_failure_manifest);
    assert_eq!(
        before_failure_runtime,
        fs::read(runtime.join("mission/journal/acceptbut.png")).unwrap()
    );
    let mut after_failure_tree = BTreeMap::new();
    collect_runtime_files(&runtime, &runtime, &mut after_failure_tree).unwrap();
    assert_eq!(first_tree, after_failure_tree);
    let rollback_manifest_sha256 = sha256_hex(&after_failure_manifest);
    let rollback_tree_sha256 = runtime_tree_sha256(&after_failure_tree);
    assert!(
        !asset_root
            .join(format!(".gameplay-ui-backup-{}", std::process::id()))
            .exists()
    );
    assert!(
        !asset_root
            .join(format!(".gameplay-ui-stage-{}", std::process::id()))
            .exists()
    );
    eprintln!(
        "native_ui_work_copy_proof baseline_manifest_files={} baseline_manifest_sha256={} baseline_manifest_blake3={} final_manifest_files={} final_manifest_sha256={} final_manifest_blake3={} baseline_runtime_files={} baseline_runtime_sha256={} final_runtime_files={} final_runtime_sha256={} added_count={} added_bytes={} added_routes={:?} second_manifest_sha256={} second_runtime_sha256={} rollback_manifest_sha256={} rollback_runtime_sha256={} stage_residue=false backup_residue=false",
        baseline_manifest_value.files.len(),
        baseline_manifest_sha256,
        baseline_manifest_blake3,
        first_manifest_value.files.len(),
        first_manifest_sha256,
        first_manifest_blake3,
        baseline_tree.len(),
        baseline_tree_sha256,
        first_tree.len(),
        first_tree_sha256,
        added_routes.len(),
        added_routes.iter().map(|(_, bytes, _)| bytes).sum::<u64>(),
        added_routes,
        second_manifest_sha256,
        second_tree_sha256,
        rollback_manifest_sha256,
        rollback_tree_sha256,
    );
    fs::remove_dir_all(project).unwrap();
}
