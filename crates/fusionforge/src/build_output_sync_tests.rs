use super::*;

#[test]
fn forced_sync_restores_source_bytes_and_rollback_removes_generated_layout_files() {
    let temp = native_build_temp_dir("forced_build_output_sync").expect("temporary directory");
    let source = temp.root.join("source");
    let out_dir = temp.root.join("output");
    fs::create_dir_all(&source).expect("source directory");
    fs::create_dir_all(&out_dir).expect("output directory");

    let relative = "DongResources_05_04.resourceFile";
    fs::write(source.join(relative), b"pristine source").expect("source bundle");
    fs::write(out_dir.join(relative), b"pristine source").expect("initial output bundle");
    let snapshot = build_source_snapshot(&source).expect("source snapshot");
    write_build_source_snapshot(&out_dir, &snapshot).expect("persist source snapshot");

    // Simulate a process stopping after rewriting an exact-name source bundle but
    // before patched_files was persisted.
    fs::write(out_dir.join(relative), b"partial layout").expect("partial layout bundle");
    let generated = out_dir.join("NPC_Pack_01.resourceFile");
    fs::write(&generated, b"generated layout").expect("generated layout bundle");

    restore_pristine_build_output(&source, &out_dir).expect("restore pristine output");

    assert_eq!(
        fs::read(out_dir.join(relative)).expect("restored bundle"),
        b"pristine source"
    );
    assert!(!generated.exists());
}

#[test]
fn forced_initial_sync_keeps_explicitly_preserved_generated_files() {
    let temp = native_build_temp_dir("forced_preserving_build_output_sync")
        .expect("temporary directory");
    let source = temp.root.join("source");
    let out_dir = temp.root.join("output");
    fs::create_dir_all(&source).expect("source directory");
    fs::create_dir_all(&out_dir).expect("output directory");

    let relative = "DongResources_05_04.resourceFile";
    fs::write(source.join(relative), b"pristine source").expect("source bundle");
    fs::write(out_dir.join(relative), b"partial layout").expect("partial output bundle");
    let generated_name = "Character_Custom.resourceFile";
    fs::write(out_dir.join(generated_name), b"preserved NPC bundle")
        .expect("preserved NPC bundle");

    sync_build_output_from_source_preserving(
        &source,
        &out_dir,
        &BTreeSet::from([generated_name.to_string()]),
        true,
    )
    .expect("forced initial sync");

    assert_eq!(
        fs::read(out_dir.join(relative)).expect("restored bundle"),
        b"pristine source"
    );
    assert_eq!(
        fs::read(out_dir.join(generated_name)).expect("preserved NPC bundle"),
        b"preserved NPC bundle"
    );
}

#[test]
fn stable_layout_input_digest_uses_exact_content_but_not_file_time() {
    let temp =
        native_build_temp_dir("stable_layout_input_digest").expect("temporary directory");
    let input = temp.root.join("source.resourceFile");
    let config = json!({"BundleLayout": {"Enabled": true}});
    let digest = |bytes: &[u8]| {
        fs::write(&input, bytes).expect("write cache input");
        let mut files = LegacyLayoutInputFiles::default();
        files
            .add_file("source/source.resourceFile", &input)
            .expect("register cache input");
        files.digest(&config).expect("hash cache input")
    };

    let first = digest(b"first");
    let same_content_after_rewrite = digest(b"first");
    let same_size_different_content = digest(b"other");
    assert_eq!(first, same_content_after_rewrite);
    assert_ne!(first, same_size_different_content);
}

#[test]
fn external_import_fingerprint_detects_same_size_donor_changes() {
    let temp = native_build_temp_dir("external_import_content_fingerprint")
        .expect("temporary directory");
    let source = temp.root.join("donor.resourceFile");
    fs::write(&source, b"first").expect("write donor");
    let spec = ExternalResourceImportSpec {
        source: source.clone(),
        bundle_name: "External_Test.resourceFile".to_string(),
        routes: BTreeSet::from(["vo/test.wav".to_string()]),
        sections: vec!["m_FreeZone".to_string()],
        project_bundle: temp.root.join("External_Test.resourceFile"),
    };
    let first = external_resource_import_fingerprint(&spec).expect("first fingerprint");
    fs::write(&source, b"other").expect("rewrite donor with same size");
    let second = external_resource_import_fingerprint(&spec).expect("second fingerprint");
    assert_ne!(first, second);
}
