use super::*;

#[test]
fn runtime_true_names_are_portable() {
    assert!(valid_true_name("mob_kevinlevin"));
    assert!(!valid_true_name("../npc_kevin"));
    assert!(!valid_true_name("Npc_Kevin"));
}
