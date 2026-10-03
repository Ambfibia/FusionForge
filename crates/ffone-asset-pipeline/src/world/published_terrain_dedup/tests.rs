use super::*;

#[test]
fn package_variants_are_frequency_then_hash_deterministic() {
    let groups = BTreeMap::from([
        (("grass".to_owned(), "bbb".to_owned()), vec![(0, 0)]),
        (("grass".to_owned(), "ccc".to_owned()), vec![(0, 1), (1, 0)]),
        (("grass".to_owned(), "aaa".to_owned()), vec![(2, 0)]),
    ]);
    let routes = allocate_package_roots(&groups);
    assert_eq!(
        routes[&("grass".to_owned(), "ccc".to_owned())],
        "map/shared/terrain/details/grass"
    );
    assert_eq!(
        routes[&("grass".to_owned(), "aaa".to_owned())],
        "map/shared/terrain/details/grass_variant_02"
    );
    assert_eq!(
        routes[&("grass".to_owned(), "bbb".to_owned())],
        "map/shared/terrain/details/grass_variant_03"
    );
}

#[test]
fn exact_pair_requires_declared_and_actual_bytes() {
    let temporary = tempfile::tempdir().unwrap();
    fs::write(temporary.path().join("base.png"), b"same").unwrap();
    fs::write(temporary.path().join("mip.png"), b"same").unwrap();
    let digest = format!("blake3:{}", hash(b"same"));
    assert!(
        exact_hashed_files_match(
            temporary.path(),
            "base.png",
            &digest,
            "mip.png",
            &digest,
            "fixture",
        )
        .unwrap()
    );
    fs::write(temporary.path().join("mip.png"), b"evil").unwrap();
    assert!(
        exact_hashed_files_match(
            temporary.path(),
            "base.png",
            &digest,
            "mip.png",
            &digest,
            "fixture",
        )
        .is_err()
    );
}

#[test]
fn scene_link_uses_plain_artifact_hash() {
    let terrain = artifact("map/tiles/map_00_00/terrain/terrain.json", b"terrain");
    let scene = serde_json::json!({
        "nativeTerrain": {
            "path": terrain["path"],
            "blake3": terrain["blake3"],
        }
    });
    validate_scene_link(&scene, &terrain, "map_00_00").unwrap();
    assert!(
        !scene["nativeTerrain"]["blake3"]
            .as_str()
            .unwrap()
            .starts_with("blake3:")
    );
}

#[test]
fn primary_shift_uv_formula_is_normalized_for_bevy_flipped_v() {
    let source = serde_json::json!({
        "nativeGeometry": {
            "vertexShiftEncoding": {
                "sourceField": "m_Heightmap.m_Shifts",
                "uvFormula": "u += (positiveSourceX - negativeSourceX) / (2 * (width - 1)); v += (positiveSourceZ - negativeSourceZ) / (2 * (height - 1))"
            }
        }
    });
    let encoding = canonical_shift_encoding(&source, "map_test").unwrap();
    assert_eq!(
        encoding["uvFormula"],
        "u += (positiveSourceX - negativeSourceX) / (2 * (width - 1)); v -= (positiveSourceZ - negativeSourceZ) / (2 * (height - 1))"
    );
    assert_eq!(encoding["sourceField"], "m_Heightmap.m_Shifts");
}
