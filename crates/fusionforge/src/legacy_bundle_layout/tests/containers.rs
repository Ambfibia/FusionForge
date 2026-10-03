use super::*;

#[test]
fn output_type_assignment_never_merges_equal_trees_across_unity_classes() {
    let tree = TypeTree {
        children: Vec::new(),
        version: 1,
        is_array: false,
        size: 4,
        index: 0,
        flags: 0,
        type_name: "AudioClip-shaped".to_string(),
        name: "Base".to_string(),
    };
    let mut trees = BTreeMap::new();
    let mut by_signature = BTreeMap::new();
    let mut next_synthetic = -1;
    let audio = assign_output_type(
        83,
        &tree,
        &mut trees,
        &mut by_signature,
        &mut next_synthetic,
    );
    let other = assign_output_type(
        21,
        &tree,
        &mut trees,
        &mut by_signature,
        &mut next_synthetic,
    );
    assert_eq!(audio, 83);
    assert_eq!(other, 21);
    assert_ne!(audio, other);
    assert_eq!(trees.len(), 2);
    assert_eq!(
        assign_output_type(
            83,
            &tree,
            &mut trees,
            &mut by_signature,
            &mut next_synthetic,
        ),
        audio
    );
}

#[test]
fn object_size_guard_accepts_only_zero_tail_or_final_align4_gap() {
    assert!(harmless_object_size_mismatch(&[1, 2, 0, 0], 2, 4));
    assert!(!harmless_object_size_mismatch(&[1, 2, 3, 0], 2, 4));
    assert!(harmless_object_size_mismatch(
        &vec![0; 10_998],
        11_000,
        10_998
    ));
    assert!(harmless_object_size_mismatch(
        &vec![0; 10_999],
        11_000,
        10_999
    ));
    assert!(!harmless_object_size_mismatch(
        &vec![0; 10_998],
        11_004,
        10_998
    ));
    assert!(!harmless_object_size_mismatch(
        &vec![0; 10_998],
        11_001,
        10_998
    ));
}

#[test]
fn exact_dong_contraction_discards_contaminated_object_phase_provenance() {
    let dong_family = Family::Dong("DongResources_06_03".to_string());
    let mut contaminated = test_packing_unit(dong_family.clone(), 1, &[]);
    contaminated.sections = BTreeSet::from([
        "m_FreeZoneComplete".to_string(),
        "m_PaidZoneComplete".to_string(),
    ]);
    contaminated.sections_pinned = true;
    let literal_paid = BTreeSet::from(["m_PaidZoneComplete".to_string()]);
    let literal_sections = BTreeMap::from([(dong_family.clone(), literal_paid.clone())]);

    let contracted = contract_exact_dong_units(vec![contaminated], &literal_sections).unwrap();
    assert_eq!(contracted.len(), 1);
    assert_eq!(contracted[0].sections, literal_paid);
    let packed = pack_dependency_units(
        &contracted,
        PART_METADATA_RESERVE_BYTES.saturating_add(1024),
    )
    .unwrap();
    assert_eq!(packed.len(), 1);
    assert_eq!(
        packed[0].sections,
        BTreeSet::from(["m_PaidZoneComplete".to_string()])
    );
}

#[test]
fn exact_dong_may_exceed_inner_estimate_but_never_physical_bundle_limit() {
    let mut dong = test_packing_unit(Family::Dong("DongResources_01_01".to_string()), 10, &[]);
    dong.nodes.push((0, 1));
    let max_part_bytes = PART_METADATA_RESERVE_BYTES + 9;
    let packed = pack_dependency_units(&[dong], max_part_bytes).unwrap();
    assert_eq!(packed.len(), 1);
    assert!(packed[0].estimated_bytes > max_part_bytes);

    let part = Part {
        family: Family::Dong("DongResources_01_01".to_string()),
        sections: BTreeSet::new(),
        ordinal: 0,
        nodes: vec![(0, 1)],
        estimated_bytes: packed[0].estimated_bytes,
        output_name: "DongResources_01_01.resourceFile".to_string(),
        internal_name: "CustomAssetBundle-test".to_string(),
    };
    assert!(validate_inner_serialized_size(&part, max_part_bytes + 1, max_part_bytes).is_ok());
    assert!(
        validate_physical_bundle_size(&part.output_name, max_part_bytes, max_part_bytes)
            .is_ok()
    );
    let error =
        validate_physical_bundle_size(&part.output_name, max_part_bytes + 1, max_part_bytes)
            .unwrap_err();
    assert!(error.contains("DongResources_01_01.resourceFile"));
    assert!(error.contains("final .resourceFile"));
}
