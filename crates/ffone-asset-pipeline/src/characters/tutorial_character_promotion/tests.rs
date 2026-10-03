use super::*;

#[test]
fn closed_promotion_set_excludes_props_and_missing_nano() {
    let ids = CANDIDATES
        .iter()
        .map(|candidate| candidate.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), 8);
    assert!(!ids.contains("etc_domeglass_04"));
    assert!(!ids.contains("npc_building"));
    assert!(!ids.contains("nano_buttercup"));
}

#[test]
fn fixed_candidates_have_general_character_destinations() {
    let paths = CANDIDATES
        .iter()
        .map(|candidate| {
            format!(
                "{}/{}/{}",
                category_directory(candidate.category),
                candidate.id,
                candidate.glb_name
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paths[0],
        "characters/fusions/fusion_buttercup/fusion_buttercup.glb"
    );
    assert!(paths.contains(&"characters/mobs/mob_bat/mob_bat.glb".to_owned()));
    assert!(paths.contains(&"characters/npcs/npc_ben/npc_ben.glb".to_owned()));
}
