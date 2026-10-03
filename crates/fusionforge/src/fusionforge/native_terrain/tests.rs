use super::*;

#[test]
fn unity_x_major_heights_are_transposed_to_canonical_z_rows() {
    let source = vec![10, 11, 12, 20, 21, 22];
    let canonical = canonicalize_unity_heights(2, 3, &source).unwrap();
    assert_eq!(canonical, vec![10, 20, 11, 21, 12, 22]);
}

#[test]
fn gray16_png_preserves_exact_u16_samples() {
    let samples = vec![0, 1, 32_767, 65_535];
    let png = encode_gray16_png(2, 2, &samples).unwrap();
    let decoded = image::load_from_memory_with_format(&png, ImageFormat::Png)
        .unwrap()
        .into_luma16();
    assert_eq!(decoded.dimensions(), (2, 2));
    assert_eq!(decoded.into_raw(), samples);
}

#[test]
fn layer_channel_mapping_uses_four_linear_channels_per_weight_map() {
    let mappings = (0..11).map(weight_channel).collect::<Vec<_>>();
    assert_eq!(
        mappings,
        vec![
            WeightChannel {
                map_index: 0,
                channel_index: 0,
                channel: "r"
            },
            WeightChannel {
                map_index: 0,
                channel_index: 1,
                channel: "g"
            },
            WeightChannel {
                map_index: 0,
                channel_index: 2,
                channel: "b"
            },
            WeightChannel {
                map_index: 0,
                channel_index: 3,
                channel: "a"
            },
            WeightChannel {
                map_index: 1,
                channel_index: 0,
                channel: "r"
            },
            WeightChannel {
                map_index: 1,
                channel_index: 1,
                channel: "g"
            },
            WeightChannel {
                map_index: 1,
                channel_index: 2,
                channel: "b"
            },
            WeightChannel {
                map_index: 1,
                channel_index: 3,
                channel: "a"
            },
            WeightChannel {
                map_index: 2,
                channel_index: 0,
                channel: "r"
            },
            WeightChannel {
                map_index: 2,
                channel_index: 1,
                channel: "g"
            },
            WeightChannel {
                map_index: 2,
                channel_index: 2,
                channel: "b"
            },
        ]
    );
}

#[test]
fn canonical_weight_orientation_is_an_exact_vertical_flip() {
    let top_red = [255, 0, 0, 255];
    let bottom_blue = [0, 0, 255, 255];
    let source = [top_red, bottom_blue].concat();
    let canonical = canonicalize_rgba_flip_y(1, 2, &source).unwrap();
    assert_eq!(canonical, [bottom_blue, top_red].concat());
    let restored = canonicalize_rgba_flip_y(1, 2, &canonical).unwrap();
    assert_eq!(restored, source);
}

#[test]
fn semantic_names_fail_closed_on_windows_aliases_and_path_syntax() {
    for invalid in ["..", "a/b", "a\\b", "CON", "trailing."] {
        assert!(semantic_component(invalid).is_err(), "{invalid:?}");
    }
    assert_eq!(
        semantic_component("Grass_Base").unwrap(),
        "Grass_Base".to_string()
    );
}

#[test]
fn raw_height_hash_uses_explicit_little_endian_u16_bytes() {
    let bytes = u16_le_bytes(&[0x1234, 0xabcd]);
    assert_eq!(bytes, vec![0x34, 0x12, 0xcd, 0xab]);
}

#[test]
fn unity_heightmap_vertex_shifts_are_preserved_in_source_order() {
    let shifts = UnityValue::Array(vec![
        UnityValue::Object(BTreeMap::from([
            ("flags".to_string(), UnityValue::Int(4)),
            ("x".to_string(), UnityValue::Int(60)),
            ("y".to_string(), UnityValue::Int(18)),
        ])),
        UnityValue::Object(BTreeMap::from([
            ("flags".to_string(), UnityValue::Int(9)),
            ("x".to_string(), UnityValue::Int(29)),
            ("y".to_string(), UnityValue::Int(5)),
        ])),
    ]);
    assert_eq!(
        exact_vertex_shifts(Some(&shifts), 129, 129, "Terrain.m_Shifts").unwrap(),
        [
            TerrainVertexShift {
                flags: 4,
                column: 60,
                row: 18,
            },
            TerrainVertexShift {
                flags: 9,
                column: 29,
                row: 5,
            },
        ]
    );
}

#[test]
fn unity_heightmap_vertex_shifts_fail_closed_on_invalid_masks_and_coordinates() {
    assert!(exact_vertex_shifts(None, 129, 129, "Terrain.m_Shifts").is_err());
    assert!(
        exact_vertex_shifts(Some(&UnityValue::Int(0)), 129, 129, "Terrain.m_Shifts").is_err()
    );
    for (flags, x, y) in [(0, 1, 1), (3, 1, 1), (12, 1, 1), (16, 1, 1), (4, 129, 1)] {
        let shifts = UnityValue::Array(vec![UnityValue::Object(BTreeMap::from([
            ("flags".to_string(), UnityValue::Int(flags)),
            ("x".to_string(), UnityValue::Int(x)),
            ("y".to_string(), UnityValue::Int(y)),
        ]))]);
        assert!(
            exact_vertex_shifts(Some(&shifts), 129, 129, "Terrain.m_Shifts").is_err(),
            "flags={flags} coordinate=({x},{y})"
        );
    }
}

#[test]
fn absent_splat_mode_uses_proven_unity_schema_default_zero() {
    let splat = BTreeMap::from([
        ("texture".to_string(), UnityValue::Int(0)),
        ("tileSize".to_string(), UnityValue::Int(0)),
    ]);
    let (mode, source, evidence) =
        splat_mode_with_provenance(&splat, "TerrainData_02_05", 0).unwrap();
    assert_eq!(mode, 0);
    assert_eq!(source, "implicitSchemaDefault");
    assert_eq!(evidence["fieldPresence"], "absent");
    assert_eq!(evidence["resolvedValue"], 0);
    assert_eq!(evidence["enum"]["Splat"], 0);
}

#[test]
fn present_non_integral_splat_mode_is_not_defaulted() {
    let splat = BTreeMap::from([("mode".to_string(), UnityValue::Float(0.0))]);
    let error = splat_mode_with_provenance(&splat, "Terrain", 2).unwrap_err();
    assert!(error.contains("present but not an exact integer"));
}

#[test]
fn splat_alpha_sampler_contract_preserves_exact_unity_settings() {
    let texture = UnityValue::Object(BTreeMap::from([
        ("m_TextureFormat".to_string(), UnityValue::Int(5)),
        ("m_MipMap".to_string(), UnityValue::Bool(true)),
        (
            "m_TextureSettings".to_string(),
            UnityValue::Object(BTreeMap::from([
                ("m_Aniso".to_string(), UnityValue::Int(1)),
                ("m_FilterMode".to_string(), UnityValue::Int(1)),
                ("m_MipBias".to_string(), UnityValue::Float(0.0)),
                ("m_WrapMode".to_string(), UnityValue::Int(1)),
            ])),
        ),
    ]));
    let contract = texture_import_contract(&texture, "linear", 9).unwrap();
    assert_eq!(contract["textureFormat"], 5);
    assert_eq!(contract["mipMap"], true);
    assert_eq!(contract["mipCount"], 9);
    assert_eq!(contract["filterMode"]["unityName"], "Bilinear");
    assert_eq!(contract["wrapMode"]["unityName"], "Clamp");
    assert_eq!(contract["anisoLevel"], 1);
    assert_eq!(contract["mipBias"], 0.0);
    assert_eq!(contract["usageColorSpace"], "linear");
}

#[test]
fn native_geometry_covers_negative_x_and_has_positive_y_ccw_winding() {
    let spacing_x = 4.0;
    let spacing_z = 4.0;
    let origin = native_position(0, 0, 8_192, spacing_x, 600.0, spacing_z);
    let far_x = native_position(128, 0, 8_192, spacing_x, 600.0, spacing_z);
    let far_z = native_position(0, 128, 8_192, spacing_x, 600.0, spacing_z);
    assert_eq!(origin[0], 0.0);
    assert_eq!(far_x[0], -512.0);
    assert_eq!(far_z[2], 512.0);

    let edge_column = subtract3(far_x, origin);
    let edge_row = subtract3(far_z, origin);
    let normal = cross3(edge_column, edge_row);
    assert!(
        normal[1] > 0.0,
        "{NATIVE_CELL_TRIANGLE_ORDER} must face +nativeY, got {normal:?}"
    );
    assert_eq!(
        NATIVE_VERTEX_FORMULA,
        "[-column * sampleSpacingX, rawU16 / 32767 * heightScale, row * sampleSpacingZ]"
    );
}

#[test]
fn scene_owner_translation_is_applied_after_unmodified_terrain_data_height() {
    let local = native_position(0, 0, 8_192, 4.0, 600.0, 4.0);
    let scene_owner_translation_y = -300.0;
    let placed_height = scene_owner_translation_y + local[1];
    assert_eq!(
        placed_height,
        -300.0 + f64::from(8_192_u16) / 32_767.0 * 600.0
    );
    assert_eq!(
        SCENE_HEIGHT_FORMULA,
        "sceneOwnerLocalTranslationY + rawU16 / 32767 * heightScale"
    );
    assert!(!SCENE_HEIGHT_FORMULA.contains("-300"));
}

#[test]
fn ordered_preload_duplicates_are_covered_only_by_exact_prototype_pointers() {
    let shared = Pointer {
        source_asset: 7,
        file_id: 2,
        path_id: 71,
    };
    let unmatched = Pointer {
        source_asset: 7,
        file_id: 2,
        path_id: 72,
    };
    let value = UnityValue::Array(vec![
        UnityValue::Pointer(shared.clone()),
        UnityValue::Pointer(shared.clone()),
        UnityValue::Pointer(unmatched),
    ]);
    let (contract, blocked_count) = preload_texture_atlas_contract(Some(&value), &[shared]);
    assert_eq!(blocked_count, 1);
    assert_eq!(contract["orderedDuplicatesPreserved"], true);
    assert_eq!(contract["entries"].as_array().unwrap().len(), 3);
    assert_eq!(
        contract["entries"][0]["status"],
        "coveredByPrototypeTextureExport"
    );
    assert_eq!(
        contract["entries"][1]["status"],
        "coveredByPrototypeTextureExport"
    );
    assert_eq!(
        contract["entries"][2]["status"],
        "unmatchedPointerClosureBlocked"
    );
}

#[test]
fn detail_texture_semantic_names_fail_closed_on_case_folded_object_collision() {
    let mut names = BTreeMap::new();
    register_unique_detail_texture_name(
        &mut names,
        "Grass_Base",
        "Grass_Base",
        ObjectKey {
            asset: 1,
            path_id: 71,
        },
        "blake3:first",
    )
    .unwrap();
    let error = register_unique_detail_texture_name(
        &mut names,
        "grass_base",
        "grass_base",
        ObjectKey {
            asset: 1,
            path_id: 72,
        },
        "blake3:second",
    )
    .unwrap_err();
    assert!(error.contains("case-insensitive detail texture semantic-path collision"));
}

#[cfg(windows)]
#[test]
fn atomic_publish_retries_only_bounded_transient_windows_lock_errors() {
    use std::cell::{Cell, RefCell};

    let calls = Cell::new(0_usize);
    let delayed_attempts = RefCell::new(Vec::new());
    retry_transient_rename(
        || {
            let next = calls.get() + 1;
            calls.set(next);
            match next {
                1 => Err(std::io::Error::from_raw_os_error(5)),
                2 => Err(std::io::Error::from_raw_os_error(32)),
                _ => Ok(()),
            }
        },
        |attempt| delayed_attempts.borrow_mut().push(attempt),
    )
    .unwrap();
    assert_eq!(calls.get(), 3);
    assert_eq!(&*delayed_attempts.borrow(), &[1, 2]);

    let fatal_calls = Cell::new(0_usize);
    let error = retry_transient_rename(
        || {
            fatal_calls.set(fatal_calls.get() + 1);
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "not transient",
            ))
        },
        |_| panic!("non-transient rename must not back off"),
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(fatal_calls.get(), 1);
}

#[test]
fn caller_owned_staging_removes_partial_tile_until_explicit_completion() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = std::env::temp_dir().join(format!(
        "ffone-native-terrain-caller-staging-{}-{nonce:x}",
        std::process::id()
    ));
    assert!(!output.exists());

    fs::create_dir(&output).unwrap();
    {
        let staging = StagingDirectory::new(output.clone());
        fs::write(staging.path().join("partial.bin"), b"partial").unwrap();
    }
    assert!(
        !output.exists(),
        "failed caller-owned tile must leave no partial terrain directory"
    );

    fs::create_dir(&output).unwrap();
    {
        let mut staging = StagingDirectory::new(output.clone());
        fs::write(staging.path().join("complete.bin"), b"complete").unwrap();
        staging.keep();
    }
    assert!(output.join("complete.bin").is_file());
    fs::remove_dir_all(&output).unwrap();
}

fn native_position(
    column: usize,
    row: usize,
    raw: u16,
    spacing_x: f64,
    height_scale: f64,
    spacing_z: f64,
) -> [f64; 3] {
    [
        -(column as f64) * spacing_x,
        f64::from(raw) / f64::from(HEIGHT_NORMALIZATION_DENOMINATOR) * height_scale,
        row as f64 * spacing_z,
    ]
}

fn subtract3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn cross3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}
