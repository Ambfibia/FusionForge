use super::*;
use crate::{ProjectAssetKind, SourcePackIdentity};
use tempfile::tempdir;

fn write_fixture(root: &Path, mismatch: Option<&str>, reference: Option<&str>) {
    let mut files = Vec::new();
    for mapping in DUPLICATE_PACKAGES {
        for package_root in [mapping.redundant_root, mapping.canonical_root] {
            let path = root.join(native_path(package_root)).join("model.glb");
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let payload =
                if mismatch == Some(mapping.id) && package_root == mapping.redundant_root {
                    b"different".as_slice()
                } else {
                    mapping.id.as_bytes()
                };
            fs::write(&path, payload).unwrap();
            files.push(ProjectAssetFile {
                source_path: package_root.to_owned(),
                path: format!("{package_root}/model.glb"),
                kind: ProjectAssetKind::Model,
                bytes: payload.len() as u64,
                blake3: blake3::hash(payload).to_hex().to_string(),
            });
        }
    }
    if let Some(redundant_root) = reference {
        let path = root.join("data/catalog.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let payload = format!("{{\"model\":\"{redundant_root}/model.glb\"}}\n");
        fs::write(&path, payload.as_bytes()).unwrap();
        files.push(ProjectAssetFile {
            source_path: "test/catalog".to_owned(),
            path: "data/catalog.json".to_owned(),
            kind: ProjectAssetKind::Data,
            bytes: payload.len() as u64,
            blake3: blake3::hash(payload.as_bytes()).to_hex().to_string(),
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = ProjectAssetManifest {
        schema: PROJECT_ASSET_SCHEMA.to_owned(),
        protocol: 104,
        locale: "ru-RU".to_owned(),
        source_pack: SourcePackIdentity {
            schema: "ffone.content-pack.v1".to_owned(),
            manifest_blake3: "0".repeat(64),
        },
        files,
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    bytes.push(b'\n');
    fs::write(root.join(ASSET_MANIFEST_FILE), bytes).unwrap();
}

#[test]
fn dry_run_proves_all_packages_without_mutating_assets() {
    let root = tempdir().unwrap();
    write_fixture(root.path(), None, None);
    let manifest_before = fs::read(root.path().join(ASSET_MANIFEST_FILE)).unwrap();

    let report = dedupe_tutorial_character_models(&TutorialCharacterModelDedupeOptions::new(
        root.path(),
    ))
    .unwrap();

    assert_eq!(report.mode, TutorialCharacterModelDedupeMode::DryRun);
    assert_eq!(report.counts.proven_packages, 12);
    assert_eq!(report.counts.removed_packages, 12);
    assert_eq!(report.counts.removed_files, 12);
    assert_eq!(
        fs::read(root.path().join(ASSET_MANIFEST_FILE)).unwrap(),
        manifest_before
    );
    assert!(
        root.path()
            .join(native_path(DUPLICATE_PACKAGES[0].redundant_root))
            .is_dir()
    );
}

#[test]
fn apply_is_manifest_safe_and_idempotent() {
    let root = tempdir().unwrap();
    write_fixture(root.path(), None, None);

    let report = dedupe_tutorial_character_models(
        &TutorialCharacterModelDedupeOptions::new(root.path()).with_apply(true),
    )
    .unwrap();
    assert_eq!(report.counts.removed_packages, 12);
    assert_eq!(report.counts.manifest_files_before, 24);
    assert_eq!(report.counts.manifest_files_after, 12);
    for mapping in DUPLICATE_PACKAGES {
        assert!(
            !root
                .path()
                .join(native_path(mapping.redundant_root))
                .exists()
        );
        assert!(
            root.path()
                .join(native_path(mapping.canonical_root))
                .is_dir()
        );
    }

    let repeated = dedupe_tutorial_character_models(
        &TutorialCharacterModelDedupeOptions::new(root.path()).with_apply(true),
    )
    .unwrap();
    assert_eq!(repeated.counts.removed_packages, 0);
    assert_eq!(repeated.counts.already_deduplicated_packages, 12);
    assert!(!repeated.requires_asset_index_regeneration);
}

#[test]
fn rejects_any_package_that_is_not_byte_exact() {
    let root = tempdir().unwrap();
    write_fixture(root.path(), Some("npc_dexter"), None);
    let error = dedupe_tutorial_character_models(
        &TutorialCharacterModelDedupeOptions::new(root.path()).with_apply(true),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("npc_dexter"));
    assert!(root.path().join("tutorial/models/mob/npc_dexter").is_dir());
}

#[test]
fn rejects_active_data_references_to_redundant_paths() {
    let root = tempdir().unwrap();
    let redundant_root = DUPLICATE_PACKAGES[0].redundant_root;
    write_fixture(root.path(), None, Some(redundant_root));
    let error = dedupe_tutorial_character_models(&TutorialCharacterModelDedupeOptions::new(
        root.path(),
    ))
    .unwrap_err()
    .to_string();
    assert!(error.contains("data/catalog.json"));
    assert!(error.contains(redundant_root));
}
