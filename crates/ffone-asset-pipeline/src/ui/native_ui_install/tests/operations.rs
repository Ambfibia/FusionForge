use super::*;

pub(super) fn first_verified_bytes() -> (&'static VerifiedIconRoute, Vec<u8>) {
    let route = &VERIFIED_ICON_ROUTES[0];
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
        .join("game")
        .join(route.source.replace('/', std::path::MAIN_SEPARATOR_STR));
    let bytes = fs::read(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    (route, bytes)
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn exact_table_data_icon_source_is_accepted() {
    let (route, bytes) = first_verified_bytes();
    let manifest = manifest_entry(route, &bytes);
    verify_table_data_icon_source(&manifest, route, &bytes).unwrap();
}

#[test]
fn minimap_waypoint_and_tutorial_exit_dialog_routes_are_exact() {
    const EXPECTED: [(&str, &str, &str); 5] = [
        (
            "map_icon_02",
            "ui/gameplay/minimap/map_icon_02.png",
            "minimap/map_icon_02.png",
        ),
        (
            "map_icon_03",
            "ui/gameplay/minimap/map_icon_03.png",
            "minimap/map_icon_03.png",
        ),
        (
            "plus_but_1",
            "ui/gameplay/minimap/plus_but_1.png",
            "minimap/plus_but_1.png",
        ),
        (
            "minus_but_1",
            "ui/gameplay/minimap/minus_but_1.png",
            "minimap/minus_but_1.png",
        ),
        (
            "systemDialogBox",
            "ui/gameplay/system/systemDialogBox.png",
            "system/systemDialogBox.png",
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
        assert!(strict_route_proof(source).is_some());
    }
    assert_eq!(ROUTES.len(), 145);
    assert_eq!(VERIFIED_ICON_ROUTES.len(), 79);
    assert_eq!(ROUTES.len() + VERIFIED_ICON_ROUTES.len() + 6, 230);
}

#[test]
fn retrobution_tutorial_hint_routes_are_complete_and_exact() {
    const EXPECTED: [(&str, &str); 12] = [
        ("tut_down", "ui/gameplay/tutorial/tut_down.png"),
        ("tut_jump", "ui/gameplay/tutorial/tut_jump.png"),
        ("tut_left", "ui/gameplay/tutorial/tut_left.png"),
        ("tut_lmouse", "ui/gameplay/tutorial/tut_lmouse.png"),
        ("tut_mouse", "ui/gameplay/tutorial/tut_mouse.png"),
        ("tut_move", "ui/gameplay/tutorial/tut_move.png"),
        ("tut_move_s", "ui/gameplay/tutorial/tut_move_s.png"),
        ("tut_move_w", "ui/gameplay/tutorial/tut_move_w.png"),
        ("tut_one", "ui/gameplay/tutorial/tut_one.png"),
        ("tut_right", "ui/gameplay/tutorial/tut_right.png"),
        ("tut_rmouse", "ui/gameplay/tutorial/tut_rmouse.png"),
        ("tut_up", "ui/gameplay/tutorial/tut_up.png"),
    ];

    let expected = EXPECTED.into_iter().collect::<BTreeMap<_, _>>();
    let routes = ROUTES
        .iter()
        .filter(|route| route.destination.starts_with("tutorial/"))
        .collect::<Vec<_>>();
    let actual = routes
        .iter()
        .map(|route| (route.legacy_name, route.source))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(routes.len(), EXPECTED.len());
    assert_eq!(actual, expected);
    assert_eq!(
        routes
            .iter()
            .map(|route| route.destination)
            .collect::<BTreeSet<_>>()
            .len(),
        EXPECTED.len()
    );

    for route in routes {
        assert_eq!(route.kind, ProjectAssetKind::Texture);
        assert_eq!(
            route.destination,
            format!("tutorial/{}.png", route.legacy_name)
        );
    }
}

#[test]
fn retrobution_tutorial_mission_ui_routes_are_complete_and_exact() {
    const EXPECTED: [(&str, &str, &str); 27] = [
        (
            "acceptbut",
            "ui/gameplay/mission/journal/acceptbut.png",
            "mission/journal/acceptbut.png",
        ),
        (
            "activedlg",
            "ui/gameplay/mission/journal/activedlg.png",
            "mission/journal/activedlg.png",
        ),
        (
            "allow_right",
            "ui/gameplay/mission/journal/allow_right.png",
            "mission/journal/allow_right.png",
        ),
        ("close", "ui/rule/close.png", "mission/journal/close.png"),
        (
            "closeover",
            "ui/world-map/controls/closeover.png",
            "mission/journal/close_over.png",
        ),
        (
            "current_mission",
            "ui/gameplay/mission/npc/current_mission.png",
            "mission/npc/current_mission.png",
        ),
        (
            "enddlg",
            "ui/gameplay/mission/journal/enddlg.png",
            "mission/journal/enddlg.png",
        ),
        (
            "fmicon",
            "ui/gameplay/mission/journal/fmicon.png",
            "mission/journal/fmicon.png",
        ),
        (
            "menu_box",
            "ui/gameplay/mission/nanocom/menu_box.png",
            "mission/nanocom/menu_box.png",
        ),
        (
            "mission_back",
            "ui/gameplay/mission/npc/mission_back.png",
            "mission/npc/mission_back.png",
        ),
        (
            "mission_body",
            "ui/gameplay/mission/npc/mission_body.png",
            "mission/npc/mission_body.png",
        ),
        (
            "mission_bottom_func",
            "ui/gameplay/mission/npc/mission_bottom_func.png",
            "mission/npc/mission_bottom_func.png",
        ),
        (
            "mission_bottom_func2",
            "ui/gameplay/mission/npc/mission_bottom_func2.png",
            "mission/npc/mission_bottom_func2.png",
        ),
        (
            "mission_button",
            "ui/gameplay/mission/npc/mission_button.png",
            "mission/npc/mission_button.png",
        ),
        (
            "mission_button_over",
            "ui/gameplay/mission/npc/mission_button_over.png",
            "mission/npc/mission_button_over.png",
        ),
        (
            "mission_top",
            "ui/gameplay/mission/npc/mission_top.png",
            "mission/npc/mission_top.png",
        ),
        (
            "multifunc_win",
            "ui/gameplay/mission/npc/npc_multi_window.png",
            "mission/npc/npc_multi_window.png",
        ),
        (
            "NanoMachineBG2",
            "ui/gameplay/mission/journal/window.png",
            "mission/journal/window.png",
        ),
        (
            "ncp_icon_back",
            "ui/gameplay/guide/ncp_icon_back.png",
            "mission/journal/npc_icon_back.png",
        ),
        (
            "npcicon_exit",
            "ui/gameplay/mission/npc/npcicon_exit.png",
            "mission/npc/npcicon_exit.png",
        ),
        (
            "npcicon_mission",
            "ui/gameplay/mission/npc/npcicon_mission.png",
            "mission/npc/npcicon_mission.png",
        ),
        (
            "npcicon_warp",
            "ui/gameplay/mission/npc/npcicon_warp.png",
            "mission/npc/npcicon_warp.png",
        ),
        (
            "npc_win",
            "ui/gameplay/mission/npc/npc_window.png",
            "mission/npc/npc_window.png",
        ),
        (
            "offdlg",
            "ui/gameplay/mission/journal/offdlg.png",
            "mission/journal/offdlg.png",
        ),
        (
            "sel_mission_back",
            "ui/gameplay/mission/npc/sel_mission_back.png",
            "mission/npc/sel_mission_back.png",
        ),
        (
            "rwd_box",
            "ui/gameplay/mission/journal/reward_box.png",
            "mission/journal/reward_box.png",
        ),
        (
            "slotbox",
            "ui/gameplay/quick-slot/slotbox.png",
            "mission/journal/item_slot.png",
        ),
    ];

    let expected = EXPECTED
        .into_iter()
        .map(|(name, source, destination)| (name, (source, destination)))
        .collect::<BTreeMap<_, _>>();
    let routes = ROUTES
        .iter()
        .filter(|route| route.destination.starts_with("mission/"))
        .collect::<Vec<_>>();
    let actual = routes
        .iter()
        .map(|route| (route.legacy_name, (route.source, route.destination)))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(routes.len(), EXPECTED.len());
    assert_eq!(actual, expected);
    assert_eq!(
        routes
            .iter()
            .map(|route| route.destination)
            .collect::<BTreeSet<_>>()
            .len(),
        EXPECTED.len()
    );
    assert!(
        routes
            .iter()
            .all(|route| route.kind == ProjectAssetKind::Texture)
    );
    let nanocom = ROUTES
        .iter()
        .find(|route| route.destination == "nanocom/message/npc.png")
        .unwrap();
    assert_eq!(nanocom.legacy_name, "nanocom_message_npc");
    assert_eq!(
        nanocom.source,
        "ui/gameplay/nanocom/nanocom_message_npc.png"
    );
}
