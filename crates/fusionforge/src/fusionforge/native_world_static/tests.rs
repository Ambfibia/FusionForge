use super::*;

#[test]
fn parses_exact_tutorial_map_identity() {
    let path = Path::new(r"C:\build\Map_01_09.unity3d");
    let (map, tile, coordinate) = parse_map_bundle_identity(path).unwrap();
    assert_eq!(map, "Map_01_09");
    assert_eq!(tile, "tile_01_09");
    assert_eq!(coordinate, [1, 9]);
}

#[test]
fn rejects_case_and_traversal_in_publication_paths() {
    assert!(validate_relative_path("models/a.glb", Some("glb")).is_ok());
    assert!(validate_relative_path("../models/a.glb", Some("glb")).is_err());
    assert!(validate_relative_path("models\\a.glb", Some("glb")).is_err());
    assert!(validate_relative_path("models/a.GLB", Some("glb")).is_err());
    assert!(validate_relative_path("models/CON.glb", Some("glb")).is_err());
}

#[test]
fn optional_pointer_accepts_unity_null_pptr() {
    let env = UnityEnvironment::from_dir(Path::new(
        "this-path-intentionally-does-not-exist-for-null-pointer-test",
    ));
    let null = UnityValue::Pointer(super::super::unity::Pointer {
        source_asset: 0,
        file_id: 1,
        path_id: 0,
    });
    assert_eq!(
        object_key_from_optional_pointer(&env, Some(&null)).unwrap(),
        None
    );
}

#[test]
fn scripted_prefab_instantiation_materializes_inactive_asset_storage() {
    assert!(scripted_prefab_runtime_active(false));
    assert!(scripted_prefab_runtime_active(true));
}

#[test]
fn scripted_prefab_instantiation_replaces_only_the_root_transform() {
    let source = JsonTransform {
        translation: [-203.60000610351562, 39.0, -25.0],
        rotation: [0.0, 1.0, 0.0, -4.371138828673793e-8],
        scale: [2.0, 3.0, 4.0],
    };

    let root = scripted_prefab_runtime_transform(source, true);
    assert_eq!(root.translation, IDENTITY_TRANSFORM.translation);
    assert_eq!(root.rotation, IDENTITY_TRANSFORM.rotation);
    assert_eq!(root.scale, IDENTITY_TRANSFORM.scale);

    let child = scripted_prefab_runtime_transform(source, false);
    assert_eq!(child.translation, source.translation);
    assert_eq!(child.rotation, source.rotation);
    assert_eq!(child.scale, source.scale);
}

#[test]
fn final_report_persists_actual_output_counts_and_manifest_hash() {
    let scratch =
        ScratchDirectory::fresh(&std::env::temp_dir(), "ffone-static-report-test").unwrap();
    write_new_file(&scratch.path.join("payload.bin"), b"exact fixture payload").unwrap();
    let mut report = NativeStaticWorldExportReport {
        schema: EXPORT_SCHEMA.to_string(),
        status: "complete-hash-verified-no-preview-budget".to_string(),
        source_build: "fixture-build".to_string(),
        tile_id: "tile_01_01".to_string(),
        source_archive: "Map_01_01.unity3d".to_string(),
        source_archive_blake3: "source-hash".to_string(),
        output_root: slash_path(&scratch.path),
        scene_path: "scene.json".to_string(),
        scene_blake3: "scene-hash".to_string(),
        hierarchy_path: "hierarchy.json".to_string(),
        hierarchy_blake3: "hierarchy-hash".to_string(),
        material_path: "materials.json".to_string(),
        material_blake3: "material-hash".to_string(),
        catalog_path: "catalog.json".to_string(),
        catalog_blake3: "catalog-hash".to_string(),
        manifest_path: "export-manifest.json".to_string(),
        counts: ExportCounts::default(),
    };
    finalize_report_and_manifest(
        &scratch.path,
        "export-report.json",
        "export-manifest.json",
        "fixture-build",
        "tile_01_01",
        &mut report,
    )
    .unwrap();

    let report_bytes = fs::read(scratch.path.join("export-report.json")).unwrap();
    let disk_report: JsonValue = serde_json::from_slice(&report_bytes).unwrap();
    let files = collect_regular_files(&scratch.path).unwrap();
    let actual_bytes = files
        .iter()
        .map(|relative| {
            fs::metadata(join_relative(&scratch.path, relative).unwrap())
                .unwrap()
                .len()
        })
        .sum::<u64>();
    assert_eq!(report.counts.output_files, files.len());
    assert_eq!(report.counts.output_bytes, actual_bytes);
    assert_eq!(
        disk_report.pointer("/counts/outputFiles"),
        Some(&json!(files.len()))
    );
    assert_eq!(
        disk_report.pointer("/counts/outputBytes"),
        Some(&json!(actual_bytes))
    );

    let manifest_bytes = fs::read(scratch.path.join("export-manifest.json")).unwrap();
    let manifest: JsonValue = serde_json::from_slice(&manifest_bytes).unwrap();
    let report_entry = manifest
        .get("files")
        .and_then(JsonValue::as_array)
        .unwrap()
        .iter()
        .find(|entry| {
            entry.get("path").and_then(JsonValue::as_str) == Some("export-report.json")
        })
        .unwrap();
    let report_hash = blake3_hex(&report_bytes);
    assert_eq!(
        report_entry.get("blake3").and_then(JsonValue::as_str),
        Some(report_hash.as_str())
    );
}

#[test]
fn single_root_glb_validator_rejects_extra_nodes() {
    let document = json!({
        "asset": { "version": "2.0" },
        "scene": 0,
        "scenes": [{ "nodes": [0, 1] }],
        "nodes": [{ "name": "root", "mesh": 0 }, { "name": "extra" }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
        "buffers": [{ "byteLength": 0 }],
        "bufferViews": [],
        "accessors": []
    });
    let glb = encode_glb(document, Vec::new()).unwrap();
    assert!(validate_single_root_glb(&glb, "root", "fixture.glb").is_err());
}

#[test]
fn native_scene_enrichment_preserves_terrain_and_refuses_stale_models() {
    let terrain = json!({
        "descriptor": "world/tutorial/terrain/tiles/tile_01_01/terrain.json"
    });
    let mut scene = json!({
        "schema": SCENE_SCHEMA,
        "scope": "tutorial",
        "name": "tile_01_01",
        "tile": [1, 1],
        "root": IDENTITY_TRANSFORM,
        "models": [],
        "visuals": [],
        "colliders": [],
        "nativeTerrain": terrain,
    });
    enrich_native_scene(
        &mut scene,
        "tutorial",
        "tile_01_01",
        [1, 1],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
    )
    .unwrap();
    assert_eq!(scene.get("nativeTerrain"), Some(&terrain));
    scene["models"] = json!([{"id": "stale"}]);
    assert!(enrich_native_scene(
        &mut scene,
        "tutorial",
        "tile_01_01",
        [1, 1],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
    )
    .is_err());
}

#[test]
fn native_scene_enrichment_refuses_a_scope_mismatch() {
    let mut scene = json!({
        "schema": SCENE_SCHEMA,
        "scope": "worldMap",
        "name": "map_03_04",
        "tile": [3, 4],
        "root": IDENTITY_TRANSFORM,
        "models": [],
        "visuals": [],
        "colliders": [],
        "nativeTerrain": json!({ "descriptor": "world/maps/map_03_04/terrain/terrain.json" }),
    });
    assert!(enrich_native_scene(
        &mut scene,
        "tutorial",
        "map_03_04",
        [3, 4],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
    )
    .is_err());
    enrich_native_scene(
        &mut scene,
        "worldMap",
        "map_03_04",
        [3, 4],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
    )
    .unwrap();
}

#[test]
fn exact_previous_static_publication_can_be_replaced_but_tampering_cannot() {
    let root = std::env::temp_dir().join(format!(
        "ffone-static-world-replace-{}",
        unique_nonce().unwrap()
    ));
    let layout = TileLayout {
        scope: "worldMap".to_string(),
        tile_id: "map_03_04".to_string(),
        source_tile_relative: "world/maps/map_03_04".to_string(),
        model_relative_root: "models/world/maps/map_03_04".to_string(),
        static_relative_root: "world/maps/static/tiles/map_03_04".to_string(),
    };
    let scene = json!({
        "schema": SCENE_SCHEMA,
        "scope": "worldMap",
        "name": "map_03_04",
        "tile": [3, 4],
        "root": IDENTITY_TRANSFORM,
        "models": [{
            "id": "static-map_03_04-00000",
            "path": "models/world/maps/map_03_04/v-00000.glb"
        }],
        "visuals": [{"model": "static-map_03_04-00000"}],
        "colliders": [],
        "nativeTerrain": {"descriptor": "world/maps/map_03_04/terrain/terrain.json"},
        "coverage": "native-heightmap+exact-static-scene",
    });
    let scene_lf = pretty_json_bytes(&scene).unwrap();
    let scene_crlf = String::from_utf8(scene_lf.clone())
        .unwrap()
        .replace('\n', "\r\n")
        .into_bytes();
    let scene_path = root.join(&layout.source_tile_relative).join("scene.json");
    fs::create_dir_all(scene_path.parent().unwrap()).unwrap();
    fs::write(&scene_path, &scene_crlf).unwrap();
    let catalog_path = root.join(&layout.static_relative_root).join("catalog.json");
    fs::create_dir_all(catalog_path.parent().unwrap()).unwrap();
    let hierarchy_path = root
        .join(&layout.static_relative_root)
        .join("hierarchy.json");
    let material_path = root
        .join(&layout.static_relative_root)
        .join("materials.json");
    fs::write(&hierarchy_path, b"hierarchy").unwrap();
    fs::write(&material_path, b"materials").unwrap();
    fs::write(
        &catalog_path,
        serde_json::to_vec(&json!({
            "schema": CATALOG_SCHEMA,
            "sourceBuild": "primary",
            "sourceArchiveBlake3": "archive-hash",
            "tileId": "map_03_04",
            "scene": {
                "path": "world/maps/map_03_04/scene.json",
                "blake3": blake3_hex(&scene_lf),
            },
            "hierarchy": {
                "path": "world/maps/static/tiles/map_03_04/hierarchy.json",
                "blake3": blake3_hex(b"hierarchy"),
            },
            "materials": {
                "path": "world/maps/static/tiles/map_03_04/materials.json",
                "blake3": blake3_hex(b"materials"),
            },
            "counts": {
                "exportedModels": 1,
                "runtimeVisuals": 1,
                "runtimeColliders": 0,
            },
        }))
        .unwrap(),
    )
    .unwrap();

    assert!(
        verified_existing_static_publication(&root, &layout, "primary", "archive-hash")
            .unwrap()
    );
    let mut changed_scene = scene;
    changed_scene["nativeTerrain"]["blake3"] = json!("new-terrain-hash");
    fs::write(&scene_path, pretty_json_bytes(&changed_scene).unwrap()).unwrap();
    assert!(
        verified_existing_static_publication(&root, &layout, "primary", "archive-hash")
            .unwrap()
    );
    changed_scene["models"][0]["id"] = json!("hand-edited-model");
    fs::write(&scene_path, pretty_json_bytes(&changed_scene).unwrap()).unwrap();
    assert!(
        verified_existing_static_publication(&root, &layout, "primary", "archive-hash")
            .is_err()
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn tile_layout_uses_the_runtime_registry_scope() {
    let root = std::env::temp_dir().join(format!(
        "ffone-static-world-layout-{}",
        unique_nonce().unwrap()
    ));
    fs::create_dir_all(root.join("_runtime")).unwrap();
    fs::write(
        root.join("_runtime/world.json"),
        serde_json::to_vec(&json!({
            "schema": "ffone.runtime-world.v1",
            "entries": [
                {
                    "id": "tile_01_01",
                    "scope": "tutorial",
                    "tile": [1, 1],
                    "scene": { "path": "world/tutorial/terrain/tiles/tile_01_01/scene.json" }
                },
                {
                    "id": "map_03_04",
                    "scope": "worldMap",
                    "tile": [3, 4],
                    "scene": { "path": "world/maps/map_03_04/scene.json" }
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    let tutorial = resolve_tile_layout(&root, [1, 1]).unwrap();
    assert_eq!(tutorial.scope, "tutorial");
    assert_eq!(
        tutorial.source_tile_relative,
        "world/tutorial/terrain/tiles/tile_01_01"
    );
    assert_eq!(
        tutorial.model_relative_root,
        "models/world/tutorial/tile_01_01"
    );
    assert_eq!(
        tutorial.static_relative_root,
        "world/tutorial/static/tiles/tile_01_01"
    );

    let world_map = resolve_tile_layout(&root, [3, 4]).unwrap();
    assert_eq!(world_map.scope, "worldMap");
    assert_eq!(world_map.tile_id, "map_03_04");
    assert_eq!(world_map.source_tile_relative, "world/maps/map_03_04");
    assert_eq!(world_map.model_relative_root, "models/world/maps/map_03_04");
    assert_eq!(
        world_map.static_relative_root,
        "world/maps/static/tiles/map_03_04"
    );

    assert!(resolve_tile_layout(&root, [9, 9]).is_err());
    fs::remove_dir_all(&root).unwrap();
}

fn fixture_gltf_material(shader_name: &str, script: &str) -> JsonValue {
    let material = json!({
        "name": "fixture",
        "shaderName": shader_name,
        "savedProperties": {
            "colors": [{
                "name": "_Color",
                "value": { "r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0 }
            }],
            "floats": [],
            "textureEnvs": []
        },
        "shader": { "script": { "text": script } }
    });
    gltf_material_from_exact(
        &material,
        "fixture:1",
        &BTreeMap::new(),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut BTreeMap::new(),
    )
    .unwrap()
}

#[test]
fn exact_shader_commands_distinguish_opaque_cutout_blend_and_cull() {
    let opaque =
        fixture_gltf_material("normal", "Shader \"normal\" {\nBlend Off\nCull Back\n}");
    assert_eq!(opaque["alphaMode"], json!("OPAQUE"));
    assert_eq!(opaque.get("doubleSided"), None);

    let blended = fixture_gltf_material(
        "normal_blendSrcalphaInvsrcalpha_zwriteOff",
        "Shader \"normal_blendSrcalphaInvsrcalpha_zwriteOff\" {\nBlend SrcAlpha OneMinusSrcAlpha\n}",
    );
    assert_eq!(blended["alphaMode"], json!("BLEND"));

    let cutout = fixture_gltf_material(
        "normal_blendSrcalphaInvsrcalphaTest_cullOff",
        "Shader \"normal_blendSrcalphaInvsrcalphaTest_cullOff\" {\nAlphaTest Greater 0.35\nCull Off\n}",
    );
    assert_eq!(cutout["alphaMode"], json!("MASK"));
    assert_eq!(cutout["alphaCutoff"], json!(0.35));
    assert_eq!(cutout["doubleSided"], json!(true));
}

#[test]
fn glow_bump_slot_is_a_mask_transport_not_a_gltf_normal_map() {
    let material = json!({
        "name": "fixture glow",
        "shaderName": "normal_glow_blendSrcalphaInvsrcalpha",
        "savedProperties": {
            "colors": [{
                "name": "_Color",
                "value": { "r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0 }
            }],
            "floats": [],
            "textureEnvs": [
                { "slot": 0, "name": "_MainTex", "textureId": "fixture:base" },
                { "slot": 1, "name": "_BumpMap", "textureId": "fixture:glow" }
            ]
        },
        "shader": {
            "script": {
                "text": "Shader glow { Blend SrcAlpha OneMinusSrcAlpha }"
            }
        }
    });
    let texture_files = BTreeMap::from([
        (
            "fixture:base".to_string(),
            "models/world/maps/map_00_00/textures/base.png".to_string(),
        ),
        (
            "fixture:glow".to_string(),
            "models/world/maps/map_00_00/textures/glow.png".to_string(),
        ),
    ]);
    let mut images = Vec::new();
    let gltf = gltf_material_from_exact(
        &material,
        "fixture:material",
        &texture_files,
        &mut images,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut BTreeMap::new(),
    )
    .unwrap();

    assert!(gltf.get("normalTexture").is_none());
    assert_eq!(gltf["emissiveFactor"], json!([0.0, 0.0, 0.0]));
    assert_eq!(
        gltf["extras"]["runtimeTextureContract"]["schema"],
        json!("ffone.legacy-glow-texture-bindings.v1")
    );
    assert_eq!(
        gltf["extras"]["runtimeTextureContract"]["emissiveTexture"],
        json!("_BumpMap")
    );
    assert_eq!(images[1]["uri"], json!("textures/glow.png"));
}

#[test]
fn native_world_glb_retains_winding_already_aligned_with_authored_normals() {
    let mesh = MeshData {
        vertices: vec![(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)],
        normals: vec![(0.0, 0.0, 1.0); 3],
        uv1: vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
        triangles: vec![vec![0, 1, 2]],
    };
    let glb = build_single_root_glb(
        "fixture",
        &mesh,
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        PayloadKind::Visual,
        &[None],
        &BTreeMap::new(),
        &BTreeMap::new(),
        "fixture#1",
    )
    .unwrap();
    let (document, buffers, _) = gltf::import_slice(&glb.bytes).unwrap();
    let primitive = document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .next()
        .unwrap();
    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()].0));
    let indices = reader
        .read_indices()
        .unwrap()
        .into_u32()
        .collect::<Vec<_>>();
    let positions = reader.read_positions().unwrap().collect::<Vec<_>>();
    let normals = reader.read_normals().unwrap().collect::<Vec<_>>();

    assert_eq!(indices, vec![0, 1, 2]);
    let edge_a = [
        positions[1][0] - positions[0][0],
        positions[1][1] - positions[0][1],
        positions[1][2] - positions[0][2],
    ];
    let edge_b = [
        positions[2][0] - positions[0][0],
        positions[2][1] - positions[0][1],
        positions[2][2] - positions[0][2],
    ];
    let geometric = [
        edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1],
        edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2],
        edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0],
    ];
    let agreement = geometric[0] * normals[0][0]
        + geometric[1] * normals[0][1]
        + geometric[2] * normals[0][2];
    assert!(agreement > 0.999);
}

#[test]
fn native_world_glb_reverses_winding_that_opposes_authored_normals() {
    let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let outward = [[0.0, 0.0, -1.0]; 3];
    assert!(published_visual_winding_needs_reversal(
        &positions,
        &outward,
        &[0, 1, 2]
    ));
    assert!(!published_visual_winding_needs_reversal(
        &positions,
        &outward,
        &[0, 2, 1]
    ));
    assert!(published_visual_winding_needs_reversal(
        &positions,
        &[],
        &[0, 1, 2]
    ));
}
