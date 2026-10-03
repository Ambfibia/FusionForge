use super::*;

pub(super) fn tiny_conversion_payload_fixture() -> (
    TempDir,
    PathBuf,
    ProjectAssetManifest,
    ProvenConversionPayloadProof,
    BTreeSet<String>,
) {
    let temp = TempDir::new().unwrap();
    let asset_root = temp.path().join("assets/game");
    fs::create_dir_all(&asset_root).unwrap();
    let mut files = Vec::new();
    let source_path = "world/maps/map_00_00/terrain/weights/mip_00.source.bin";
    add_asset(&asset_root, source_path, b"s", &mut files);
    for name in PROVEN_CONVERSION_RAW_NAMES {
        add_asset(
            &asset_root,
            &format!("world/maps/map_00_00/terrain/raw/{name}"),
            b"r",
            &mut files,
        );
    }
    let attributes_path = "world/maps/map_00_00/terrain/gameplay/attributes.raw.json";
    let attributes = br#"{"raw":true}"#;
    add_asset(&asset_root, attributes_path, attributes, &mut files);
    for excluded in [
        "world/maps/map_00_00/terrain/details/textures/tree/mips/mip_00.source.bin",
        "world/maps/map_00_00/terrain/details/patch-density.raw.bin",
        "world/maps/map_00_00/terrain/raw/terrain-collider.raw.bin",
    ] {
        add_asset(&asset_root, excluded, b"keep", &mut files);
    }
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
    let proof = ProvenConversionPayloadProof {
        source_files: 1,
        source_bytes: 1,
        raw_files: PROVEN_CONVERSION_RAW_NAMES.len() as u64,
        raw_files_per_name: 1,
        raw_bytes: PROVEN_CONVERSION_RAW_NAMES.len() as u64,
        attribute_json_files: 1,
        attribute_json_bytes: attributes.len() as u64,
        total_files: 2 + PROVEN_CONVERSION_RAW_NAMES.len() as u64,
        total_bytes: 1 + PROVEN_CONVERSION_RAW_NAMES.len() as u64 + attributes.len() as u64,
    };
    let cohort_paths = manifest
        .files
        .iter()
        .filter(|entry| classify_proven_conversion_payload(&entry.path).is_some())
        .map(|entry| entry.path.clone())
        .collect();
    (temp, asset_root, manifest, proof, cohort_paths)
}

#[test]
fn proven_conversion_payload_classifier_is_exact() {
    assert_eq!(
        classify_proven_conversion_payload(
            "world/maps/map_00_00/terrain/weights/mip_00.source.bin"
        ),
        Some(ProvenConversionPayloadKind::SourceEncoded)
    );
    assert_eq!(
        classify_proven_conversion_payload(
            "world/maps/map_00_00/terrain/details/textures/tree/mip_00.source.bin"
        ),
        None
    );
    for name in PROVEN_CONVERSION_RAW_NAMES {
        assert_eq!(
            classify_proven_conversion_payload(&format!(
                "world/maps/map_00_00/terrain/raw/{name}"
            )),
            Some(ProvenConversionPayloadKind::RawComponent)
        );
    }
    assert_eq!(
        classify_proven_conversion_payload(
            "world/maps/map_00_00/terrain/details/patch-density.raw.bin"
        ),
        None
    );
    assert_eq!(
        classify_proven_conversion_payload(
            "world/maps/map_00_00/terrain/raw/terrain-collider.raw.bin"
        ),
        None
    );
    assert_eq!(
        classify_proven_conversion_payload(
            "world/tutorial/terrain/tiles/tile_00_00/gameplay/attributes.raw.json"
        ),
        Some(ProvenConversionPayloadKind::GameplayAttributesJson)
    );
}

#[test]
fn proven_conversion_payload_plan_requires_exact_root_closure_and_is_idempotent() {
    let (_temp, asset_root, mut manifest, proof, cohort_paths) =
        tiny_conversion_payload_fixture();
    let initial = plan_proven_conversion_payloads(
        &asset_root,
        &manifest,
        &ArchiveHistory::default(),
        &proof,
        &BTreeSet::new(),
    )
    .unwrap();
    assert_eq!(initial.archived.len() as u64, proof.total_files);
    assert_eq!(
        initial
            .archived
            .iter()
            .map(|entry| entry.bytes)
            .sum::<u64>(),
        proof.total_bytes
    );
    let mut history = ArchiveHistory::default();
    history.add(&initial.archived);
    for path in &cohort_paths {
        fs::remove_file(asset_root.join(native_path(path).unwrap())).unwrap();
    }
    manifest
        .files
        .retain(|entry| !cohort_paths.contains(&entry.path));
    let idempotent = plan_proven_conversion_payloads(
        &asset_root,
        &manifest,
        &history,
        &proof,
        &BTreeSet::new(),
    )
    .unwrap();
    assert!(idempotent.archived.is_empty());
    assert_eq!(idempotent.already_archived_files, proof.total_files);

    let reappeared = "world/maps/map_00_00/terrain/weights/mip_00.source.bin";
    add_asset(&asset_root, reappeared, b"s", &mut manifest.files);
    let removal_only = plan_proven_conversion_payloads(
        &asset_root,
        &manifest,
        &history,
        &proof,
        &BTreeSet::new(),
    )
    .unwrap();
    assert_eq!(removal_only.archived.len(), 1);
    assert!(history.contains(&removal_only.archived[0]));

    let changed = b"changed";
    fs::write(asset_root.join(native_path(reappeared).unwrap()), changed).unwrap();
    let entry = manifest
        .files
        .iter_mut()
        .find(|entry| entry.path == reappeared)
        .unwrap();
    entry.bytes = changed.len() as u64;
    entry.blake3 = blake3::hash(changed).to_hex().to_string();
    let error = plan_proven_conversion_payloads(
        &asset_root,
        &manifest,
        &history,
        &proof,
        &BTreeSet::new(),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("identity outside its immutable archive")
    );
}

#[test]
fn proven_conversion_payload_references_cover_relative_direct_and_backslash_paths() {
    let (_temp, asset_root, mut manifest, proof, cohort_paths) =
        tiny_conversion_payload_fixture();
    let target = "world/maps/map_00_00/terrain/weights/mip_00.source.bin";
    let document = "world/maps/map_00_00/terrain/terrain.json";
    for reference in [
        "weights/mip_00.source.bin",
        target,
        "assets\\game\\world\\maps\\map_00_00\\terrain\\weights\\mip_00.source.bin",
    ] {
        let value = serde_json::json!({"path": reference});
        assert_eq!(
            find_archived_asset_reference(document, &value, &cohort_paths)
                .map(|(_, target)| target),
            Some(target.to_owned())
        );
    }

    let descriptor = br#"{"path":"weights/mip_00.source.bin"}"#;
    add_asset(&asset_root, document, descriptor, &mut manifest.files);
    let error = plan_proven_conversion_payloads(
        &asset_root,
        &manifest,
        &ArchiveHistory::default(),
        &proof,
        &BTreeSet::new(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("retains relative reference"));

    fs::write(
        asset_root.join(native_path(document).unwrap()),
        br#"{"path":"safe.bin"}"#,
    )
    .unwrap();
    let error = plan_proven_conversion_payloads(
        &asset_root,
        &manifest,
        &ArchiveHistory::default(),
        &proof,
        &BTreeSet::new(),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("active JSON manifest identity mismatch")
    );
}

#[test]
fn production_conversion_payload_proof_requires_exact_source_pack_identity() {
    let (_temp, _asset_root, manifest, _proof, _paths) = tiny_conversion_payload_fixture();
    assert!(proven_conversion_payload_proof(&manifest).is_none());
    let mut schema_only = manifest.clone();
    schema_only.source_pack.schema = PROVEN_ORPHAN_WORLD_SOURCE_PACK_SCHEMA.to_owned();
    assert!(proven_conversion_payload_proof(&schema_only).is_none());
    let mut hash_only = manifest.clone();
    hash_only.source_pack.manifest_blake3 = PROVEN_ORPHAN_WORLD_SOURCE_PACK_BLAKE3.to_owned();
    assert!(proven_conversion_payload_proof(&hash_only).is_none());
    let mut production = manifest;
    production.source_pack.schema = PROVEN_ORPHAN_WORLD_SOURCE_PACK_SCHEMA.to_owned();
    production.source_pack.manifest_blake3 = PROVEN_ORPHAN_WORLD_SOURCE_PACK_BLAKE3.to_owned();
    let proof = proven_conversion_payload_proof(&production).unwrap();
    assert_eq!(proof.total_files, 20_056);
    assert_eq!(proof.total_bytes, 343_585_139);
    let content_index = proven_offline_content_index_proof(&production).unwrap();
    assert_eq!(content_index.path, PROVEN_OFFLINE_CONTENT_INDEX_PATH);
    assert_eq!(
        content_index.source_path,
        PROVEN_OFFLINE_CONTENT_INDEX_SOURCE_PATH
    );
    assert_eq!(content_index.bytes, PROVEN_OFFLINE_CONTENT_INDEX_BYTES);
    assert_eq!(content_index.blake3, PROVEN_OFFLINE_CONTENT_INDEX_BLAKE3);
}
