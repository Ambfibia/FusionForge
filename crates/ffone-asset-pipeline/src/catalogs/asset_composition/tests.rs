use super::*;
use tempfile::TempDir;

fn write_root(
    parent: &Path,
    name: &str,
    protocol: u16,
    files: &[(&str, &[u8], ProjectAssetKind)],
) -> PathBuf {
    let root = parent.join(name);
    fs::create_dir(&root).unwrap();
    let mut entries = Vec::new();
    for (path, bytes, kind) in files {
        let target = join_manifest_path(&root, path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, bytes).unwrap();
        entries.push(ProjectAssetFile {
            source_path: format!("{name}/{path}"),
            path: (*path).to_owned(),
            kind: *kind,
            bytes: bytes.len() as u64,
            blake3: blake3::hash(bytes).to_hex().to_string(),
        });
    }
    let manifest = ProjectAssetManifest {
        schema: PROJECT_ASSET_SCHEMA.to_owned(),
        protocol,
        locale: "ru-RU".to_owned(),
        source_pack: SourcePackIdentity {
            schema: "ffone.content-pack.v1".to_owned(),
            manifest_blake3: blake3::hash(name.as_bytes()).to_hex().to_string(),
        },
        files: entries,
    };
    fs::write(
        root.join(ASSET_MANIFEST_FILE),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    root
}

#[test]
fn overlay_owns_only_declared_prefixes_and_inputs_stay_unchanged() {
    let temp = TempDir::new().unwrap();
    let base = write_root(
        temp.path(),
        "base",
        104,
        &[
            (
                "world/tutorial/scene.json",
                b"world",
                ProjectAssetKind::Data,
            ),
            ("audio/voice/line.ogg", b"old", ProjectAssetKind::Audio),
            ("textures/base.png", b"base", ProjectAssetKind::Texture),
        ],
    );
    let overlay = write_root(
        temp.path(),
        "overlay",
        104,
        &[
            ("audio/voice/line.ogg", b"new", ProjectAssetKind::Audio),
            ("audio/catalog.json", b"{}", ProjectAssetKind::Data),
            ("textures/raw.png", b"raw", ProjectAssetKind::Texture),
        ],
    );
    let output = temp.path().join("composed");
    let report = compose_asset_roots(
        &AssetCompositionOptions::new(&base, &overlay, &output).with_overlay_prefix("audio/"),
    )
    .unwrap();

    assert_eq!(
        fs::read(output.join("audio/voice/line.ogg")).unwrap(),
        b"new"
    );
    assert_eq!(
        fs::read(output.join("world/tutorial/scene.json")).unwrap(),
        b"world"
    );
    assert!(!output.join("textures/raw.png").exists());
    assert_eq!(fs::read(base.join("audio/voice/line.ogg")).unwrap(), b"old");
    assert_eq!(report.base_files_replaced, 1);
    assert_eq!(report.overlay_files_selected, 2);
    assert_eq!(report.resolved_path_conflicts, 1);
    assert!(output.join(ASSET_COMPOSITION_REPORT).is_file());

    let persisted_report: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join(ASSET_COMPOSITION_REPORT)).unwrap())
            .unwrap();
    assert_eq!(persisted_report, serde_json::to_value(&report).unwrap());
    let output_manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(output.join(ASSET_MANIFEST_FILE)).unwrap()).unwrap();
    assert_eq!(output_manifest.files.len() as u64, report.output_files);
    assert_eq!(
        output_manifest
            .files
            .iter()
            .map(|entry| entry.bytes)
            .sum::<u64>(),
        report.output_bytes
    );
    assert_eq!(
        output_manifest
            .files
            .iter()
            .map(
                |entry| fs::metadata(join_manifest_path(&output, &entry.path))
                    .unwrap()
                    .len()
            )
            .sum::<u64>(),
        report.output_bytes
    );
}

#[test]
fn incompatible_protocol_fails_without_creating_output() {
    let temp = TempDir::new().unwrap();
    let base = write_root(
        temp.path(),
        "base",
        104,
        &[("audio/a.ogg", b"a", ProjectAssetKind::Audio)],
    );
    let overlay = write_root(
        temp.path(),
        "overlay",
        105,
        &[("audio/a.ogg", b"b", ProjectAssetKind::Audio)],
    );
    let output = temp.path().join("composed");
    let error = compose_asset_roots(
        &AssetCompositionOptions::new(base, overlay, &output).with_overlay_prefix("audio/"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("protocol mismatch"));
    assert!(!output.exists());
}

#[test]
fn corrupt_selected_source_is_rejected_and_stage_is_removed() {
    let temp = TempDir::new().unwrap();
    let base = write_root(
        temp.path(),
        "base",
        104,
        &[("world/a.json", b"a", ProjectAssetKind::Data)],
    );
    let overlay = write_root(
        temp.path(),
        "overlay",
        104,
        &[("audio/a.ogg", b"good", ProjectAssetKind::Audio)],
    );
    fs::write(overlay.join("audio/a.ogg"), b"evil").unwrap();
    let output = temp.path().join("composed");
    let error = compose_asset_roots(
        &AssetCompositionOptions::new(base, overlay, &output).with_overlay_prefix("audio/"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("BLAKE3 mismatch"));
    assert!(!output.exists());
    assert!(fs::read_dir(temp.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("stage")
    }));
}

#[test]
fn unsafe_or_overlapping_prefixes_are_rejected() {
    let error =
        normalize_prefixes(&["audio/".to_owned(), "audio/voice/".to_owned()]).unwrap_err();
    assert!(error.to_string().contains("must not overlap"));
    assert!(normalize_prefixes(&["../audio/".to_owned()]).is_err());
    assert!(normalize_prefixes(&["audio".to_owned()]).is_err());
}

#[test]
fn case_insensitive_collision_is_rejected_even_outside_selected_overlay() {
    let temp = TempDir::new().unwrap();
    let base = write_root(
        temp.path(),
        "base",
        104,
        &[("world/a.json", b"a", ProjectAssetKind::Data)],
    );
    let overlay = write_root(
        temp.path(),
        "overlay",
        104,
        &[
            ("audio/a.ogg", b"a", ProjectAssetKind::Audio),
            ("textures/a.png", b"x", ProjectAssetKind::Texture),
        ],
    );
    let manifest_path = overlay.join(ASSET_MANIFEST_FILE);
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let mut duplicate = manifest.files[1].clone();
    duplicate.path = "Textures/A.PNG".to_owned();
    duplicate.source_path = "overlay/Textures/A.PNG".to_owned();
    manifest.files.push(duplicate);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let output = temp.path().join("composed");
    let error = compose_asset_roots(
        &AssetCompositionOptions::new(base, overlay, &output).with_overlay_prefix("audio/"),
    )
    .unwrap_err();
    assert!(matches!(error, PipelineError::OutputCollision { .. }));
    assert!(!output.exists());
}

#[test]
fn selected_file_directory_collision_is_rejected() {
    let temp = TempDir::new().unwrap();
    let base = write_root(
        temp.path(),
        "base",
        104,
        &[("world", b"file", ProjectAssetKind::Data)],
    );
    let manifest_path = base.join(ASSET_MANIFEST_FILE);
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest.files.push(ProjectAssetFile {
        source_path: "base/world/a.json".to_owned(),
        path: "world/a.json".to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: 5,
        blake3: blake3::hash(b"child").to_hex().to_string(),
    });
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let overlay = write_root(
        temp.path(),
        "overlay",
        104,
        &[("audio/a.ogg", b"a", ProjectAssetKind::Audio)],
    );
    let output = temp.path().join("composed");
    let error = compose_asset_roots(
        &AssetCompositionOptions::new(base, overlay, &output).with_overlay_prefix("audio/"),
    )
    .unwrap_err();
    assert!(matches!(error, PipelineError::OutputCollision { .. }));
    assert!(!output.exists());
}

#[test]
fn forced_hard_link_failure_falls_back_to_verified_copy() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source.bin");
    let target = temp.path().join("target.bin");
    let bytes = b"verified fallback";
    fs::write(&source, bytes).unwrap();
    let entry = ProjectAssetFile {
        source_path: "source.bin".to_owned(),
        path: "target.bin".to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: bytes.len() as u64,
        blake3: blake3::hash(bytes).to_hex().to_string(),
    };

    let hard_linked = materialize_verified_asset_with(&source, &target, &entry, |_, _| {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "forced hard-link failure",
        ))
    })
    .unwrap();

    assert!(!hard_linked);
    assert_eq!(fs::read(target).unwrap(), bytes);
}

#[test]
fn report_serialization_reaches_a_true_byte_count_fixed_point() {
    let mut report = AssetCompositionReport {
        schema: ASSET_COMPOSITION_SCHEMA,
        base_manifest_blake3: "a".repeat(64),
        overlay_manifest_blake3: "b".repeat(64),
        overlay_prefixes: vec!["audio/".to_owned()],
        base_files_retained: 1,
        base_files_replaced: 2,
        overlay_files_selected: 3,
        resolved_path_conflicts: 1,
        output_files: 5,
        output_bytes: 0,
        hard_linked_files: 4,
        copied_files: 0,
    };
    let selected_bytes = 999;
    let bytes = serialize_stable_report(&mut report, selected_bytes).unwrap();
    let persisted: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(persisted, serde_json::to_value(&report).unwrap());
    assert_eq!(
        report.output_bytes,
        selected_bytes + u64::try_from(bytes.len()).unwrap()
    );
}
