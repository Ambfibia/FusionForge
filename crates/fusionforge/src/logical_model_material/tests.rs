use super::*;

#[test]
fn missing_external_texture_never_resolves_by_a_matching_local_path_id() {
    use crate::fusionforge::{AssetRef, TypeMetadata};
    let asset = Asset {
        name: "owner.asset".into(),
        data: Vec::new(),
        format: 7,
        metadata_size: 0,
        file_size: 0,
        data_offset: 0,
        long_object_ids: false,
        tree: TypeMetadata::default(),
        objects: BTreeMap::from([(
            42,
            ObjectInfo {
                path_id: 42,
                data_offset: 0,
                size: 0,
                type_id: 28,
                class_id: 28,
            },
        )]),
        asset_refs: vec![
            AssetRef {
                asset_path: String::new(),
                guid: [0; 16],
                type_id: 0,
                file_path: "owner.asset".into(),
            },
            AssetRef {
                asset_path: String::new(),
                guid: [0; 16],
                type_id: 0,
                file_path: "missing.asset".into(),
            },
        ],
    };
    let env = UnityEnvironment::from_assets(vec![asset]);
    let pointer = Pointer {
        source_asset: 0,
        file_id: 1,
        path_id: 42,
    };
    assert!(strict_non_null_pointer(&env, &pointer)
        .unwrap_err()
        .contains("not loaded"));
    let bad_external = Pointer {
        file_id: 500,
        ..pointer
    };
    assert!(strict_non_null_pointer(&env, &bad_external).is_err());
    let local = Pointer {
        file_id: 0,
        ..pointer
    };
    assert_eq!(
        strict_non_null_pointer(&env, &local).unwrap(),
        ObjectKey {
            asset: 0,
            path_id: 42
        }
    );
}

#[test]
fn exact_texture_identity_uses_class_id_for_legacy_base_texture_type_tree() {
    let asset = Asset {
        name: "legacy-icons.asset".into(), data: Vec::new(), format: 7,
        metadata_size: 0, file_size: 0, data_offset: 0,
        long_object_ids: false, tree: Default::default(),
        objects: BTreeMap::new(), asset_refs: Vec::new(),
    };
    let info = ObjectInfo { path_id: 1, data_offset: 0, size: 0,
        type_id: 27, class_id: 28 };
    assert_eq!(asset.object_type_name(&info), "Texture");
    assert_eq!(exact_object_type_name(&asset, &info), "Texture2D");
    let base = ObjectInfo { class_id: 27, ..info };
    assert_eq!(exact_object_type_name(&asset, &base), "Texture");
}

#[test]
fn exact_dxt1_mip_decode_isolates_level_from_original_mip_flags() {
    let texture = UnityValue::Object(BTreeMap::from([
        ("m_Width".to_string(), UnityValue::Int(256)),
        ("m_Height".to_string(), UnityValue::Int(256)),
        ("m_TextureFormat".to_string(), UnityValue::Int(10)),
        ("m_MipMap".to_string(), UnityValue::Bool(true)),
        ("m_MipCount".to_string(), UnityValue::Int(9)),
    ]));
    let env = UnityEnvironment::from_assets(Vec::new());
    // One opaque red DXT1 block. The source Texture2D claims a complete
    // mip chain, but this helper deliberately receives one exact level.
    let encoded = [0x00, 0xf8, 0x00, 0xf8, 0, 0, 0, 0];

    let rgba =
        decode_exact_mip_level(&env, &texture, 10, 4, 4, &encoded).expect("exact DXT1 mip");
    assert_eq!(rgba.len(), 4 * 4 * 4);
    assert!(rgba.chunks_exact(4).all(|pixel| pixel == [255, 0, 0, 255]));
}

#[test]
fn lossless_json_does_not_truncate_arrays_or_bytes() {
    let value = UnityValue::Array(vec![
        UnityValue::Int(0),
        UnityValue::Int(1),
        UnityValue::Int(2),
        UnityValue::Int(3),
        UnityValue::Int(4),
        UnityValue::Int(5),
        UnityValue::Int(6),
        UnityValue::Int(7),
        UnityValue::Int(8),
        UnityValue::Bytes(vec![0, 1, 2, 3]),
    ]);
    let json = unity_to_lossless_json(&value);
    assert_eq!(json.as_array().map(Vec::len), Some(10));
    assert_eq!(json.pointer("/9/$bytes/byteLength"), Some(&json!(4)));
    assert_eq!(json.pointer("/9/$bytes/data"), Some(&json!("AAECAw==")));
}

#[test]
fn exact_png_is_full_resolution_flip_only_and_keeps_transparent_rgb() {
    let rgba = vec![
        1, 2, 3, 0, // source top row; transparent RGB must not be repaired
        10, 20, 30, 255, // source bottom row
    ];
    let png = exact_png_bytes(1, 2, &rgba, true).expect("encode PNG");
    let decoded = image::load_from_memory(&png)
        .expect("decode PNG")
        .into_rgba8();
    assert_eq!(decoded.dimensions(), (1, 2));
    assert_eq!(decoded.get_pixel(0, 0).0, [10, 20, 30, 255]);
    assert_eq!(decoded.get_pixel(0, 1).0, [1, 2, 3, 0]);
}

#[test]
fn shader_name_and_state_hints_are_source_evidence() {
    let script = r#"Shader "FusionFall/Glass" {
        Tags { "Queue"="Transparent" }
        Pass {
            Blend SrcAlpha OneMinusSrcAlpha
            Cull Off
            ZWrite Off
        }
    }"#;
    assert_eq!(
        shader_declared_name(script).as_deref(),
        Some("FusionFall/Glass")
    );
    let evidence = shader_render_state_evidence(script);
    assert_eq!(evidence["blend"].as_array().map(Vec::len), Some(1));
    assert_eq!(evidence["cull"].as_array().map(Vec::len), Some(1));
    assert_eq!(evidence["zWriteAndZTest"].as_array().map(Vec::len), Some(1));
    assert_eq!(evidence["passes"].as_array().map(Vec::len), Some(1));
}

#[test]
fn null_pointer_json_is_preserved_as_an_explicit_slot() {
    let pointer = Pointer {
        source_asset: 3,
        file_id: 9,
        path_id: 0,
    };
    let json = pointer_json(&pointer);
    assert_eq!(json["fileId"], json!(9));
    assert_eq!(json["pathId"], json!(0));
    assert_eq!(json["isNull"], json!(true));
}

#[test]
fn mip_count_is_derived_from_exact_source_layout() {
    assert_eq!(derive_mip_count(256, 256, 10, 32_768, false), Ok(1));
    assert_eq!(derive_mip_count(256, 256, 10, 43_704, false), Ok(9));
    assert_eq!(derive_mip_count(32, 32, 12, 1_392, true), Ok(6));
    assert!(derive_mip_count(32, 32, 12, 1_391, true).is_err());
}

#[test]
fn dxt5_mip_layout_preserves_every_source_slice_in_order() {
    let levels = exact_mip_layout(32, 32, 12, 6, 1_392).expect("exact DXT5 layout");
    let actual = levels
        .iter()
        .map(|level| {
            (
                level.level,
                level.width,
                level.height,
                level.source_byte_offset,
                level.source_byte_length,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            (0, 32, 32, 0, 1_024),
            (1, 16, 16, 1_024, 256),
            (2, 8, 8, 1_280, 64),
            (3, 4, 4, 1_344, 16),
            (4, 2, 2, 1_360, 16),
            (5, 1, 1, 1_376, 16),
        ]
    );
    assert!(exact_mip_layout(32, 32, 12, 6, 1_391).is_err());
    assert!(exact_mip_layout(32, 32, 12, 6, 1_393).is_err());
}

#[test]
fn raw_argb32_and_dxt1_mip_lengths_are_not_approximated() {
    let argb = exact_mip_layout(4, 2, 5, 3, 44).expect("ARGB32 chain");
    assert_eq!(argb[0].source_byte_length, 32);
    assert_eq!(argb[1].source_byte_length, 8);
    assert_eq!(argb[2].source_byte_length, 4);

    let dxt1 = exact_mip_layout(4, 4, 10, 3, 24).expect("DXT1 chain");
    assert_eq!(
        dxt1.iter()
            .map(|level| level.source_byte_length)
            .collect::<Vec<_>>(),
        vec![8, 8, 8]
    );
    assert!(exact_mip_layout(4, 4, 28, 3, 24).is_err());
}

#[test]
fn texture_sampler_uses_nested_serialized_settings() {
    let texture = UnityValue::Object(BTreeMap::from([(
        "m_TextureSettings".to_string(),
        UnityValue::Object(BTreeMap::from([(
            "m_FilterMode".to_string(),
            UnityValue::Int(2),
        )])),
    )]));
    let filter = exact_texture_setting(&texture, "m_FilterMode", 1);
    assert_eq!(filter["value"], json!(2));
    assert_eq!(filter["source"], json!("m_TextureSettings.m_FilterMode"));
    assert_eq!(filter["serialized"], json!(true));
    let wrap = exact_texture_setting(&texture, "m_WrapMode", 0);
    assert_eq!(wrap["value"], json!(0));
    assert_eq!(wrap["source"], json!("unity-texture-serialized-default"));
    assert_eq!(wrap["serialized"], json!(false));
}

#[test]
fn exact_mesh_geometry_matches_extract_mesh_without_preview_rounding() {
    fn vec3(x: f64, y: f64, z: f64) -> UnityValue {
        UnityValue::Object(BTreeMap::from([
            ("x".to_string(), UnityValue::Float(x)),
            ("y".to_string(), UnityValue::Float(y)),
            ("z".to_string(), UnityValue::Float(z)),
        ]))
    }
    fn vec2(x: f64, y: f64) -> UnityValue {
        UnityValue::Object(BTreeMap::from([
            ("x".to_string(), UnityValue::Float(x)),
            ("y".to_string(), UnityValue::Float(y)),
        ]))
    }

    let mesh = UnityValue::Object(BTreeMap::from([
        (
            "m_Name".to_string(),
            UnityValue::String("Exact".to_string()),
        ),
        ("m_MeshCompression".to_string(), UnityValue::Int(0)),
        (
            "m_Vertices".to_string(),
            UnityValue::Array(vec![
                vec3(0.123_456_789_012_345, 0.0, 0.0),
                vec3(0.0, 0.987_654_321_098_765, 0.0),
                vec3(0.0, 0.0, 0.111_111_111_111_111),
            ]),
        ),
        (
            "m_Normals".to_string(),
            UnityValue::Array(vec![
                vec3(0.123_456_789, 0.987_654_321, 0.111_111_111),
                vec3(0.0, 1.0, 0.0),
                vec3(0.0, 0.0, 1.0),
            ]),
        ),
        (
            "m_UV".to_string(),
            UnityValue::Array(vec![
                vec2(0.123_456_789, 0.987_654_321),
                vec2(0.5, 0.25),
                vec2(1.0, 0.0),
            ]),
        ),
        (
            "m_IndexBuffer".to_string(),
            UnityValue::Bytes(vec![0, 0, 1, 0, 2, 0]),
        ),
        (
            "m_SubMeshes".to_string(),
            UnityValue::Array(vec![UnityValue::Object(BTreeMap::from([
                ("firstByte".to_string(), UnityValue::Int(0)),
                ("indexCount".to_string(), UnityValue::Int(3)),
                ("isTriStrip".to_string(), UnityValue::Int(0)),
            ]))]),
        ),
    ]));
    let env = UnityEnvironment::from_assets(Vec::new());
    let extracted = crate::fusionforge::extract_mesh(&mesh).expect("extract mesh");
    let exact =
        crate::fusionforge::mesh_to_exact_source(&env, &mesh, "fixture", 1, None, &[], "mesh")
            .expect("exact mesh");
    let preview = crate::fusionforge::mesh_to_preview(
        &env,
        &mesh,
        "fixture",
        1,
        None,
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        &[],
        "mesh",
    )
    .expect("preview mesh");

    let expected_positions = extracted
        .vertices
        .iter()
        .flat_map(|value| [value.0, value.1, value.2])
        .collect::<Vec<_>>();
    let expected_normals = extracted
        .normals
        .iter()
        .flat_map(|value| [value.0, value.1, value.2])
        .collect::<Vec<_>>();
    let expected_uvs = extracted
        .uv1
        .iter()
        .flat_map(|value| [value.0, value.1])
        .collect::<Vec<_>>();
    let json_f64 = |value: &JsonValue, field: &str| {
        value[field]
            .as_array()
            .expect("geometry array")
            .iter()
            .map(|value| value.as_f64().expect("f64"))
            .collect::<Vec<_>>()
    };
    assert_eq!(json_f64(&exact, "positions"), expected_positions);
    assert_eq!(json_f64(&exact, "normals"), expected_normals);
    assert_eq!(json_f64(&exact, "uvs"), expected_uvs);
    assert_ne!(preview["positions"], exact["positions"]);
    assert_ne!(preview["normals"], exact["normals"]);
    assert_ne!(preview["uvs"], exact["uvs"]);
}
