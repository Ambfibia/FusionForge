use super::*;
use tempfile::tempdir;

fn fixture_bundle(levels: &[(&str, &[(&str, &[u8])])]) -> (tempfile::TempDir, AssetBundle) {
    let root = tempdir().expect("tempdir");
    for (level, files) in levels {
        let directory = root.path().join(level);
        fs::create_dir_all(&directory).expect("level directory");
        for (name, bytes) in *files {
            fs::write(directory.join(name), bytes).expect("fixture entry");
        }
    }
    let bundle = AssetBundle::from_directory(root.path().to_str().expect("unicode path"))
        .expect("bundle");
    (root, bundle)
}

fn managed_proof() -> ManagedFormatProof {
    ManagedFormatProof {
        pe_kind: "PE32",
        is_dll: true,
        clr_header_major: 2,
        clr_header_minor: 5,
        clr_metadata_bytes: 3,
        clr_metadata_sha256: sha256_hex(b"clr"),
    }
}

#[test]
fn exact_entry_round_trip_has_versioned_hash_envelope_without_machine_paths() {
    let payload = b"MZ exact managed payload";
    let (_root, bundle) =
        fixture_bundle(&[("level0", &[("Assembly - CSharp.dll", payload.as_slice())])]);
    let selected = select_exact_entry(bundle.iter_files(), "Assembly - CSharp.dll", None)
        .expect("exact entry");
    assert_eq!(selected.level, 0);
    assert_eq!(selected.bytes, payload);

    let raw = b"raw UnityWeb authority";
    let evidence = build_evidence(
        "primary".to_string(),
        "main.unity3d".to_string(),
        raw,
        selected.level,
        selected.name,
        selected.bytes,
        managed_proof(),
        PayloadMaterialization {
            mode: "embedded-base64",
            portable_relative_path: None,
            base64: Some(BASE64_STANDARD.encode(selected.bytes)),
        },
    );
    assert_eq!(evidence.schema, SCHEMA);
    assert_eq!(evidence.schema_version, 1);
    assert_eq!(evidence.source.raw_container.bytes, raw.len() as u64);
    assert_eq!(evidence.source.raw_container.sha256, sha256_hex(raw));
    assert_eq!(evidence.payload.bytes, payload.len() as u64);
    assert_eq!(evidence.payload.sha256, sha256_hex(payload));
    assert_eq!(evidence.selection.route, "level0/Assembly - CSharp.dll");
    assert_eq!(
        BASE64_STANDARD
            .decode(
                evidence
                    .payload
                    .materialization
                    .base64
                    .as_deref()
                    .expect("embedded payload")
            )
            .expect("base64"),
        payload
    );

    let json = serde_json::to_string(&evidence).expect("json");
    assert!(json.contains("\"schema\":\"fftools.managed-assembly-evidence.v1\""));
    assert!(!json.contains("sourceRoot"));
    assert!(!json.contains("C:\\\\"));
}

#[test]
fn duplicate_exact_entry_across_levels_is_ambiguous_without_level() {
    let (_root, bundle) = fixture_bundle(&[
        ("level0", &[("Assembly - CSharp.dll", b"MZ first")]),
        ("level1", &[("Assembly - CSharp.dll", b"MZ second")]),
    ]);
    let error = select_exact_entry(bundle.iter_files(), "Assembly - CSharp.dll", None)
        .err()
        .expect("ambiguous");
    assert!(error.contains("ambiguous"));
    assert!(error.contains("level0/Assembly - CSharp.dll"));
    assert!(error.contains("level1/Assembly - CSharp.dll"));

    let selected = select_exact_entry(bundle.iter_files(), "Assembly - CSharp.dll", Some(1))
        .expect("level-scoped exact entry");
    assert_eq!(selected.bytes, b"MZ second");
}

#[test]
fn missing_exact_entry_fails_closed() {
    let (_root, bundle) = fixture_bundle(&[("level0", &[("System.dll", b"MZ system")])]);
    let error = select_exact_entry(bundle.iter_files(), "Assembly - CSharp.dll", None)
        .err()
        .expect("missing");
    assert!(error.contains("was not found exactly"));
}

#[test]
fn source_relative_container_rejects_parent_and_absolute_escape() {
    let root = tempdir().expect("root");
    let outside = tempdir().expect("outside");
    fs::write(root.path().join("main.unity3d"), b"bundle").expect("source");
    fs::write(outside.path().join("outside.unity3d"), b"bundle").expect("outside");

    let parent_error = resolve_source_relative_container(root.path(), "../outside.unity3d")
        .err()
        .expect("parent traversal");
    assert!(parent_error.contains("parent traversal"));

    let absolute_error = resolve_source_relative_container(
        root.path(),
        outside
            .path()
            .join("outside.unity3d")
            .to_str()
            .expect("unicode"),
    )
    .err()
    .expect("absolute escape");
    assert!(absolute_error.contains("relative path"));

    let (_, relative) =
        resolve_source_relative_container(root.path(), "./main.unity3d").expect("valid");
    assert_eq!(relative, "main.unity3d");
}

#[test]
fn payload_locator_is_portable_and_rejects_machine_or_parent_paths() {
    let output = portable_output("work\\cases\\Assembly.dll").expect("portable");
    assert_eq!(output.locator, "work/cases/Assembly.dll");
    assert!(portable_output("..\\Assembly.dll").is_err());
    assert!(portable_output("D:\\case\\Assembly.dll").is_err());
}

#[test]
fn non_managed_payload_is_rejected_even_when_named_dll() {
    let error = validate_managed_assembly("Assembly - CSharp.dll", b"MZ not a PE")
        .err()
        .expect("invalid managed assembly");
    assert!(error.contains("not a readable PE DLL"));
}

#[test]
fn exact_raw_bytes_can_be_parsed_without_an_extraction_directory() {
    let (root, bundle) =
        fixture_bundle(&[("level0", &[("Assembly - CSharp.dll", b"MZ exact bytes")])]);
    let packed = root.path().join("main.unity3d");
    bundle
        .to_file(packed.to_str().expect("unicode"), 6, None)
        .expect("pack");
    let raw = fs::read(&packed).expect("raw bundle");
    let (_header, reparsed) = AssetBundle::from_bytes(&raw).expect("in-memory parse");
    let selected = select_exact_entry(reparsed.iter_files(), "Assembly - CSharp.dll", Some(0))
        .expect("exact entry");
    assert_eq!(selected.bytes, b"MZ exact bytes");
}
