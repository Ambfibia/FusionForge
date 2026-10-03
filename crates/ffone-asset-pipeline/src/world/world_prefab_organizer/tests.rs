use super::*;

fn detail_texture_fixture(root: &Path, true_name: &str, resolved_path_id: i64) -> JsonValue {
    let package_relative = format!("details/textures/{true_name}");
    let package = root.join(&package_relative);
    fs::create_dir_all(package.join("mips")).unwrap();
    let base = b"exact base and mip zero";
    let mip_one = b"smaller mip";
    let source_zero = b"source zero";
    let source_one = b"source one";
    fs::write(package.join("texture.png"), base).unwrap();
    fs::write(package.join("mips/mip_00.png"), base).unwrap();
    fs::write(package.join("mips/mip_01.png"), mip_one).unwrap();
    fs::write(package.join("mips/mip_00.source.bin"), source_zero).unwrap();
    fs::write(package.join("mips/mip_01.source.bin"), source_one).unwrap();
    let source_hash = format!(
        "blake3:{}",
        hash_bytes(format!("source-{resolved_path_id}").as_bytes())
    );
    let base_hash = format!("blake3:{}", hash_bytes(base));
    let mip_one_hash = format!("blake3:{}", hash_bytes(mip_one));
    let document = serde_json::json!({
        "schema": "ffone.native-terrain-detail-texture.v1",
        "trueTextureName": true_name,
        "source": {
            "resolvedAssetName": "fixture-asset",
            "resolvedPathId": resolved_path_id,
            "serializedObjectRawBlake3": source_hash,
        },
        "path": format!("{package_relative}/texture.png"),
        "pngBlake3": base_hash,
        "mips": [
            {
                "level": 0,
                "path": format!("{package_relative}/mips/mip_00.png"),
                "pngBlake3": base_hash,
                "sourceEncoded": {
                    "path": format!("{package_relative}/mips/mip_00.source.bin"),
                    "blake3": format!("blake3:{}", hash_bytes(source_zero)),
                },
            },
            {
                "level": 1,
                "path": format!("{package_relative}/mips/mip_01.png"),
                "pngBlake3": mip_one_hash,
                "sourceEncoded": {
                    "path": format!("{package_relative}/mips/mip_01.source.bin"),
                    "blake3": format!("blake3:{}", hash_bytes(source_one)),
                },
            },
        ],
    });
    let document_bytes = pretty_json(&document).unwrap();
    fs::write(package.join("texture.json"), &document_bytes).unwrap();
    serde_json::json!({
        "documentBlake3": format!("blake3:{}", hash_bytes(&document_bytes)),
        "documentPath": format!("{package_relative}/texture.json"),
        "path": format!("{package_relative}/texture.png"),
        "resolvedAssetName": "fixture-asset",
        "resolvedPathId": resolved_path_id,
        "serializedObjectRawBlake3": source_hash,
        "trueTextureName": true_name,
    })
}

fn detail_terrain(record: JsonValue) -> JsonValue {
    serde_json::json!({
        "detailAndTrees": {
            "textures": [record.clone()],
            "prototypes": [{"prototypeTexture": record}],
        }
    })
}

#[test]
fn taxonomy_preserves_observed_prefix_and_category() {
    let aliases = BTreeSet::from([
        "dt_sdde_darkengine_factory_01".to_owned(),
        "factory wall".to_owned(),
    ]);
    let taxonomy = taxonomy(&aliases);
    assert_eq!(taxonomy.category, "structures");
    assert_eq!(taxonomy.prefix, "DT");
    assert_eq!(taxonomy.family, "SDDE");
}

#[test]
fn unknown_names_fail_closed_into_unclassified() {
    let aliases = BTreeSet::from(["DW_ZZQ_opaque_thing_07".to_owned()]);
    let taxonomy = taxonomy(&aliases);
    assert_eq!(taxonomy.category, "unclassified");
    assert_eq!(taxonomy.prefix, "DW");
    assert_eq!(taxonomy.family, "ZZQ");
}

#[test]
fn character_payload_names_are_excluded_but_named_buildings_are_not() {
    assert!(aliases_look_character_like(&BTreeSet::from([
        "mob_roadgorlem-sub-link_a".to_owned()
    ])));
    assert!(aliases_look_character_like(&BTreeSet::from([
        "!!maincharacter-01 - default-m_body".to_owned()
    ])));
    assert!(!aliases_look_character_like(&BTreeSet::from([
        "npc_building_front_wall".to_owned()
    ])));
}

#[test]
fn disabled_renderer_model_is_not_organized_as_runtime_geometry() {
    let mut payload = HierarchyPayload {
        id: "visual:disabled".to_owned(),
        name: "collisionBuilding".to_owned(),
        kind: "visual".to_owned(),
        hierarchy_node_id: "node:collisionBuilding".to_owned(),
        source_mesh: WorldPrefabSourceIdentity {
            asset: "primary-resource".to_owned(),
            path_id: 2286,
            object_type: "Mesh".to_owned(),
        },
        material_ids: Vec::new(),
        world_matrix: [[0.0; 4]; 4],
        runtime_published: false,
        model_path: Some("models/world/maps/map_04_03/v-00010-2287.glb".to_owned()),
        model_blake3: Some("unused-export-proof".to_owned()),
        is_trigger: false,
    };

    assert!(!payload_has_runtime_model(&payload));
    payload.runtime_published = true;
    assert!(payload_has_runtime_model(&payload));
    payload.model_path = None;
    assert!(!payload_has_runtime_model(&payload));
}

#[test]
fn only_primary_collision_alpha_visual_identities_are_suppressed() {
    let payload = |kind: &str, mesh: i64, material: Option<i64>| HierarchyPayload {
        id: format!("{kind}:{mesh}"),
        name: "collision helper".to_owned(),
        kind: kind.to_owned(),
        hierarchy_node_id: "BuildPlayer-Map_08_06#9126".to_owned(),
        source_mesh: WorldPrefabSourceIdentity {
            asset: LEGACY_COLLISION_HELPER_ASSET.to_owned(),
            path_id: mesh,
            object_type: "Mesh".to_owned(),
        },
        material_ids: material
            .map(|material| vec![Some(format!("{LEGACY_COLLISION_HELPER_ASSET}:{material}"))])
            .unwrap_or_default(),
        world_matrix: identity_matrix(),
        runtime_published: true,
        model_path: Some("models/world/maps/map_08_06/helper.glb".to_owned()),
        model_blake3: Some("proof".to_owned()),
        is_trigger: false,
    };

    assert!(is_legacy_collision_helper_payload(&payload(
        "visual",
        701,
        Some(1_717)
    )));
    assert!(is_legacy_collision_helper_payload(&payload(
        "visual",
        502,
        Some(1_892)
    )));
    assert!(!is_legacy_collision_helper_payload(&payload(
        "collider", 699, None
    )));
    assert!(!is_legacy_collision_helper_payload(&payload(
        "collider", 503, None
    )));
    assert!(!is_legacy_collision_helper_payload(&payload(
        "visual",
        701,
        Some(999)
    )));
    assert!(is_legacy_additive_black_visual(&payload(
        "visual",
        439,
        Some(1_847)
    )));
    assert!(!is_legacy_additive_black_visual(&payload(
        "collider",
        439,
        Some(1_847)
    )));
    assert!(!is_legacy_additive_black_visual(&payload(
        "visual",
        439,
        Some(999)
    )));
    assert!(!is_legacy_additive_black_visual(&payload(
        "visual",
        440,
        Some(1_847)
    )));
}

#[test]
fn relative_texture_path_stays_inside_map_root() {
    assert_eq!(
        relative_path(
            Path::new("objects/nature/WD/WOOD/tree"),
            Path::new("shared/textures/hash.png")
        )
        .unwrap(),
        "../../../../../shared/textures/hash.png"
    );
}

#[test]
fn inverse_bake_restores_local_position_and_normal() {
    let world = [
        [2.0, 0.0, 0.0, 10.0],
        [0.0, 3.0, 0.0, -4.0],
        [0.0, 0.0, 4.0, 7.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let matrix = dmat4(world);
    let local = DVec3::new(1.0, 2.0, 3.0);
    let baked = matrix.transform_point3(local);
    let restored = matrix.inverse().transform_point3(baked);
    assert!((restored - local).length() < 1.0e-12);

    let local_normal = DVec3::new(1.0, 1.0, 0.0).normalize();
    let baked_normal = DMat3::from_mat4(matrix).inverse().transpose() * local_normal;
    let restored_normal = (DMat3::from_mat4(matrix).transpose() * baked_normal).normalize();
    assert!((restored_normal - local_normal).length() < 1.0e-12);
}

#[test]
fn exact_detail_closures_publish_once_and_provenance_variants_remain_distinct() {
    let temporary = tempfile::tempdir().unwrap();
    let stage = temporary.path().join("stage");
    fs::create_dir(&stage).unwrap();
    let first_root = temporary.path().join("first");
    let second_root = temporary.path().join("second");
    let variant_root = temporary.path().join("variant");
    for root in [&first_root, &second_root, &variant_root] {
        fs::create_dir(root).unwrap();
    }
    let first_record = detail_texture_fixture(&first_root, "Test Grass", 17);
    let second_record = detail_texture_fixture(&second_root, "Test Grass", 17);
    let variant_record = detail_texture_fixture(&variant_root, "Test Grass", 18);
    let first_root = canonical_directory(&first_root, "fixture").unwrap();
    let second_root = canonical_directory(&second_root, "fixture").unwrap();
    let variant_root = canonical_directory(&variant_root, "fixture").unwrap();
    let mut shared = SharedTerrainFiles::default();

    let mut first = detail_terrain(first_record);
    let first_skipped =
        publish_shared_terrain_detail_textures(&first_root, &stage, &mut first, &mut shared)
            .unwrap();
    let first_texture = &first["detailAndTrees"]["textures"][0];
    assert_eq!(
        first_texture["documentPath"],
        "map/shared/terrain/details/test_grass/texture.json"
    );
    assert_eq!(
        first_texture["path"],
        "map/shared/terrain/details/test_grass/mips/mip_00.png"
    );
    assert_eq!(
        first["detailAndTrees"]["prototypes"][0]["prototypeTexture"],
        *first_texture
    );
    assert!(first_skipped.contains("details/textures/Test Grass/texture.png"));
    assert!(
        !stage
            .join("shared/terrain/details/test_grass/texture.png")
            .exists()
    );

    let mut second = detail_terrain(second_record);
    publish_shared_terrain_detail_textures(&second_root, &stage, &mut second, &mut shared)
        .unwrap();
    assert_eq!(
        second["detailAndTrees"]["textures"][0]["documentPath"],
        first_texture["documentPath"]
    );
    assert_eq!(shared.detail_package_routes.len(), 1);

    let mut variant = detail_terrain(variant_record);
    publish_shared_terrain_detail_textures(&variant_root, &stage, &mut variant, &mut shared)
        .unwrap();
    assert_eq!(
        variant["detailAndTrees"]["textures"][0]["documentPath"],
        "map/shared/terrain/details/test_grass_variant_02/texture.json"
    );
    assert_eq!(shared.detail_package_routes.len(), 2);
}

#[test]
fn exact_weight_mip_zero_collapses_only_after_byte_and_hash_verification() {
    let temporary = tempfile::tempdir().unwrap();
    fs::create_dir_all(temporary.path().join("weights/mips")).unwrap();
    fs::write(temporary.path().join("weights/base.png"), b"same").unwrap();
    fs::write(temporary.path().join("weights/mips/mip_00.png"), b"same").unwrap();
    let digest = format!("blake3:{}", hash_bytes(b"same"));
    let mut texture = serde_json::json!({
        "path": "weights/base.png",
        "pngBlake3": digest,
        "mips": [{
            "level": 0,
            "path": "weights/mips/mip_00.png",
            "pngBlake3": digest,
        }],
    });
    let mut skipped = BTreeSet::new();
    collapse_exact_terrain_base_mip_zero(
        temporary.path(),
        &mut texture,
        &mut skipped,
        "fixture weight",
    )
    .unwrap();
    assert_eq!(texture["mips"][0]["path"], "weights/base.png");
    assert_eq!(
        skipped,
        BTreeSet::from(["weights/mips/mip_00.png".to_owned()])
    );

    fs::write(
        temporary.path().join("weights/mips/mip_00.png"),
        b"different",
    )
    .unwrap();
    let mut corrupted = serde_json::json!({
        "path": "weights/base.png",
        "pngBlake3": digest,
        "mips": [{
            "level": 0,
            "path": "weights/mips/mip_00.png",
            "pngBlake3": digest,
        }],
    });
    assert!(
        collapse_exact_terrain_base_mip_zero(
            temporary.path(),
            &mut corrupted,
            &mut BTreeSet::new(),
            "fixture weight",
        )
        .is_err()
    );
}
