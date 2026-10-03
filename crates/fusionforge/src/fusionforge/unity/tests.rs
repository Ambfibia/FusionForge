use super::*;

#[test]
fn zero_path_id_is_null_even_with_stale_file_id() {
    assert!(Pointer {
        source_asset: 0,
        file_id: 21,
        path_id: 0,
    }
    .is_null());
    assert!(!Pointer {
        source_asset: 0,
        file_id: 0,
        path_id: 37,
    }
    .is_null());
}

fn node(type_name: &str, name: &str, children: Vec<TypeTree>) -> TypeTree {
    TypeTree {
        children,
        version: 0,
        is_array: false,
        size: 0,
        index: 0,
        flags: 0,
        type_name: type_name.to_string(),
        name: name.to_string(),
    }
}

fn string_node(name: &str) -> TypeTree {
    let mut child = node("Array", "Array", Vec::new());
    child.flags = 0x4000;
    node("string", name, vec![child])
}

fn array_node(type_name: &str, name: &str, item: TypeTree) -> TypeTree {
    let array = TypeTree {
        children: vec![node("int", "size", Vec::new()), item],
        version: 0,
        is_array: true,
        size: 0,
        index: 0,
        flags: 0x4000,
        type_name: "Array".to_string(),
        name: "Array".to_string(),
    };
    node(type_name, name, vec![array])
}

#[test]
fn exact_texture_cache_does_not_survive_environment_replacement_at_same_address() {
    let key = ObjectKey {
        asset: 0,
        path_id: 42,
    };
    let mut env = Box::new(UnityEnvironment::from_assets(vec![test_asset(7)]));
    let address = std::ptr::from_ref(env.as_ref());
    let first = crate::logical_model_material::exact_texture(&env, key).unwrap_err();
    assert!(first.contains("test.asset#42"));
    let mut other = test_asset(7);
    other.name = "other-source.asset".into();
    *env = UnityEnvironment::from_assets(vec![other]);
    assert_eq!(address, std::ptr::from_ref(env.as_ref()));
    let second = crate::logical_model_material::exact_texture(&env, key).unwrap_err();
    assert!(second.contains("other-source.asset#42"), "{second}");
    assert_ne!(first, second);
}

fn test_asset(format: u32) -> Asset {
    Asset {
        name: "test.asset".to_string(),
        data: Vec::new(),
        format,
        metadata_size: 0,
        file_size: 0,
        data_offset: 0,
        long_object_ids: false,
        tree: TypeMetadata::default(),
        objects: BTreeMap::new(),
        asset_refs: vec![AssetRef {
            asset_path: String::new(),
            guid: [0; 16],
            type_id: 0,
            file_path: "test.asset".to_string(),
        }],
    }
}

fn asset_with_object(name: &str, format: u32, path_id: i64) -> Asset {
    let mut asset = test_asset(format);
    asset.name = name.to_string();
    asset.asset_refs[0].file_path = name.to_string();
    asset.objects.insert(
        path_id,
        ObjectInfo {
            path_id,
            data_offset: 0,
            size: 0,
            type_id: 1,
            class_id: 1,
        },
    );
    asset
}

#[test]
fn unity_value_roundtrips_through_binary_writer() {
    let tree = node(
        "TestObject",
        "Base",
        vec![
            node("int", "m_Count", Vec::new()),
            string_node("m_Name"),
            array_node("vector", "m_Data", node("UInt8", "data", Vec::new())),
            node("PPtr<Mesh>", "m_Mesh", Vec::new()),
        ],
    );
    let asset = test_asset(7);
    let value = UnityValue::Object(BTreeMap::from([
        ("m_Count".to_string(), UnityValue::Int(42)),
        ("m_Name".to_string(), UnityValue::String("abc".to_string())),
        ("m_Data".to_string(), UnityValue::Bytes(vec![1, 2, 3, 4])),
        (
            "m_Mesh".to_string(),
            UnityValue::Pointer(Pointer {
                source_asset: 0,
                file_id: 0,
                path_id: 99,
            }),
        ),
    ]));

    let mut writer = BinaryWriter::new(Endian::Little);
    write_value(&asset, 0, &tree, &mut writer, &value).unwrap();
    let data = writer.into_inner();

    let mut reader = BinaryReader::from_slice(&data, Endian::Little);
    let read_back = read_value(&asset, 0, &tree, &mut reader).unwrap();

    assert_eq!(read_back, value);
    assert_eq!(reader.position(), data.len() as u64);
}

#[test]
fn format7_pointer_stays_32_bit_with_long_object_ids() {
    let tree = node("PPtr<GameObject>", "m_Object", Vec::new());
    let mut asset = test_asset(7);
    asset.long_object_ids = true;
    let value = UnityValue::Pointer(Pointer {
        source_asset: 0,
        file_id: 0,
        path_id: 0xff00_1234,
    });

    let mut writer = BinaryWriter::new(Endian::Little);
    write_value(&asset, 0, &tree, &mut writer, &value).unwrap();
    let data = writer.into_inner();

    assert_eq!(data.len(), 8);
    let mut reader = BinaryReader::from_slice(&data, Endian::Little);
    let read_back = read_value(&asset, 0, &tree, &mut reader).unwrap();

    assert_eq!(read_back, value);
    assert_eq!(reader.position(), data.len() as u64);
}

#[test]
fn format6_output_path_ids_are_signed_i32_values() {
    let mut asset = test_asset(6);
    asset.long_object_ids = false;

    assert_eq!(stored_path_id_for_asset(&asset, 0xff00_1234), -16_772_556);
    assert_eq!(stored_path_id_for_asset(&asset, 42), 42);
}

#[test]
fn rebuild_as_format6_sorts_by_serialized_signed_path_id() {
    let mut asset = test_asset(7);
    asset.long_object_ids = true;
    asset.tree.type_trees.insert(
        1,
        node(
            "TestObject",
            "Base",
            vec![node("int", "m_Value", Vec::new())],
        ),
    );
    for path_id in [1_i64, 0xffff_ff00_i64] {
        asset.objects.insert(
            path_id,
            ObjectInfo {
                path_id,
                data_offset: 0,
                size: 0,
                type_id: 1,
                class_id: 1,
            },
        );
    }
    let replacements = BTreeMap::from([
        (1_i64, 1_i32.to_le_bytes().to_vec()),
        (0xffff_ff00_i64, 2_i32.to_le_bytes().to_vec()),
    ]);

    let data = asset
        .rebuild_with_object_data_filtered_and_extra_as_format(
            None,
            &replacements,
            &[],
            Some(6),
        )
        .unwrap();

    let mut reader = BinaryReader::from_slice(&data, Endian::Big);
    let metadata_size = reader.read_u32().unwrap();
    let file_size = reader.read_u32().unwrap();
    let format = reader.read_u32().unwrap();
    let _data_offset = reader.read_u32().unwrap();
    reader.endian = Endian::Little;
    TypeMetadata::load(&mut reader, format, file_size, metadata_size).unwrap();
    let object_count = reader.read_u32().unwrap();
    assert_eq!(object_count, 2);
    let first_path_id = reader.read_i32().unwrap();
    reader.skip(4 + 4 + 4 + 2 + 2).unwrap();
    let second_path_id = reader.read_i32().unwrap();

    assert_eq!(first_path_id, -256);
    assert_eq!(second_path_id, 1);
}

#[test]
fn extra_objects_keep_pre_remapped_path_ids() {
    let mut asset = test_asset(7);
    asset.long_object_ids = true;
    asset.tree.type_trees.insert(
        1,
        node(
            "TestObject",
            "Base",
            vec![node("int", "m_Value", Vec::new())],
        ),
    );
    asset.objects.insert(
        58,
        ObjectInfo {
            path_id: 58,
            data_offset: 0,
            size: 0,
            type_id: 1,
            class_id: 1,
        },
    );
    let replacements = BTreeMap::from([(58_i64, 1_i32.to_le_bytes().to_vec())]);
    let extra_objects = vec![(
        ObjectInfo {
            path_id: 58,
            data_offset: 0,
            size: 0,
            type_id: 1,
            class_id: 1,
        },
        2_i32.to_le_bytes().to_vec(),
    )];
    let output_path_ids = BTreeMap::from([(58_i64, 12_i64)]);

    let data = asset
        .rebuild_with_object_data_filtered_and_extra_remapped_as_format(
            None,
            &replacements,
            &extra_objects,
            &output_path_ids,
            Some(7),
        )
        .unwrap();

    let temp = std::env::temp_dir().join("ff_unity_extra_object_path_ids.resourceFile");
    std::fs::write(&temp, &data).unwrap();
    let rebuilt = Asset::from_path(&temp).unwrap();
    let _ = std::fs::remove_file(&temp);
    assert!(rebuilt.objects.contains_key(&12));
    assert!(rebuilt.objects.contains_key(&58));
}

#[test]
fn empty_value_matches_tree_shape() {
    let tree = node(
        "TestObject",
        "Base",
        vec![
            node("bool", "m_Enabled", Vec::new()),
            string_node("m_Name"),
            array_node("vector", "m_Items", node("int", "data", Vec::new())),
        ],
    );
    let empty = empty_value_for_type(&tree).unwrap();
    let object = empty.as_object().unwrap();
    assert_eq!(object.get("m_Enabled"), Some(&UnityValue::Bool(false)));
    assert_eq!(
        object.get("m_Name"),
        Some(&UnityValue::String(String::new()))
    );
    assert_eq!(object.get("m_Items"), Some(&UnityValue::Array(Vec::new())));
}

#[test]
fn default_metadata_can_seed_core_asset_objects() {
    let asset = Asset::empty_with_metadata("generated.resourceFile", 6).unwrap();
    for class_id in [1, 4, 21, 28, 43, 74, 111, 137, 142] {
        let value = asset.empty_object_value_for_class(class_id).unwrap();
        asset
            .serialize_object_value_for_class(0, class_id, &value)
            .unwrap();
    }
}

#[test]
fn format7_pointer_resolution_stays_inside_asset_refs() {
    let unrelated = asset_with_object("unrelated.asset", 7, 10);
    let mut source = asset_with_object("source.asset", 7, 1);
    source.asset_refs.push(AssetRef {
        asset_path: String::new(),
        guid: [0; 16],
        type_id: 0,
        file_path: "target.asset".to_string(),
    });
    let target = asset_with_object("target.asset", 7, 10);
    let env = UnityEnvironment::from_assets(vec![unrelated, source, target]);

    let key = env
        .resolve_pointer(&Pointer {
            source_asset: 1,
            file_id: 0,
            path_id: 10,
        })
        .unwrap();

    assert_eq!(key.asset, 2);
    assert_eq!(key.path_id, 10);
}

#[test]
fn format7_nonzero_file_id_prefers_named_asset_ref() {
    let unrelated = asset_with_object("unrelated.asset", 7, 10);
    let mut source = asset_with_object("source.asset", 7, 10);
    source.asset_refs.push(AssetRef {
        asset_path: String::new(),
        guid: [0; 16],
        type_id: 0,
        file_path: "target.asset".to_string(),
    });
    let target = asset_with_object("target.asset", 7, 10);
    let env = UnityEnvironment::from_assets(vec![unrelated, source, target]);

    let key = env
        .resolve_pointer(&Pointer {
            source_asset: 1,
            file_id: 1,
            path_id: 10,
        })
        .unwrap();

    assert_eq!(key.asset, 2);
    assert_eq!(key.path_id, 10);
}

#[test]
fn format7_invalid_file_id_falls_back_to_local_path_id() {
    let source = asset_with_object("source.asset", 7, 10);
    let env = UnityEnvironment::from_assets(vec![source]);

    let key = env
        .resolve_pointer(&Pointer {
            source_asset: 0,
            file_id: 0x1234_5678,
            path_id: 10,
        })
        .unwrap();

    assert_eq!(key.asset, 0);
    assert_eq!(key.path_id, 10);
}

#[test]
fn format7_invalid_file_id_searches_loaded_assets_by_path_id() {
    let source = asset_with_object("source.asset", 7, 1);
    let target = asset_with_object("target.asset", 7, 10);
    let env = UnityEnvironment::from_assets(vec![source, target]);

    let key = env
        .resolve_pointer(&Pointer {
            source_asset: 0,
            file_id: 0x1234_5678,
            path_id: 10,
        })
        .unwrap();

    assert_eq!(key.asset, 1);
    assert_eq!(key.path_id, 10);
}

#[test]
fn strict_resolver_treats_stale_file_id_null_pointer_as_null() {
    let env = UnityEnvironment::from_assets(vec![asset_with_object("source.asset", 7, 1)]);
    let resolved = env
        .resolve_pointer_strict(&Pointer {
            source_asset: 0,
            file_id: 99,
            path_id: 0,
        })
        .unwrap();

    assert_eq!(resolved, None);
}

#[test]
fn academy_packed_pointer_requires_external_owner_and_full_object_tag() {
    // Raw Kimchi material shader PPtr: 01 00 e6 b1 7c 05 6f d1.
    // TrainingGrounds' object table: 7c 05 6f d1 e6 b1 00 00.
    let mut source = asset_with_object("kimchi.asset", 7, 1);
    source.long_object_ids = true;
    source.asset_refs.push(AssetRef {
        asset_path: String::new(),
        guid: [0; 16],
        type_id: 0,
        file_path: "training.asset".to_string(),
    });
    let mut target = asset_with_object("training.asset", 7, 0xd16f_057c);
    target.long_object_ids = true;
    target.objects.get_mut(&0xd16f_057c).unwrap().path_id = 0xb1e6_d16f_057c;
    let mut env = UnityEnvironment::from_assets(vec![source, target]);
    let pointer = Pointer {
        source_asset: 0,
        file_id: 0xb1e6_0001u32 as i32,
        path_id: 0xd16f_057c,
    };
    assert_eq!(
        env.resolve_pointer_strict(&pointer).unwrap().unwrap().asset,
        1
    );
    assert_eq!(env.resolve_pointer(&pointer).unwrap().asset, 1);
    // Matching low 32 bits alone cannot authorize another object.
    env.assets[1].objects.get_mut(&0xd16f_057c).unwrap().path_id = 0xabcd_d16f_057c;
    assert!(env
        .resolve_pointer_strict(&pointer)
        .unwrap_err()
        .contains("does not match"));
    assert!(env.resolve_pointer(&pointer).is_err());
    // Nor can an arbitrary loaded asset supply a missing named external.
    env.assets[1].name = "unrelated.asset".to_string();
    assert!(env
        .resolve_pointer_strict(&pointer)
        .unwrap_err()
        .contains("not loaded"));
}

#[test]
fn strict_local_pointer_never_searches_other_serialized_assets() {
    let source = asset_with_object("source.asset", 7, 1);
    let target = asset_with_object("target.asset", 7, 10);
    let env = UnityEnvironment::from_assets(vec![source, target]);
    let pointer = Pointer {
        source_asset: 0,
        file_id: 0,
        path_id: 10,
    };

    let error = env.resolve_pointer_strict(&pointer).unwrap_err();
    assert!(error.contains("absent from serialized asset source.asset"));

    let permissive = env.resolve_pointer(&pointer).unwrap();
    assert_eq!(permissive.asset, 1);
}

#[test]
fn strict_positive_file_id_never_falls_back_to_local_path_id() {
    let source = asset_with_object("source.asset", 7, 10);
    let env = UnityEnvironment::from_assets(vec![source]);
    let pointer = Pointer {
        source_asset: 0,
        file_id: 7,
        path_id: 10,
    };

    let error = env.resolve_pointer_strict(&pointer).unwrap_err();
    assert!(error.contains("outside its 0-entry table"));

    let permissive = env.resolve_pointer(&pointer).unwrap();
    assert_eq!(permissive.asset, 0);
}

#[test]
fn strict_positive_file_id_resolves_only_its_named_external() {
    let unrelated = asset_with_object("unrelated.asset", 7, 10);
    let mut source = asset_with_object("source.asset", 7, 1);
    source.asset_refs.push(AssetRef {
        asset_path: String::new(),
        guid: [1; 16],
        type_id: 0,
        file_path: "target.asset".to_string(),
    });
    let target = asset_with_object("target.asset", 7, 10);
    let env = UnityEnvironment::from_assets(vec![unrelated, source, target]);

    let resolved = env
        .resolve_pointer_strict(&Pointer {
            source_asset: 1,
            file_id: 1,
            path_id: 10,
        })
        .unwrap()
        .unwrap();

    assert_eq!(resolved.asset, 2);
    assert_eq!(resolved.path_id, 10);
}

#[test]
fn strict_positive_file_id_rejects_ambiguous_external_asset_name() {
    let mut source = asset_with_object("source.asset", 7, 1);
    source.asset_refs.push(AssetRef {
        asset_path: String::new(),
        guid: [2; 16],
        type_id: 0,
        file_path: "target.asset".to_string(),
    });
    let target_a = asset_with_object("target.asset", 7, 10);
    let target_b = asset_with_object("target.asset", 7, 10);
    let env = UnityEnvironment::from_assets(vec![source, target_a, target_b]);

    let error = env
        .resolve_pointer_strict(&Pointer {
            source_asset: 0,
            file_id: 1,
            path_id: 10,
        })
        .unwrap_err();

    assert!(error.contains("ambiguous across serialized assets"));
}
