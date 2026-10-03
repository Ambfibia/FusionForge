use super::*;

#[test]
fn previous_layout_migration_accepts_only_the_pinned_revision() {
    assert!(is_supported_layout_revision(LAYOUT_BYTES));

    let current = std::str::from_utf8(LAYOUT_BYTES).unwrap();
    let previous = current
        .replace(
            "work/hud-original-top-left.png",
            "work/retrobution-reference-20260717/after-enter-click.png",
        )
        .replace(
            "  \"referenceScope\": \"player_status crop only; remaining layers require state-specific primary client-surface captures\",\n",
            "",
        );
    assert_eq!(previous.as_bytes(), PREVIOUS_LAYOUT_BYTES);
    assert_eq!(PREVIOUS_LAYOUT_BYTES.len(), 2_929);
    assert_eq!(
        blake3::hash(PREVIOUS_LAYOUT_BYTES).to_hex().as_str(),
        PREVIOUS_LAYOUT_BLAKE3
    );
    assert!(is_supported_layout_revision(previous.as_bytes()));

    let mut drifted = previous.into_bytes();
    drifted[0] ^= 0x01;
    assert!(!is_supported_layout_revision(&drifted));
}

#[test]
fn existing_layout_verification_rejects_manifest_and_byte_drift() {
    let (project, asset_root) = native_ui_work_copy();
    let options = NativeGameplayUiInstallOptions {
        asset_root: asset_root.clone(),
        source_build: "retrobution-20260613-layout-verification".to_owned(),
    };
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let layout_path = asset_root
        .join("ui")
        .join("gameplay")
        .join("layout")
        .join("gameplay_hud.json");

    let mut previous_manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let previous_entry = previous_manifest
        .files
        .iter_mut()
        .find(|entry| entry.path == GAMEPLAY_UI_LAYOUT)
        .unwrap();
    previous_entry.source_path =
        "crates/ffone-asset-pipeline/fixtures/ui/gameplay_hud.layout.json".to_owned();
    previous_entry.kind = ProjectAssetKind::Data;
    previous_entry.bytes = PREVIOUS_LAYOUT_BYTES.len() as u64;
    previous_entry.blake3 = PREVIOUS_LAYOUT_BLAKE3.to_owned();
    fs::write(&layout_path, PREVIOUS_LAYOUT_BYTES).unwrap();
    replace_manifest(&manifest_path, &previous_manifest).unwrap();

    install_native_gameplay_ui(&options).unwrap();
    assert_eq!(fs::read(&layout_path).unwrap(), LAYOUT_BYTES);

    let pristine_layout = fs::read(&layout_path).unwrap();
    let pristine_manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let mut drifted_layout = pristine_layout.clone();
    drifted_layout[0] ^= 0x01;

    fs::write(&layout_path, &drifted_layout).unwrap();
    assert!(install_native_gameplay_ui(&options).is_err());
    fs::write(&layout_path, &pristine_layout).unwrap();

    let mut matching_drifted_manifest = pristine_manifest.clone();
    let matching_drifted_entry = matching_drifted_manifest
        .files
        .iter_mut()
        .find(|entry| entry.path == GAMEPLAY_UI_LAYOUT)
        .unwrap();
    matching_drifted_entry.bytes = drifted_layout.len() as u64;
    matching_drifted_entry.blake3 = blake3::hash(&drifted_layout).to_hex().to_string();
    fs::write(&layout_path, &drifted_layout).unwrap();
    replace_manifest(&manifest_path, &matching_drifted_manifest).unwrap();
    assert!(install_native_gameplay_ui(&options).is_err());
    fs::write(&layout_path, &pristine_layout).unwrap();
    replace_manifest(&manifest_path, &pristine_manifest).unwrap();

    let mut wrong_kind_manifest = pristine_manifest.clone();
    wrong_kind_manifest
        .files
        .iter_mut()
        .find(|entry| entry.path == GAMEPLAY_UI_LAYOUT)
        .unwrap()
        .kind = ProjectAssetKind::Texture;
    replace_manifest(&manifest_path, &wrong_kind_manifest).unwrap();
    assert!(install_native_gameplay_ui(&options).is_err());

    fs::remove_dir_all(project).unwrap();
}
