use super::*;

#[test]
fn exact_effect_set_includes_tutorial_scenes_and_all_tutorial_weapons() {
    assert_eq!(
        RETROBUTION_TUTORIAL_EFFECT_IDS,
        [
            4, 10, 38, 60, 366, 372, 527, 528, 529, 653, 668, 705, 723, 734, 736, 739, 740,
            741, 742, 750, 751, 757, 767, 771, 772, 774, 778, 809, 812, 813, 817, 865, 866
        ]
    );
    assert_eq!(
        RETROBUTION_TUTORIAL_BULLET_TYPES,
        [
            5, 6, 9, 13, 17, 51, 66, 68, 69, 72, 76, 77, 106, 113, 115, 118, 121, 131, 133,
            134, 135, 136, 145, 146, 147, 148, 149, 150, 151, 152, 155, 156, 158, 160, 161,
            162, 163, 164, 165,
        ]
    );
    assert_eq!(
        RETROBUTION_FUSION_ACTOR_EFFECT_IDS,
        [
            530, 537, 538, 542, 543, 544, 545, 546, 547, 548, 549, 550, 551, 552, 553, 554,
            555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 565, 566, 567, 568, 569, 570,
            571, 572, 573, 574, 575, 576, 577, 578, 579, 580, 581, 582, 583, 584, 592, 593,
            600, 601, 602, 603, 604, 605, 606, 607, 608, 609, 610, 611, 612, 613, 614, 615,
            616, 617, 618, 619, 620, 621, 622, 623, 624, 625, 626, 627, 628, 629, 630, 631,
            633, 634, 635, 636, 637, 638, 639, 640, 641, 642, 643, 644, 645, 647, 650
        ]
    );
    assert_eq!(
        RETROBUTION_CHARACTER_ACTOR_EFFECT_IDS,
        [
            3, 21, 50, 425, 503, 688, 689, 690, 691, 692, 693, 694, 695, 697, 698, 699, 700,
            726, 737, 763, 764, 765, 768, 769, 833, 834
        ]
    );
    assert_eq!(
        RETROBUTION_TUTORIAL_PROJECTILE_EFFECT_IDS,
        [
            15, 17, 31, 100, 321, 378, 379, 391, 718, 721, 729, 747, 749, 754, 755, 787, 788,
            789, 790, 791, 792, 793, 794, 795, 796, 797, 798, 808,
        ]
    );
    assert_eq!(
        RETROBUTION_WEAPON_EFFECT_IDS,
        [
            55, 405, 504, 667, 720, 724, 752, 753, 756, 770, 773, 775, 776, 777, 779, 780, 781,
            782, 783, 784, 810, 818, 819,
        ]
    );
    assert_eq!(
        RETROBUTION_WORLD_EP_EFFECT_IDS,
        [461, 464, 465, 466, 533, 534, 594]
    );
    assert_eq!(RETROBUTION_PLAYER_STATUS_EFFECT_IDS, [376, 385, 804]);
    assert_eq!(RETROBUTION_NPC_WARP_EFFECT_IDS, [394]);
    assert_eq!(
        RETROBUTION_NPC_GAME_ICON_EFFECT_IDS,
        [
            66, 395, 446, 672, 673, 674, 675, 676, 677, 678, 679, 680, 681, 682, 683, 685, 811,
            824, 825, 826,
        ]
    );
    assert_eq!(
        effect_route(372),
        "prefabs/particle/effectscripts/es[372].prefab"
    );
}

#[test]
fn effect_installer_preserves_separately_published_render_payloads() {
    assert!(is_preserved_effect_payload_path(
        "map/shared/effects/models/es734/T_pistol.glb"
    ));
    assert!(is_preserved_effect_payload_path(
        "map/shared/effects/textures/fusionstar.png"
    ));
    assert!(is_preserved_effect_payload_path(
        "map/shared/effects/blackhole.nif-animation.json"
    ));
    assert!(!is_preserved_effect_payload_path(
        "map/shared/effects/es734.closure.json"
    ));
    assert!(!is_preserved_effect_payload_path(
        "map/shared/projectiles/catalog.json"
    ));
    assert!(is_preserved_effect_payload_path(
        "map/shared/projectiles/effects/models/es718/sonic_ware_ta_m01.glb"
    ));
}

#[test]
fn canonical_json_sorts_nested_object_keys() {
    let value = serde_json::json!({"z": {"b": 2, "a": 1}, "a": 0});
    let bytes = serde_json::to_vec(&canonical_json(&value)).unwrap();
    assert_eq!(bytes, br#"{"a":0,"z":{"a":1,"b":2}}"#);
}

#[test]
fn rejects_unsafe_publication_paths() {
    for path in ["../escape", "effects\\x", "/absolute", "effects//x", "C:x"] {
        assert!(validate_relative(path).is_err(), "{path}");
    }
    assert!(validate_relative("effects/es372.closure.json").is_ok());
}

#[test]
fn container_routes_preserve_external_dependency_ownership() {
    let container = serde_json::json!([
        [
            "prefabs/particle/effectscripts/es[395].prefab",
            {"asset": {"fileId": 1, "pathId": 395}}
        ],
        [
            "prefabs/particle/effectscripts/es[446].prefab",
            {"asset": {"fileId": 0, "pathId": 446}}
        ]
    ]);
    let routes = collect_container_routes(&container, PRIMARY_EFFECTS_ASSET).unwrap();

    assert_eq!(
        routes["prefabs/particle/effectscripts/es[395].prefab"],
        [UnityObjectKey {
            asset: EFFECTS_DEPENDENCY_B4.to_owned(),
            path_id: 395,
        }]
    );
    assert_eq!(
        routes["prefabs/particle/effectscripts/es[446].prefab"],
        [UnityObjectKey {
            asset: PRIMARY_EFFECTS_ASSET.to_owned(),
            path_id: 446,
        }]
    );
}

#[test]
fn parses_exact_typed_bullet_fields_without_discarding_row() {
    let row = serde_json::json!({
        "m_iCancelScript": 9,
        "m_iFireScript": 10,
        "m_iParticleScript": 11,
        "m_iSuccScript": 12,
        "m_fCancelModelScale": 0.5,
        "m_fCurveHeight": 0.75,
        "m_fFireModelScale": 1.0,
        "m_fBulletModelScale": 2.0,
        "m_fSuccModelScale": 3.0,
        "m_fHideTime": 0.25,
        "m_fMaxTimer": 1.5,
        "m_strFireLink": "Bip01",
        "m_strSuccLink": "Bip02",
        "m_strSuccSound": "hit"
    });
    let parsed = parse_bullet_parameters(&row, 76).unwrap();
    assert_eq!(parsed.fire_script, 10);
    assert_eq!(parsed.cancel_script, 9);
    assert_eq!(parsed.particle_script, 11);
    assert_eq!(parsed.success_script, 12);
    assert_eq!(parsed.cancel_model_scale, 0.5);
    assert_eq!(parsed.curve_height, 0.75);
    assert_eq!(parsed.maximum_time_seconds, 1.5);
    assert_eq!(parsed.success_sound, "hit");
}

#[test]
fn closure_rejects_unresolved_external_pointer() {
    let object = DumpObject {
        asset: "Effects".to_owned(),
        path_id: 5,
        type_id: 1,
        class_id: 1,
        object_type: "GameObject".to_owned(),
        name: "root".to_owned(),
        value: serde_json::json!({"m_Component": {"fileId": 2, "pathId": 99}}),
    };
    let index = BTreeMap::from([(
        UnityObjectKey {
            asset: "Effects".to_owned(),
            path_id: 5,
        },
        &object,
    )]);
    let error = build_closure(
        Some(10),
        "route",
        "Effects",
        5,
        &index,
        "bundle",
        "dump",
        &[],
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("dependency closure is incomplete"));
}
