use super::*;

#[test]
fn superseded_scripts_are_not_published_as_runtime_records() {
    assert!(superseded_script("Combiner").is_some());
    assert!(superseded_script("DongColorSetup").is_some());
    assert!(superseded_script("MapAttributeTable").is_some());
    assert!(superseded_script("EpRingTrigger").is_none());
    assert!(superseded_script("BillboardNode").is_none());
}

#[test]
fn builtin_terrain_payload_is_not_a_false_unresolved_script() {
    let record: ExportBehaviour = serde_json::from_value(serde_json::json!({
        "node": "BuildPlayer-Map_03_05#8576",
        "worldMatrix": [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0]
        ],
        "type": "MonoBehaviour",
        "id": "BuildPlayer-Map_03_05#8577",
        "enabled": true,
        "script": null,
        "fields": {
            "m_TerrainData": { "fileId": 2, "pathId": 272 },
            "m_HeightmapPixelError": 20.0,
            "m_HeightmapMaximumLOD": 0,
            "m_DetailObjectDistance": 80.0,
            "m_TreeDistance": 5000.0
        }
    }))
    .unwrap();
    assert!(is_builtin_terrain_component(&record));
}

#[test]
fn billboard_owner_includes_models_on_descendant_nodes() {
    let parents = BTreeMap::from([
        ("billboard-mesh".to_owned(), "billboard-root".to_owned()),
        ("billboard-root".to_owned(), "tile-root".to_owned()),
    ]);
    let direct = BTreeMap::from([(
        "billboard-mesh".to_owned(),
        vec!["static-map-0042".to_owned()],
    )]);
    let subtrees = models_in_node_subtrees(&parents, &direct);
    assert_eq!(
        subtrees.get("billboard-root").unwrap(),
        &["static-map-0042"]
    );
    assert_eq!(subtrees.get("tile-root").unwrap(), &["static-map-0042"]);
}

#[test]
fn ownership_proof_accepts_only_the_windows_crlf_materialization() {
    let canonical = b"{\n  \"schema\": \"fixture\"\n}\n";
    let crlf = canonical
        .iter()
        .flat_map(|byte| {
            if *byte == b'\n' {
                vec![b'\r', b'\n']
            } else {
                vec![*byte]
            }
        })
        .collect::<Vec<_>>();
    let hash = hash_bytes(canonical);
    assert!(matches_owned_bytes(&crlf, canonical.len() as u64, &hash));
    assert!(!matches_owned_bytes(
        b"{\rBROKEN}",
        canonical.len() as u64,
        &hash
    ));
}

#[test]
fn vector_and_number_conversion_is_total() {
    assert_eq!(
        vector_json(Some(&serde_json::json!({ "x": 1.0, "y": 2.0, "z": 3.0 }))),
        serde_json::json!([1.0, 2.0, 3.0])
    );
    assert_eq!(vector_json(None), serde_json::json!([0.0, 0.0, 0.0]));
    assert_eq!(
        number_json(Some(&serde_json::json!(true))),
        serde_json::json!(1)
    );
    assert_eq!(
        number_json(Some(&serde_json::json!(2.5))),
        serde_json::json!(2.5)
    );
    assert_eq!(number_json(None), serde_json::json!(0));
    assert_eq!(bool_value(Some(&serde_json::json!(true))), Some(true));
    assert_eq!(bool_value(Some(&serde_json::json!(1))), Some(true));
    assert_eq!(bool_value(Some(&serde_json::json!(0))), Some(false));
    assert_eq!(bool_value(Some(&serde_json::json!("1"))), None);
}

#[test]
fn legacy_world_clip_decodes_packed_quaternions_and_float_curves() {
    let clip = ExportAnimationClip {
        id: "CustomAssetBundle-52625066c401043eda0a3d5088cda126#100".to_owned(),
        name: "nif-default".to_owned(),
        fields: serde_json::from_value(serde_json::json!({
            "m_SampleRate": 30.0,
            "m_PositionCurves": [],
            "m_RotationCurves": [],
            "m_ScaleCurves": [],
            "m_CompressedRotationCurves": [{
                "m_Path": "dt_mtsm_fence_ef03_10_a",
                "m_PreInfinity": 2,
                "m_PostInfinity": 2,
                "m_Times": {
                    "m_NumItems": 2,
                    "m_BitSize": 2,
                    "m_Data": {"base64": "DA==", "bytes": 1}
                },
                "m_Slopes": {
                    "m_NumItems": 8,
                    "m_BitSize": 6,
                    "m_Range": 0.0,
                    "m_Start": 0.0,
                    "m_Data": {"base64": "AAAAAAAA", "bytes": 6}
                },
                "m_Values": {
                    "m_NumItems": 2,
                    "m_Data": {"base64": "//fff//3338=", "bytes": 8}
                }
            }],
            "m_FloatCurves": [{
                "attribute": "_Emission.r",
                "classID": 21,
                "path": "dt_mtsm_fence_ef03_10_a/dt_mtsm_fence_ef03_10_a:0",
                "curve": {
                    "m_PreInfinity": 1,
                    "m_PostInfinity": 1,
                    "m_Curve": [
                        {"time": 0.0, "value": 0.0, "inSlope": 0.5, "outSlope": 0.5},
                        {"time": 2.0, "value": 1.0, "inSlope": 0.5, "outSlope": 0.5}
                    ]
                }
            }],
            "m_Events": []
        }))
        .unwrap(),
    };

    let decoded = decode_world_animation_clip(&clip).unwrap();
    assert_eq!(decoded["duration"], serde_json::json!(2.0));
    assert_eq!(decoded["looped"], serde_json::json!(true));
    assert_eq!(decoded["channels"][0]["sourceEncoding"], "compressed");
    assert_eq!(
        decoded["channels"][0]["times"],
        serde_json::json!([0.0, 0.03])
    );
    assert_eq!(
        decoded["channels"][0]["values"].as_array().unwrap().len(),
        2
    );
    assert_eq!(decoded["floatCurves"][0]["attribute"], "_Emission.r");
    assert_eq!(decoded["floatCurves"][0]["classId"], 21);
}

#[test]
fn animation_targets_preserve_a_billboard_pivot_below_an_animated_node() {
    let identity = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let node = |name: &str, parent: Option<&str>| HierarchyNode {
        name: name.to_owned(),
        parent: parent.map(str::to_owned),
        local_translation: [0.0, 0.0, 0.0],
        local_rotation: [0.0, 0.0, 0.0, 1.0],
        local_scale: [1.0, 1.0, 1.0],
        world_matrix: identity,
    };
    let hierarchy = HierarchyIndex {
        direct_model_ids_by_node: BTreeMap::from([(
            "mesh-node".to_owned(),
            vec!["static-map-0042".to_owned()],
        )]),
        nodes: BTreeMap::from([
            ("animation-owner".to_owned(), node("owner", None)),
            (
                "animated-node".to_owned(),
                node("animated", Some("animation-owner")),
            ),
            (
                "billboard-node".to_owned(),
                node("billboard", Some("animated-node")),
            ),
            ("mesh-node".to_owned(), node("mesh", Some("billboard-node"))),
        ]),
        children_by_node: BTreeMap::from([
            (
                "animation-owner".to_owned(),
                vec!["animated-node".to_owned()],
            ),
            (
                "animated-node".to_owned(),
                vec!["billboard-node".to_owned()],
            ),
            ("billboard-node".to_owned(), vec!["mesh-node".to_owned()]),
        ]),
        ..HierarchyIndex::default()
    };
    let targets = build_animation_targets(
        "animation-owner",
        &BTreeSet::from(["animated".to_owned()]),
        &BTreeSet::from(["animated/billboard".to_owned()]),
        &["static-map-0042".to_owned()],
        &hierarchy,
    )
    .unwrap();

    assert_eq!(targets.len(), 3);
    assert_eq!(targets[0]["path"], "");
    assert_eq!(targets[1]["path"], "animated");
    assert_eq!(targets[1]["animated"], true);
    assert_eq!(targets[2]["path"], "animated/billboard");
    assert_eq!(targets[2]["parentPath"], "animated");
    assert_eq!(targets[2]["animated"], false);
    assert_eq!(targets[2]["models"], serde_json::json!(["static-map-0042"]));
}

#[test]
fn every_trigger_class_maps_to_a_lowercase_kind() {
    for (class_name, expected) in [
        ("EpRingTrigger", "ring"),
        ("EpJumppadTrigger", "jumppad"),
        ("EpPlatformTrigger", "platform"),
        ("EpZiplineTrigger", "zipline"),
        ("EpSlopeTrigger", "slope"),
    ] {
        assert!(class_name.starts_with("Ep") && class_name.ends_with("Trigger"));
        let kind = class_name
            .trim_start_matches("Ep")
            .trim_end_matches("Trigger")
            .to_ascii_lowercase();
        assert_eq!(kind, expected);
    }
}
