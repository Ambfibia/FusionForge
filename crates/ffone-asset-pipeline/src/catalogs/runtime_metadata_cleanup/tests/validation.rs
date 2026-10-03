use super::*;

#[test]
fn production_orphan_world_proofs_require_exact_source_pack_identity() {
    let (_temp, project_root) = fixture();
    let registry = RuntimeWorldRegistry {
        schema: RUNTIME_WORLD_REGISTRY_SCHEMA.to_owned(),
        entries: Vec::new(),
    };
    add_fixture_asset(
        &project_root,
        RUNTIME_WORLD_REGISTRY_PATH,
        &serde_json::to_vec(&registry).unwrap(),
    );

    let asset_root = project_root.join("assets/game");
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    assert!(proven_orphan_world_payloads(&manifest).is_empty());

    let synthetic = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-20260613",
    ))
    .unwrap();
    assert!(synthetic.apply_ready);
    assert!(synthetic.orphan_world_payload_roots.is_empty());
    assert!(
        synthetic
            .already_archived_orphan_world_payload_roots
            .is_empty()
    );
    assert!(
        synthetic
            .blockers
            .iter()
            .all(|blocker| !blocker.contains("absent without its immutable archive"))
    );

    let mut schema_only = manifest.clone();
    schema_only.source_pack.schema = PROVEN_ORPHAN_WORLD_SOURCE_PACK_SCHEMA.to_owned();
    assert!(proven_orphan_world_payloads(&schema_only).is_empty());

    let mut hash_only = manifest.clone();
    hash_only.source_pack.manifest_blake3 = PROVEN_ORPHAN_WORLD_SOURCE_PACK_BLAKE3.to_owned();
    assert!(proven_orphan_world_payloads(&hash_only).is_empty());

    let mut production = manifest;
    production.source_pack.schema = PROVEN_ORPHAN_WORLD_SOURCE_PACK_SCHEMA.to_owned();
    production.source_pack.manifest_blake3 = PROVEN_ORPHAN_WORLD_SOURCE_PACK_BLAKE3.to_owned();
    assert_eq!(proven_orphan_world_payloads(&production).len(), 3);
    fs::remove_file(&manifest_path).unwrap();
    write_json_new(&manifest_path, &production).unwrap();

    let error = clean_runtime_metadata(&CleanRuntimeMetadataOptions::new(
        &project_root,
        "retrobution-20260613",
    ))
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("production conversion payload proof mismatch")
    );
}
