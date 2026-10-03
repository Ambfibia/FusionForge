use std::collections::BTreeSet;

use super::{MAX_COMPONENT_LEN, allocate_component, compact_slug};

#[test]
fn repeated_legacy_names_collapse_to_the_first_semantic_name() {
    assert_eq!(
        compact_slug(
            "wd_dlfl_war_obj_dark_cannon_plate_02_wd_dlfl_war_obj_dark_cannon_plate_02_dds_wd_dlfl_war_obj_dark_cannon_plate",
            "object",
        ),
        "wd_dlfl_war_obj_dark_cannon_plate_02"
    );
}

#[test]
fn variants_remain_unique_and_component_bounded() {
    let mut used = BTreeSet::new();
    let first = allocate_component(&"a".repeat(100), &mut used);
    let second = allocate_component(&"a".repeat(100), &mut used);
    assert!(first.len() <= MAX_COMPONENT_LEN);
    assert!(second.len() <= MAX_COMPONENT_LEN);
    assert_ne!(first, second);
    assert!(second.ends_with("_variant_0002"));
}
