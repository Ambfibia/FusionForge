use super::*;

pub(super) fn cfg() -> LegacyConfig {
    config(&serde_json::json!({ "BundleLayout": { "Enabled": true } }))
}

#[test]
fn only_dong_roots_receive_tile_scopes() {
    assert_eq!(root_scope(&Family::Npc), None);
    assert_eq!(root_scope(&Family::Core), None);
    assert_eq!(
        root_scope(&Family::Dong("DongResources_05_04".to_string())),
        Some("dongresources_05_04".to_string())
    );
}

#[test]
fn only_proven_missing_local_or_known_internal_targets_are_clearable() {
    let object = |path_id| ObjectInfo {
        path_id,
        data_offset: 0,
        size: 0,
        type_id: 0,
        class_id: 0,
    };
    let reference = |file_path: &str| AssetRef {
        asset_path: String::new(),
        guid: [0; 16],
        type_id: 0,
        file_path: file_path.to_string(),
    };
    let source = SourceAsset {
        bundle_index: 0,
        path: PathBuf::new(),
        name: "source".to_string(),
        format: 9,
        tree: TypeMetadata::default(),
        objects: BTreeMap::from([(11, object(11))]),
        refs: vec![reference(""), reference("target"), reference("external")],
    };
    let target = SourceAsset {
        bundle_index: 0,
        path: PathBuf::new(),
        name: "target".to_string(),
        format: 9,
        tree: TypeMetadata::default(),
        objects: BTreeMap::from([(6826, object(6826))]),
        refs: vec![reference("")],
    };
    let assets = vec![source, target];
    let by_internal_name = BTreeMap::from([
        ("source".to_string(), vec![0]),
        ("target".to_string(), vec![1]),
    ]);
    let pointer = |file_id, path_id| Pointer {
        source_asset: 0,
        file_id,
        path_id,
    };

    assert!(
        !proven_missing_source_target(&pointer(0, 11), &assets, &by_internal_name).unwrap()
    );
    assert!(proven_missing_source_target(&pointer(0, 12), &assets, &by_internal_name).unwrap());
    assert!(
        !proven_missing_source_target(&pointer(1, 6826), &assets, &by_internal_name).unwrap()
    );
    assert!(
        proven_missing_source_target(&pointer(1, 6827), &assets, &by_internal_name).unwrap()
    );

    let external = pointer(2, 6827);
    assert!(!proven_missing_source_target(&external, &assets, &by_internal_name).unwrap());
    assert!(preserved_external_ref(&external, &assets, &by_internal_name).is_some());

    let invalid_file_id = pointer(3, 6827);
    assert!(
        proven_missing_source_target(&invalid_file_id, &assets, &by_internal_name).unwrap()
    );
    assert!(preserved_external_ref(&invalid_file_id, &assets, &by_internal_name).is_none());
}

#[test]
fn exact_dangling_pointer_clear_has_a_strict_occurrence_guard() {
    let missing = Pointer {
        source_asset: 4,
        file_id: 3,
        path_id: 6827,
    };
    let other_target = Pointer {
        source_asset: 4,
        file_id: 3,
        path_id: 6828,
    };
    let other_source = Pointer {
        source_asset: 5,
        file_id: 3,
        path_id: 6827,
    };
    let stale_null = Pointer {
        source_asset: 4,
        file_id: 3,
        path_id: 0,
    };
    let recorded = vec![
        DanglingObjectPointer {
            source: SourcePointerKey::from(&missing),
            description: "first".to_string(),
        },
        DanglingObjectPointer {
            source: SourcePointerKey::from(&missing),
            description: "second".to_string(),
        },
    ];
    let mut value = UnityValue::Array(vec![
        UnityValue::Pointer(missing.clone()),
        UnityValue::Object(BTreeMap::from([(
            "nested".to_string(),
            UnityValue::Pair(
                Box::new(UnityValue::Pointer(missing.clone())),
                Box::new(UnityValue::Pointer(other_target.clone())),
            ),
        )])),
        UnityValue::Pointer(other_source.clone()),
        UnityValue::Pointer(stale_null.clone()),
    ]);

    assert_eq!(
        clear_recorded_dangling_pointers(&mut value, &recorded).unwrap(),
        2
    );
    assert_eq!(
        value,
        UnityValue::Array(vec![
            pointer_value(0, 0),
            UnityValue::Object(BTreeMap::from([(
                "nested".to_string(),
                UnityValue::Pair(
                    Box::new(pointer_value(0, 0)),
                    Box::new(UnityValue::Pointer(other_target)),
                ),
            )])),
            UnityValue::Pointer(other_source),
            UnityValue::Pointer(stale_null),
        ])
    );

    let mut drifted_value = UnityValue::Pointer(missing);
    let error = clear_recorded_dangling_pointers(&mut drifted_value, &recorded).unwrap_err();
    assert!(error.contains("planned 2"));
    assert!(error.contains("found 1"));
}

#[test]
fn dependency_order_reports_only_the_real_part_cycle() {
    let part = |output_name: &str, family: Family| Part {
        family,
        sections: BTreeSet::new(),
        ordinal: 0,
        nodes: Vec::new(),
        estimated_bytes: 0,
        output_name: output_name.to_string(),
        internal_name: output_internal_name(output_name),
    };
    let parts = vec![
        part("NPC_Pack_001.resourceFile", Family::Npc),
        part("Items_Pack_001.resourceFile", Family::Items),
        part("CoreShared.resourceFile", Family::Core),
    ];
    let dependencies = BTreeMap::from([
        (0usize, BTreeSet::from([1usize])),
        (1usize, BTreeSet::from([0usize])),
        // This part is blocked by the cycle, but is not a member of it.
        (2usize, BTreeSet::from([0usize])),
    ]);

    assert_eq!(
        dependency_cycle_components(&BTreeSet::from([0, 1, 2]), &dependencies),
        vec![vec![0, 1]]
    );
    let error = dependency_order(&parts, &dependencies).unwrap_err();
    assert!(error.contains("NPC_Pack_001.resourceFile [NPC]"));
    assert!(error.contains("Items_Pack_001.resourceFile [Items]"));
    assert!(error.contains("NPC_Pack_001.resourceFile -> Items_Pack_001.resourceFile"));
    assert!(error.contains("Items_Pack_001.resourceFile -> NPC_Pack_001.resourceFile"));
    assert!(!error.contains("CoreShared.resourceFile [CoreShared]"));
}

pub(super) fn test_packing_unit(family: Family, size: u64, dependencies: &[usize]) -> PackingUnit {
    PackingUnit {
        family,
        sections: BTreeSet::new(),
        sections_pinned: false,
        nodes: Vec::new(),
        roots: Vec::new(),
        estimated_bytes: size,
        owner: CompactOwner::None,
        dependencies: dependencies.to_vec(),
    }
}

pub(super) fn packed_part_dependencies(
    units: &[PackingUnit],
    packed: &[PackedUnitsPart],
) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut unit_parts = vec![usize::MAX; units.len()];
    for (part, value) in packed.iter().enumerate() {
        for unit in &value.units {
            unit_parts[*unit] = part;
        }
    }
    let mut dependencies = BTreeMap::<usize, BTreeSet<usize>>::new();
    for (unit, value) in units.iter().enumerate() {
        for dependency in &value.dependencies {
            let source_part = unit_parts[unit];
            let dependency_part = unit_parts[*dependency];
            if source_part != dependency_part {
                dependencies
                    .entry(source_part)
                    .or_default()
                    .insert(dependency_part);
            }
        }
    }
    dependencies
}

pub(super) fn test_parts(packed: &[PackedUnitsPart]) -> Vec<Part> {
    packed
        .iter()
        .enumerate()
        .map(|(index, packed)| Part {
            family: packed.family.clone(),
            sections: packed.sections.clone(),
            ordinal: index,
            nodes: Vec::new(),
            estimated_bytes: packed.estimated_bytes,
            output_name: format!("test_part_{index}.resourceFile"),
            internal_name: format!("test_part_{index}"),
        })
        .collect()
}

#[test]
fn dependency_first_packing_does_not_recreate_the_old_ffd_cycle() {
    // Two independent DAG branches: a -> b and c -> d. The former FFD could create
    // blocks {a,d} and {b,c}, yielding both block0 -> block1 and block1 -> block0.
    let units = vec![
        test_packing_unit(Family::Npc, 6, &[1]), // a
        test_packing_unit(Family::Npc, 6, &[]),  // b
        test_packing_unit(Family::Npc, 6, &[3]), // c
        test_packing_unit(Family::Npc, 6, &[]),  // d
    ];
    let old_parts = [0usize, 1, 1, 0];
    let old_dependencies = BTreeMap::from([
        (old_parts[0], BTreeSet::from([old_parts[1]])),
        (old_parts[2], BTreeSet::from([old_parts[3]])),
    ]);
    assert!(dependency_order(
        &test_parts(&[
            PackedUnitsPart {
                family: Family::Npc,
                sections: BTreeSet::new(),
                units: vec![0, 3],
                estimated_bytes: 12,
            },
            PackedUnitsPart {
                family: Family::Npc,
                sections: BTreeSet::new(),
                units: vec![1, 2],
                estimated_bytes: 12,
            },
        ]),
        &old_dependencies
    )
    .is_err());

    let packed = pack_dependency_units(&units, PART_METADATA_RESERVE_BYTES + 12).unwrap();
    let dependencies = packed_part_dependencies(&units, &packed);
    assert!(dependency_order(&test_parts(&packed), &dependencies).is_ok());
}

#[test]
fn synthetic_root_unit_owns_the_late_family_part_without_a_cycle() {
    // h1 -> (nothing), core -> h1, root(HNPC) -> core. Assigning the root to the
    // first HNPC part would add h1-part -> core and cycle with core -> h1-part.
    let mut units = vec![
        test_packing_unit(Family::Hnpc, 6, &[]),
        test_packing_unit(Family::Core, 6, &[0]),
        test_packing_unit(Family::Hnpc, 0, &[1]),
    ];
    units[2].roots.push(0);
    let packed = pack_dependency_units(&units, PART_METADATA_RESERVE_BYTES + 10).unwrap();
    let root_part = packed
        .iter()
        .position(|part| part.units.contains(&2))
        .unwrap();
    let first_hnpc_part = packed
        .iter()
        .position(|part| part.family == Family::Hnpc)
        .unwrap();
    assert_ne!(root_part, first_hnpc_part);
    let dependencies = packed_part_dependencies(&units, &packed);
    assert!(dependency_order(&test_parts(&packed), &dependencies).is_ok());
}

#[test]
fn exact_dong_contraction_reports_a_real_atomicity_cycle() {
    // Dong(a) -> core -> Dong(b) is a DAG until both Dong vertices are contracted.
    let dong_family = Family::Dong("DongResources_01_01".to_string());
    let units = vec![
        test_packing_unit(dong_family.clone(), 1, &[1]),
        test_packing_unit(Family::Core, 1, &[2]),
        test_packing_unit(dong_family.clone(), 1, &[]),
    ];
    let literal_sections = BTreeMap::from([(
        dong_family,
        BTreeSet::from(["m_FreeZoneComplete".to_string()]),
    )]);
    let contracted = contract_exact_dong_units(units, &literal_sections).unwrap();
    let error =
        pack_dependency_units(&contracted, PART_METADATA_RESERVE_BYTES + 1024).unwrap_err();
    assert!(error.contains("keeping exact DongResources_* atomic"));
    assert!(error.contains("DongResources_01_01"));
}

#[test]
fn oversized_non_dong_atomic_units_are_rejected_before_packing() {
    let mut scc = test_packing_unit(Family::Npc, 10, &[]);
    scc.nodes.push((0, 1));
    let error = pack_dependency_units(&[scc], PART_METADATA_RESERVE_BYTES + 9).unwrap_err();
    assert!(error.contains("atomic object SCC"));
    assert!(error.contains("cannot be split safely"));
}

#[test]
fn scc_keeps_a_pointer_cycle_atomic_for_part_packing() {
    let a = (0, 10);
    let b = (0, 20);
    let make = |key: NodeKey, edges: Vec<NodeKey>| Node {
        key,
        object_type: "GameObject".to_string(),
        name: format!("node_{}", key.1),
        raw_hash: String::new(),
        semantic_hash: None,
        audio_payload_sha1: None,
        shape_hash: String::new(),
        type_tree_hash: String::new(),
        size: 1,
        edges,
    };
    let nodes = BTreeMap::from([(a, make(a, vec![b])), (b, make(b, vec![a]))]);
    let keys = BTreeSet::from([a, b]);
    let canonical = BTreeMap::from([(a, a), (b, b)]);
    let components = strongly_connected_components(&keys, &nodes, &canonical);
    assert_eq!(components, vec![vec![a, b]]);
}

#[test]
fn packer_never_coalesces_incompatible_nano_phases() {
    let mut future = test_packing_unit(Family::Nano, 1, &[]);
    future.sections = BTreeSet::from(["m_FreeZone".to_string()]);
    future.sections_pinned = true;
    let mut past = test_packing_unit(Family::Nano, 1, &[]);
    past.sections = BTreeSet::from(["m_PaidZone".to_string()]);
    past.sections_pinned = true;

    let packed = pack_dependency_units(
        &[future, past],
        PART_METADATA_RESERVE_BYTES.saturating_add(1024),
    )
    .unwrap();
    assert_eq!(packed.len(), 2);
    assert_eq!(
        packed
            .iter()
            .map(|part| part.sections.clone())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            BTreeSet::from(["m_FreeZone".to_string()]),
            BTreeSet::from(["m_PaidZone".to_string()]),
        ])
    );
}

#[test]
fn consumer_phases_make_dependency_only_units_self_contained() {
    let mut consumer = test_packing_unit(Family::Core, 1, &[1, 2, 3]);
    consumer.sections = BTreeSet::from(["m_CharacterSelection".to_string()]);
    consumer.sections_pinned = true;
    let mut paid_nano = test_packing_unit(Family::Nano, 1, &[]);
    paid_nano.sections = BTreeSet::from(["m_PaidZone".to_string()]);
    paid_nano.sections_pinned = true;
    let custom_dependency = test_packing_unit(Family::Core, 1, &[]);
    let mut complete_dependency = test_packing_unit(Family::WorldShared, 1, &[]);
    complete_dependency.sections = BTreeSet::from(["m_PaidZoneComplete".to_string()]);
    complete_dependency.sections_pinned = true;
    let mut units = vec![consumer, paid_nano, custom_dependency, complete_dependency];

    propagate_dependency_sections(&mut units);
    assert_eq!(
        units[1].sections,
        BTreeSet::from(["m_CharacterSelection".to_string(), "m_PaidZone".to_string(),])
    );
    assert_eq!(units[1].family, Family::Nano);
    assert_eq!(
        units[2].sections,
        BTreeSet::from(["m_CharacterSelection".to_string()])
    );
    assert_eq!(
        units[3].sections,
        BTreeSet::from([
            "m_CharacterSelection".to_string(),
            "m_PaidZoneComplete".to_string(),
        ])
    );
}

#[test]
fn cross_phase_nano_root_and_its_packing_unit_remain_nano() {
    let mut roots = vec![Root {
        path: "nano/shared_cross_phase.prefab".to_string(),
        normalized_path: "nano/shared_cross_phase.prefab".to_string(),
        target: (0, 10),
        preload_roots: Vec::new(),
        preserved_preloads: Vec::new(),
        source_asset: 0,
        source_assetbundle: 1,
        source_entry: UnityValue::Object(BTreeMap::new()),
        family: Family::Nano,
        semantic_owner: SemanticOwnerHint::None,
        sections: BTreeSet::from(["m_FreeZone".to_string(), "m_PaidZone".to_string()]),
        output_part: None,
    }];

    let runtime_classes = normalize_root_runtime_classes(&mut roots, &cfg());
    assert_eq!(roots[0].family, Family::Nano);
    let (sections, sections_pinned) = runtime_classes.into_iter().next().unwrap();
    let units = vec![PackingUnit {
        family: roots[0].family.clone(),
        sections,
        sections_pinned,
        nodes: Vec::new(),
        roots: vec![0],
        estimated_bytes: 0,
        owner: CompactOwner::None,
        dependencies: Vec::new(),
    }];
    let packed =
        pack_dependency_units(&units, PART_METADATA_RESERVE_BYTES.saturating_add(1024))
            .unwrap();
    let parts = test_parts(&packed);
    let output_part = packed
        .iter()
        .position(|part| part.units.contains(&0))
        .unwrap();
    roots[0].output_part = Some(output_part);

    assert_eq!(parts[output_part].family, Family::Nano);
    assert_eq!(parts[output_part].family, roots[0].family);
}

#[test]
fn layout_cache_hash_detects_same_length_content_changes() {
    let temp = crate::native_build_temp_dir("layout_cache_hash_test").unwrap();
    let path = temp.path().join("same-size.resourceFile");
    fs::write(&path, b"first").unwrap();
    let first = sha256_file(&path).unwrap();
    fs::write(&path, b"other").unwrap();
    let second = sha256_file(&path).unwrap();
    assert_eq!(first.0, second.0);
    assert_ne!(first.1, second.1);
}

#[test]
fn layout_cache_key_is_bound_to_valid_stable_input_digest() {
    let patch_config = serde_json::json!({"BundleLayout": {"Enabled": true}});
    let first_input = "1".repeat(64);
    let second_input = "2".repeat(64);
    let first = result_cache_key(&patch_config, &first_input).unwrap();
    let repeated = result_cache_key(&patch_config, &first_input).unwrap();
    let changed = result_cache_key(&patch_config, &second_input).unwrap();
    assert_eq!(first, repeated);
    assert_ne!(first, changed);
    assert!(result_cache_key(&patch_config, "not-a-sha256").is_err());
}
