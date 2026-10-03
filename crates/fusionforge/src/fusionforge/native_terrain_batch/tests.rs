use super::*;
use std::f64::consts::FRAC_PI_2;

#[test]
fn exact_dong_names_map_to_stable_semantic_routes() {
    assert_eq!(
        parse_dong_tile_name("DongResources_08_06.resourceFile"),
        Some("08_06".to_string())
    );
    assert_eq!(
        parse_dong_tile_name("DongResources_12_03.resourceFile"),
        Some("12_03".to_string())
    );
    for invalid in [
        "dongresources_08_06.resourceFile",
        "DongResources_8_06.resourceFile",
        "DongResources_08_006.resourceFile",
        "DongResources_08_06.unity3d",
        "DongResources_08_06_extra.resourceFile",
    ] {
        assert_eq!(parse_dong_tile_name(invalid), None, "{invalid}");
    }
}

#[test]
fn duplicate_dependency_aliases_are_deduplicated_by_normalized_cab_identity() {
    let dependencies = vec![
        "customassetbundle-ABCDEF".to_string(),
        "CAB-abcdef".to_string(),
        "customassetbundle-1234".to_string(),
    ];
    let normalized = normalized_dependency_requests(dependencies);
    assert_eq!(normalized.len(), 2);
    assert_eq!(
        normalized
            .iter()
            .map(|value| normalize_bundle_name(value))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["1234".to_string(), "abcdef".to_string()])
    );
}

#[test]
fn wrong_terrain_asset_name_never_matches_source_ownership() {
    let owned = BTreeSet::from(["CustomAssetBundle-source".to_string()]);
    assert!(owned.contains("CustomAssetBundle-source"));
    assert!(!owned.contains("CustomAssetBundle-dependency"));
}

#[test]
fn scene_dependency_alias_is_key_exact_not_name_or_path_id_only() {
    let nominal = ObjectKey {
        asset: 4,
        path_id: 139,
    };
    assert!(!scene_dependency_alias_required(nominal, nominal));
    assert!(scene_dependency_alias_required(
        nominal,
        ObjectKey {
            asset: 5,
            path_id: 139,
        }
    ));
    assert!(scene_dependency_alias_required(
        nominal,
        ObjectKey {
            asset: 4,
            path_id: 4,
        }
    ));
}

#[test]
fn map08_and_map12_scene_evidence_contract_is_not_baked_into_heightmap() {
    let evidence = [
        ("08_06", 334_i64, 15_281_i64, 15_279_i64, 742_i64),
        ("12_03", 433_i64, 13_865_i64, 13_863_i64, 2_890_i64),
    ];
    for (tile, terrain, collider, game_object, transform) in evidence {
        assert!(parse_dong_tile_name(&format!("DongResources_{tile}.resourceFile")).is_some());
        assert!(terrain > 0 && collider > 0 && game_object > 0 && transform > 0);
    }
    assert_eq!(
        "terrainDataLocalVertex -> this localTransform -> map tile root transform",
        "terrainDataLocalVertex -> this localTransform -> map tile root transform"
    );
}

#[test]
fn identical_tile_ids_in_world_and_tutorial_use_distinct_work_sessions() {
    let work_root = Path::new("work");
    let world = tile_session_path(work_root, TerrainScope::WorldMap, "00_00");
    let tutorial = tile_session_path(work_root, TerrainScope::Tutorial, "00_00");
    assert_eq!(world, PathBuf::from("work/worldMap/tile_00_00"));
    assert_eq!(tutorial, PathBuf::from("work/tutorial/tile_00_00"));
    assert_ne!(world, tutorial);
}

#[test]
fn quaternion_basis_conversion_matches_h_r_h_for_nontrivial_rotation() {
    let axis_length = (1.0_f64 + 4.0 + 9.0).sqrt();
    let half_angle = FRAC_PI_2 * 0.5;
    let sine = half_angle.sin();
    let unity_rotation = [
        sine / axis_length,
        2.0 * sine / axis_length,
        3.0 * sine / axis_length,
        half_angle.cos(),
    ];
    let native_rotation = unity_rotation_to_native(unity_rotation);
    let unity_vector = [2.5, -7.0, 4.25];
    let native_vector = unity_translation_to_native(unity_vector);
    let expected_native =
        unity_translation_to_native(rotate_vector(unity_rotation, unity_vector));
    let actual_native = rotate_vector(native_rotation, native_vector);
    assert_vec3_near(actual_native, expected_native, 1.0e-12);
}

#[test]
fn map08_parent_chain_and_owner_transform_are_composed_once() {
    let owner_unity_translation = [0.0, -300.0, 0.0];
    let root_unity_translation = [4096.0, 0.0, 3072.0];
    let owner_native = unity_translation_to_native(owner_unity_translation);
    let root_native = unity_translation_to_native(root_unity_translation);
    assert_eq!(owner_native, [0.0, -300.0, 0.0]);
    assert_eq!(root_native, [-4096.0, 0.0, 3072.0]);
    assert_eq!(add3(root_native, owner_native), [-4096.0, -300.0, 3072.0]);
    assert_eq!(
        "terrainDataLocalVertex -> owner localTransform -> each rootChain parent in owner-to-root order",
        "terrainDataLocalVertex -> owner localTransform -> each rootChain parent in owner-to-root order"
    );
}

#[test]
fn incomplete_detail_graph_roots_and_preloads_propagate_as_batch_blockers() {
    let document = json!({
        "detailAndTrees": {
            "prototypes": [{
                "prototypeMeshRoot": {
                    "status": "rootObjectExportedGraphClosureBlocked",
                    "resolvedAssetName": "CustomAssetBundle-mesh",
                    "resolvedPathId": 42,
                }
            }],
            "trees": {
                "prototypes": [{
                    "prefabRoot": {
                        "status": "rootObjectExportedGraphClosureBlocked",
                        "resolvedAssetName": "CustomAssetBundle-tree",
                        "resolvedPathId": 84,
                    }
                }]
            },
            "preloadTextureAtlasData": {
                "entries": [{
                    "status": "unmatchedPointerClosureBlocked",
                }]
            }
        }
    });
    let blockers = terrain_graph_closure_blockers_from_document(&document);
    assert_eq!(blockers.len(), 3);
    assert_eq!(blockers[0].0, "detailPrototypeGraphClosure");
    assert_eq!(blockers[1].0, "treePrefabGraphClosure");
    assert_eq!(blockers[2].0, "detailPreloadTextureClosure");
}

#[test]
fn publication_instance_ids_are_scope_qualified_for_strict_publisher_schema() {
    assert_eq!(
        publication_instance_id("worldMap", "02_05").unwrap(),
        "map_02_05"
    );
    assert_eq!(
        publication_instance_id("tutorial", "02_05").unwrap(),
        "tile_02_05"
    );
    assert!(publication_instance_id("unknown", "02_05").is_err());
}

#[test]
fn component_sidecar_names_are_semantic_and_case_fold_collisions_fail_closed() {
    assert_eq!(component_sidecar_stem("transform"), Some("transform"));
    assert_eq!(
        component_sidecar_stem("terrainRenderer"),
        Some("terrain_renderer")
    );
    assert_eq!(
        component_sidecar_stem("terrainCollider"),
        Some("terrain_collider")
    );
    assert_eq!(
        component_sidecar_stem("mapAttributeTable"),
        Some("map_attribute_table")
    );
    assert_eq!(
        component_sidecar_stem("terrainDetailManager"),
        Some("terrain_detail_manager")
    );
    assert_eq!(component_sidecar_stem("unresolved"), None);

    let mut names = BTreeMap::new();
    register_case_folded_sidecar_stem(&mut names, "terrain_renderer").unwrap();
    let error = register_case_folded_sidecar_stem(&mut names, "Terrain_Renderer").unwrap_err();
    assert!(error.contains("case-insensitive component sidecar collision"));
}

#[test]
fn ambience_colors_require_all_finite_rgba_channels() {
    let color = UnityValue::Object(BTreeMap::from([
        ("r".to_string(), UnityValue::Float(0.1)),
        ("g".to_string(), UnityValue::Float(0.2)),
        ("b".to_string(), UnityValue::Float(0.3)),
        ("a".to_string(), UnityValue::Float(1.0)),
    ]));
    assert_eq!(
        strict_color(Some(&color), "fogColor").unwrap(),
        [0.1, 0.2, 0.3, 1.0]
    );
    let non_finite = UnityValue::Object(BTreeMap::from([
        ("r".to_string(), UnityValue::Float(0.1)),
        ("g".to_string(), UnityValue::Float(f64::NAN)),
        ("b".to_string(), UnityValue::Float(0.3)),
        ("a".to_string(), UnityValue::Float(1.0)),
    ]));
    assert!(strict_color(Some(&non_finite), "fogColor")
        .unwrap_err()
        .contains("finite number"));
}

#[test]
fn runtime_ambience_and_detail_contracts_preserve_exact_legacy_formulas() {
    let world = runtime_ambience_contract(TerrainScope::WorldMap);
    let tutorial = runtime_ambience_contract(TerrainScope::Tutorial);
    assert_eq!(
        world
            .pointer("/sampling/localCoordinates/x")
            .and_then(JsonValue::as_str),
        Some("playerPosition.x / 512 - 0.5")
    );
    assert_eq!(
        tutorial
            .pointer("/defaultAmbienceApplication/tutorialBlendAppliesToThisScope")
            .and_then(JsonValue::as_bool),
        Some(true)
    );
    assert_eq!(
        tutorial
            .pointer("/defaultAmbienceApplication/fogDensity")
            .and_then(JsonValue::as_str),
        Some("sampledFogDepth * 0.005")
    );
    let detail = terrain_detail_runtime_contract();
    assert_eq!(
        detail
            .pointer("/bindings/detailObjectDistance")
            .and_then(JsonValue::as_str),
        Some("cnOption.graphicOption.GetDetailObjectCull()")
    );
}

#[test]
fn legacy_collider_blocker_is_reclassified_without_synthesizing_linkage() {
    let mut manifest = json!({
        "blocked": [{
            "scope": "worldMap",
            "tileId": "00_08",
            "sourcePath": "DongResources_00_08.resourceFile",
            "stage": "scene-link",
            "code": "terrainColliderLinkage",
            "message": "found 0"
        }]
    });
    replace_manifest_blockers(
        &mut manifest,
        vec![BatchBlocked {
            scope: Some("worldMap"),
            tile_id: Some("00_08".to_string()),
            source_path: Some("DongResources_00_08.resourceFile".to_string()),
            stage: "scene-link",
            code: "sourceResourceSceneTerrainMismatch",
            message: "source and scene TerrainData differ".to_string(),
        }],
    )
    .unwrap();
    let blocked = manifest["blocked"].as_array().unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(
        blocked[0]["code"].as_str(),
        Some("sourceResourceSceneTerrainMismatch")
    );
    assert!(blocked[0].get("syntheticLinkage").is_none());
}

#[test]
fn hardlink_clone_replacement_keeps_source_batch_byte_immutable() {
    let root = std::env::temp_dir().join(format!(
        "ffone-terrain-enrichment-hardlink-{}-{:x}",
        std::process::id(),
        unique_nonce().unwrap()
    ));
    let source = root.join("source");
    let destination = root.join("destination");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("document.json"), b"{\"version\":4}\n").unwrap();
    fs::write(source.join("nested/payload.bin"), b"immutable-payload").unwrap();
    let (files, bytes) = hard_link_tree_exact(&source, &destination).unwrap();
    assert_eq!(files, 2);
    assert_eq!(bytes, 14 + 17);
    replace_json_hardlink_safe(&destination.join("document.json"), &json!({"version": 5}))
        .unwrap();
    assert_eq!(
        fs::read(source.join("document.json")).unwrap(),
        b"{\"version\":4}\n"
    );
    assert_eq!(
        fs::read(destination.join("document.json")).unwrap(),
        b"{\n  \"version\": 5\n}\n"
    );
    assert_eq!(
        fs::read(destination.join("nested/payload.bin")).unwrap(),
        b"immutable-payload"
    );
    fs::remove_dir_all(root).unwrap();
}

fn rotate_vector(quaternion: [f64; 4], vector: [f64; 3]) -> [f64; 3] {
    let [x, y, z, w] = quaternion;
    let u = [x, y, z];
    let cross_uv = cross3(u, vector);
    let cross_u_cross_uv = cross3(u, cross_uv);
    [
        vector[0] + 2.0 * (w * cross_uv[0] + cross_u_cross_uv[0]),
        vector[1] + 2.0 * (w * cross_uv[1] + cross_u_cross_uv[1]),
        vector[2] + 2.0 * (w * cross_uv[2] + cross_u_cross_uv[2]),
    ]
}

fn cross3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn add3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn assert_vec3_near(actual: [f64; 3], expected: [f64; 3], epsilon: f64) {
    for index in 0..3 {
        assert!(
            (actual[index] - expected[index]).abs() <= epsilon,
            "component {index}: actual={} expected={} epsilon={epsilon}",
            actual[index],
            expected[index]
        );
    }
}
