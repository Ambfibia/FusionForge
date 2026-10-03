use super::*;

#[test]
fn runtime_character_replacement_path_is_strictly_relative_glb() {
    assert_eq!(
        normalize_relative_glb("characters\\mobs\\oil\\oil.glb").unwrap(),
        "characters/mobs/oil/oil.glb"
    );
    for invalid in ["", "/oil.glb", "../oil.glb", "oil.GLB", "oil.gltf"] {
        assert!(normalize_relative_glb(invalid).is_err(), "{invalid:?}");
    }
}

#[test]
fn runtime_character_install_id_maps_to_semantic_directory() {
    assert_eq!(
        parse_runtime_character_id("npc/npc_bubbie2").unwrap(),
        (RuntimeCharacterCategory::Npc, "npcs", "npc_bubbie2")
    );
    for invalid in [
        "npc",
        "npc/",
        "npc/Npc_Bubbie2",
        "npc/npc-bubbie2",
        "unknown/npc_bubbie2",
        "npc/npc_bubbie2/extra",
    ] {
        assert!(parse_runtime_character_id(invalid).is_err(), "{invalid:?}");
    }
}
