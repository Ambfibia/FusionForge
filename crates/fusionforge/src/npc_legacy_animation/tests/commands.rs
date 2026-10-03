use super::*;

#[test]
fn stale_file_id_null_event_pointer_preserves_exact_provenance() {
    use fusionforge::UnityValue::{Array, Float, Object, Pointer};

    let body = Object(BTreeMap::from([(
        "m_Events".to_string(),
        Array(vec![Object(BTreeMap::from([
            ("time".to_string(), Float(0.5)),
            (
                "objectReferenceParameter".to_string(),
                Pointer(fusionforge::Pointer {
                    source_asset: 3,
                    file_id: 7,
                    path_id: 0,
                }),
            ),
        ]))]),
    )]));
    let mut errors = Vec::new();
    let events = decode_animation_events(&body, &mut errors);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(events.len(), 1);
    assert!(events[0]["objectParameter"].is_null());
    assert_eq!(
        events[0]["objectParameterProvenance"],
        json!({
            "presence": "serialized-pointer",
            "sourceAssetIndex": 3,
            "fileId": 7,
            "pathId": 0,
            "interpretation": "null-path-id",
        })
    );
}

#[test]
fn non_null_event_pointer_remains_typed_and_unresolved() {
    use fusionforge::UnityValue::{Array, Float, Object, Pointer};

    let body = Object(BTreeMap::from([(
        "m_Events".to_string(),
        Array(vec![Object(BTreeMap::from([
            ("time".to_string(), Float(0.5)),
            (
                "objectReferenceParameter".to_string(),
                Pointer(fusionforge::Pointer {
                    source_asset: 2,
                    file_id: 1,
                    path_id: 99,
                }),
            ),
        ]))]),
    )]));
    let mut errors = Vec::new();
    let events = decode_animation_events(&body, &mut errors);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0]["objectParameter"],
        json!({
            "sourceAssetIndex": 2,
            "fileId": 1,
            "pathId": 99,
        })
    );
    assert_eq!(
        events[0]["objectParameterProvenance"],
        json!({
            "presence": "serialized-pointer",
            "sourceAssetIndex": 2,
            "fileId": 1,
            "pathId": 99,
            "interpretation": "non-null-unresolved",
        })
    );
}

#[test]
fn present_nonfinite_event_float_is_not_replaced_with_zero() {
    use fusionforge::UnityValue::{Array, Float, Object};

    let body = Object(BTreeMap::from([(
        "m_Events".to_string(),
        Array(vec![Object(BTreeMap::from([
            ("time".to_string(), Float(0.5)),
            ("floatParameter".to_string(), Float(f64::NAN)),
        ]))]),
    )]));
    let mut errors = Vec::new();
    let events = decode_animation_events(&body, &mut errors);

    assert!(events.is_empty(), "invalid event must not be defaulted");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("m_Events[0]"));
    assert!(errors[0].contains("invalid floatParameter"));
}

#[test]
fn exact_legacy_larry_end_event_normalizes_only_its_proven_stale_float() {
    use fusionforge::UnityValue::{Array, Float, Int, Object, String as UnityString};

    let body = Object(BTreeMap::from([
        ("m_Name".to_string(), UnityString("walk".to_string())),
        (
            "m_Events".to_string(),
            Array(vec![Object(BTreeMap::from([
                ("time".to_string(), Float(3.0)),
                ("functionName".to_string(), UnityString("end".to_string())),
                ("data".to_string(), UnityString(String::new())),
                (
                    "floatParameter".to_string(),
                    Float(f64::from(f32::from_bits(0xffff_ff00))),
                ),
                ("messageOptions".to_string(), Int(1_852_788_223)),
            ]))]),
        ),
    ]));
    let mut errors = Vec::new();
    let events = decode_animation_events_for_clip(
        "CustomAssetBundle-574ca8c2fed89492582a3fbd49965c12",
        154,
        "walk",
        &body,
        &mut errors,
    );

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["floatParameter"], json!(0.0));
    assert_eq!(
        events[0]["floatParameterProvenance"],
        json!({
            "sourceAssetFormat": 6,
            "rawFloat32Bits": "0xffffff00",
            "interpretation": "legacy-unused-nonfinite-normalized-to-zero",
        })
    );
    validate_exact_event_float_parameter(&events[0], 0, "AnimationClip walk")
        .expect("typed legacy provenance");
}

#[test]
fn live_legacy_event_null_pointers_keep_stale_file_ids() {
    let repo_root = crate::repository_root()
        .parent()
        .expect("workspace root");
    let extracted = repo_root
        .join("builds")
        .join("retrobution-20260613.ffclient")
        .join("cache")
        .join("extracted-bundles");
    let cases = [(
        extracted
            .join("3a5031a10b0c75f4")
            .join("CustomAssetBundle-Retro_shared_part2"),
        vec![1618_i64, 1222_i64, 1358_i64, 612_i64],
    )];
    for (asset_path, path_ids) in cases {
        if !asset_path.is_file() {
            eprintln!(
                "skipping live event pointer fixture: not found at {}",
                asset_path.display()
            );
            continue;
        }
        let env = fusionforge::UnityEnvironment::from_paths(&[asset_path]);
        let asset = env.assets.first().expect("legacy shared asset");
        for path_id in path_ids {
            let info = asset.objects.get(&path_id).expect("AnimationClip path id");
            assert_eq!(asset.object_type_name(info), "AnimationClip");
            let body = asset.read_object(0, info).expect("AnimationClip body");
            let raw_events = fusionforge::value_array(body.get("m_Events"));
            assert!(!raw_events.is_empty(), "clip {path_id} has no events");
            let mut errors = Vec::new();
            let events = decode_animation_events(&body, &mut errors);
            assert!(errors.is_empty(), "clip {path_id}: {errors:?}");
            assert_eq!(events.len(), raw_events.len());
            let mut stale_null_count = 0_usize;
            for (event_index, (raw, decoded)) in raw_events.iter().zip(&events).enumerate() {
                match event_field(
                    raw,
                    &["objectReferenceParameter", "m_ObjectReferenceParameter"],
                ) {
                    None => {
                        assert!(decoded["objectParameter"].is_null());
                        assert_eq!(
                            decoded["objectParameterProvenance"],
                            json!({
                                "presence": "missing",
                                "interpretation": "missing",
                            })
                        );
                    }
                    Some(fusionforge::UnityValue::Pointer(pointer)) if pointer.is_null() => {
                        stale_null_count += usize::from(pointer.file_id != 0);
                        assert!(decoded["objectParameter"].is_null());
                        assert_eq!(
                            decoded["objectParameterProvenance"],
                            json!({
                                "presence": "serialized-pointer",
                                "sourceAssetIndex": pointer.source_asset,
                                "fileId": pointer.file_id,
                                "pathId": pointer.path_id,
                                "interpretation": "null-path-id",
                            }),
                            "clip {path_id} event {event_index}"
                        );
                    }
                    Some(fusionforge::UnityValue::Pointer(pointer)) => {
                        assert_eq!(
                            decoded["objectParameter"],
                            json!({
                                "sourceAssetIndex": pointer.source_asset,
                                "fileId": pointer.file_id,
                                "pathId": pointer.path_id,
                            })
                        );
                        assert_eq!(
                            decoded["objectParameterProvenance"]["interpretation"],
                            json!("non-null-unresolved")
                        );
                    }
                    Some(other) => panic!(
                        "clip {path_id} event {event_index} has non-pointer object parameter {other:?}"
                    ),
                }
            }
            assert!(
                stale_null_count > 0,
                "clip {path_id} must preserve at least one stale non-zero fileId"
            );
            let clip = decode_animation_clip(&asset.name, path_id, &body, Vec::new()).preview;
            assert!(
                clip.get("decodeWarnings").is_none(),
                "clip {path_id}: {:?}",
                clip.get("decodeWarnings")
            );
        }
    }
}

#[test]
fn event_after_last_trs_key_extends_effective_duration_with_provenance() {
    use fusionforge::UnityValue::{Array, Float, Object, String as UnityString};

    let vec3 = Object(BTreeMap::from([
        ("x".to_string(), Float(0.0)),
        ("y".to_string(), Float(0.0)),
        ("z".to_string(), Float(0.0)),
    ]));
    let curve = Object(BTreeMap::from([
        ("path".to_string(), UnityString("Bip01".to_string())),
        (
            "curve".to_string(),
            Object(BTreeMap::from([(
                "m_Curve".to_string(),
                Array(vec![Object(BTreeMap::from([
                    ("time".to_string(), Float(0.716_669_082_641_601_6)),
                    ("value".to_string(), vec3),
                ]))]),
            )])),
        ),
    ]));
    let body = Object(BTreeMap::from([
        ("m_Name".to_string(), UnityString("melee1".to_string())),
        ("m_PositionCurves".to_string(), Array(vec![curve])),
        ("m_RotationCurves".to_string(), Array(Vec::new())),
        ("m_CompressedRotationCurves".to_string(), Array(Vec::new())),
        ("m_EulerCurves".to_string(), Array(Vec::new())),
        ("m_ScaleCurves".to_string(), Array(Vec::new())),
        ("m_FloatCurves".to_string(), Array(Vec::new())),
        ("m_PPtrCurves".to_string(), Array(Vec::new())),
        (
            "m_Events".to_string(),
            Array(vec![Object(BTreeMap::from([
                ("time".to_string(), Float(0.800_000_011_920_929)),
                ("functionName".to_string(), UnityString("end".to_string())),
            ]))]),
        ),
    ]));

    let clip = decode_animation_clip("Character_IceKing", 1009, &body, Vec::new()).preview;
    assert!(clip["declaredDuration"].is_null());
    assert_eq!(clip["keyedDuration"], json!(0.716_669_082_641_601_6));
    assert_eq!(clip["eventDuration"], json!(0.800_000_011_920_929));
    assert_eq!(clip["duration"], json!(0.800_000_011_920_929));
    let hierarchy = json!({ "nodes": [{ "path": "npc_simon/Bip01" }] });
    validate_exact_animation_source(&hierarchy, &[clip])
        .expect("event-extended exact clip duration");
}
