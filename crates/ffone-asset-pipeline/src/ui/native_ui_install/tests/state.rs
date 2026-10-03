use super::*;

pub(super) fn runtime_tree_sha256(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut digest = Sha256::new();
    for (path, bytes) in files {
        digest.update((path.len() as u64).to_le_bytes());
        digest.update(path.as_bytes());
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn player_fm_status_and_minimap_mission_routes_are_exact() {
    const EXPECTED: [(&str, &str, &str); 5] = [
        (
            "combat_toggle.png",
            "ui/gameplay/player/combat_toggle.png",
            "player/combat_toggle.png",
        ),
        (
            "exp_back",
            "ui/gameplay/player/fusion_matter_back.png",
            "player/fusion_matter_back.png",
        ),
        (
            "exp_bar",
            "ui/gameplay/player/fusion_matter_bar.png",
            "player/fusion_matter_bar.png",
        ),
        (
            "map_icon_15",
            "ui/gameplay/minimap/map_icon_15.png",
            "minimap/map_icon_15.png",
        ),
        (
            "map_icon_16",
            "ui/gameplay/minimap/map_icon_16.png",
            "minimap/map_icon_16.png",
        ),
    ];

    for (legacy_name, source, destination) in EXPECTED {
        let route = ROUTES
            .iter()
            .find(|route| route.legacy_name == legacy_name)
            .unwrap_or_else(|| panic!("missing exact native UI route {legacy_name}"));
        assert_eq!(route.source, source);
        assert_eq!(route.destination, destination);
        assert_eq!(route.kind, ProjectAssetKind::Texture);
    }
}

#[test]
fn group_status_routes_and_embedded_gui_skin_are_exact() {
    const EXPECTED: [(&str, &str, &str); 5] = [
        (
            "group_info",
            "ui/gameplay/group/group_info.png",
            "group/group_info.png",
        ),
        (
            "group_freechat_icon",
            "ui/gameplay/group/group_freechat_icon.png",
            "group/group_freechat_icon.png",
        ),
        (
            "group_nano_hpframe",
            "ui/gameplay/group/group_nano_hpframe.png",
            "group/group_nano_hpframe.png",
        ),
        (
            "npc_co_op",
            "ui/gameplay/group/npc_co_op.png",
            "group/npc_co_op.png",
        ),
        ("HP_BAR", "ui/gameplay/group/hp_bar.png", "group/hp_bar.png"),
    ];

    for (legacy_name, source, destination) in EXPECTED {
        let route = ROUTES
            .iter()
            .find(|route| route.destination == destination)
            .unwrap_or_else(|| panic!("missing exact native group UI route {destination}"));
        assert_eq!(route.legacy_name, legacy_name);
        assert_eq!(route.source, source);
        assert_eq!(route.kind, ProjectAssetKind::Texture);
        assert!(strict_route_proof(source).is_some());
    }

    assert_eq!(GUI_SKIN_BYTES.len(), 2_055_506);
    assert_eq!(
        blake3::hash(GUI_SKIN_BYTES).to_hex().as_str(),
        GUI_SKIN_BLAKE3
    );
}
