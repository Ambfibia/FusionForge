use super::*;

pub(super) fn plain_test_curve(
    path: String,
    value: Option<fusionforge::UnityValue>,
) -> fusionforge::UnityValue {
    use fusionforge::UnityValue::{Array, Float, Object, String as UnityString};

    let keys = value
        .map(|value| {
            vec![Object(BTreeMap::from([
                ("time".to_string(), Float(0.0)),
                ("value".to_string(), value),
            ]))]
        })
        .unwrap_or_default();
    Object(BTreeMap::from([
        ("path".to_string(), UnityString(path)),
        (
            "curve".to_string(),
            Object(BTreeMap::from([("m_Curve".to_string(), Array(keys))])),
        ),
    ]))
}

pub(super) fn simon_empty_binding_regression_fixture() -> (JsonValue, JsonValue) {
    use fusionforge::UnityValue::{Array, Float, Object, String as UnityString};

    let vec3 = || {
        Object(BTreeMap::from([
            ("x".to_string(), Float(0.0)),
            ("y".to_string(), Float(0.0)),
            ("z".to_string(), Float(0.0)),
        ]))
    };
    let quaternion = || {
        Object(BTreeMap::from([
            ("x".to_string(), Float(0.0)),
            ("y".to_string(), Float(0.0)),
            ("z".to_string(), Float(0.0)),
            ("w".to_string(), Float(1.0)),
        ]))
    };
    let path = |index: usize| {
        if index == 0 {
            "Bip01".to_string()
        } else {
            format!("Bip01/J{index}")
        }
    };
    let positions = (0..94)
        .map(|index| plain_test_curve(path(index), (index != 0).then(vec3)))
        .collect();
    let rotations = (0..94)
        .map(|index| plain_test_curve(path(index), (index >= 4).then(quaternion)))
        .collect();
    let scales = (0..94)
        .map(|index| plain_test_curve(path(index), (index >= 12).then(vec3)))
        .collect();
    let body = Object(BTreeMap::from([
        ("m_Name".to_string(), UnityString("death".to_string())),
        ("m_PositionCurves".to_string(), Array(positions)),
        ("m_RotationCurves".to_string(), Array(rotations)),
        ("m_CompressedRotationCurves".to_string(), Array(Vec::new())),
        ("m_EulerCurves".to_string(), Array(Vec::new())),
        ("m_ScaleCurves".to_string(), Array(scales)),
        ("m_FloatCurves".to_string(), Array(Vec::new())),
        ("m_PPtrCurves".to_string(), Array(Vec::new())),
        ("m_Events".to_string(), Array(Vec::new())),
    ]));
    let clip = decode_animation_clip("Character_IceKing", 1011, &body, Vec::new()).preview;
    let nodes = std::iter::once(json!({ "path": "npc_simon" }))
        .chain(std::iter::once(json!({ "path": "npc_simon/Bip01" })))
        .chain((1..94).map(|index| {
            json!({
                "path": format!("npc_simon/Bip01/J{index}")
            })
        }))
        .collect::<Vec<_>>();
    (clip, json!({ "nodes": nodes }))
}

pub(super) fn constant_test_curve(
    path: &str,
    value: fusionforge::UnityValue,
    times: &[f64],
) -> fusionforge::UnityValue {
    use fusionforge::UnityValue::{Array, Float, Object, String as UnityString};

    let keys = times
        .iter()
        .map(|time| {
            Object(BTreeMap::from([
                ("time".to_string(), Float(*time)),
                ("value".to_string(), value.clone()),
            ]))
        })
        .collect();
    Object(BTreeMap::from([
        ("path".to_string(), UnityString(path.to_string())),
        (
            "curve".to_string(),
            Object(BTreeMap::from([("m_Curve".to_string(), Array(keys))])),
        ),
    ]))
}

pub(super) fn constant_vec3(x: f64, y: f64, z: f64) -> fusionforge::UnityValue {
    use fusionforge::UnityValue::{Float, Object};

    Object(BTreeMap::from([
        ("x".to_string(), Float(x)),
        ("y".to_string(), Float(y)),
        ("z".to_string(), Float(z)),
    ]))
}

pub(super) fn exact_constant_sibling_fixture() -> (Vec<RawAnimationClip>, JsonValue) {
    let target = RawAnimationClip {
        asset_name: "synthetic-shared".to_string(),
        path_id: 100,
        body: constant_test_clip(
            "stand1",
            vec![
                constant_test_curve("Bone00", constant_vec3(0.0, 0.0, 0.0), &[0.0, 4.0]),
                constant_test_curve(
                    "Bone00",
                    constant_vec3(0.0, -0.75, 0.0),
                    &[0.0, 0.033_333_335_071_802_14],
                ),
            ],
        ),
    };
    let sibling = RawAnimationClip {
        asset_name: "synthetic-shared".to_string(),
        path_id: 101,
        body: constant_test_clip(
            "stand2",
            vec![constant_test_curve(
                "Bone00",
                constant_vec3(0.0, -0.75, 0.0),
                &[0.0, 1.0, 2.0, 3.0],
            )],
        ),
    };
    (
        vec![target, sibling],
        json!({ "nodes": [{ "path": "model/Bone00" }] }),
    )
}

#[test]
fn simon_death_preserves_17_ordered_empty_trs_bindings() {
    let (clip, hierarchy) = simon_empty_binding_regression_fixture();
    assert_eq!(clip["curveCounts"]["position"], json!(94));
    assert_eq!(clip["curveCounts"]["rotation"], json!(94));
    assert_eq!(clip["curveCounts"]["scale"], json!(94));
    assert_eq!(
        clip["animationData"]["translations"]
            .as_array()
            .unwrap()
            .len(),
        93
    );
    assert_eq!(
        clip["animationData"]["rotations"].as_array().unwrap().len(),
        90
    );
    assert_eq!(
        clip["animationData"]["scales"].as_array().unwrap().len(),
        82
    );

    let empty = clip["animationData"]["emptyTrsBindings"]
        .as_array()
        .expect("typed empty bindings");
    assert_eq!(empty.len(), 17);
    assert_eq!(
        empty
            .iter()
            .map(|binding| (
                binding["kind"].as_str().unwrap(),
                binding["sourceEncoding"].as_str().unwrap(),
                binding["sourceIndex"].as_u64().unwrap(),
            ))
            .collect::<Vec<_>>(),
        std::iter::once(("translation", "plain", 0))
            .chain((0..4).map(|index| ("rotation", "plain", index)))
            .chain((0..12).map(|index| ("scale", "plain", index)))
            .collect::<Vec<_>>()
    );
    validate_exact_animation_source(&hierarchy, &[clip]).expect("lossless exact source");
}

#[test]
fn identical_duplicate_curve_is_preserved_as_exact_metadata() {
    let (clip, hierarchy) = duplicate_translation_regression_fixture(false);
    assert!(clip.get("decodeWarnings").is_none());
    assert_eq!(
        clip["animationData"]["translations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let duplicate = &clip["animationData"]["duplicateTrsBindings"][0];
    assert_eq!(duplicate["kind"], json!("translation"));
    assert_eq!(duplicate["sourceEncoding"], json!("plain"));
    assert_eq!(duplicate["sourceIndex"], json!(1));
    assert_eq!(duplicate["relation"], json!("identical"));
    assert_eq!(duplicate["canonicalTrackIndex"], json!(0));
    assert_eq!(duplicate["canonicalSourceEncoding"], json!("plain"));
    assert_eq!(duplicate["canonicalSourceIndex"], json!(0));
    assert_eq!(
        duplicate["keys"],
        clip["animationData"]["translations"][0]["keys"]
    );
    validate_exact_animation_source(&hierarchy, &[clip])
        .expect("identical duplicate is lossless");
}

#[test]
fn conflicting_constant_curve_recovers_from_one_exact_sibling() {
    let (raw_clips, hierarchy) = exact_constant_sibling_fixture();
    let (animations, plans) = decode_constant_sibling_fixture(&raw_clips);

    assert_eq!(plans.len(), 1);
    let recovery = &animations[0]["animationData"]["curveRecoveries"][0];
    assert_eq!(recovery["kind"], json!("translation"));
    assert_eq!(recovery["path"], json!("Bone00"));
    assert_eq!(recovery["source"]["sourceIndices"], json!([0, 1]));
    assert_eq!(recovery["canonical"]["sourceIndex"], json!(1));
    assert_eq!(recovery["rejected"][0]["sourceIndex"], json!(0));
    assert_eq!(recovery["reference"]["pathId"], json!(101));
    assert_eq!(recovery["reference"]["clipName"], json!("stand2"));
    assert!(animations[0].get("decodeWarnings").is_none());
    validate_exact_animation_source(&hierarchy, &animations)
        .expect("single exact constant sibling is a lossless authority");
}

#[test]
fn constant_curve_recoveries_allow_interleaved_target_paths() {
    use fusionforge::UnityValue::Array;
    let (mut clips, _) = exact_constant_sibling_fixture();
    clips[0].body.get_mut("m_PositionCurves").map(|curves| {
        *curves = Array(vec![
            constant_test_curve("Bone00", constant_vec3(0.0, 0.0, 0.0), &[0.0, 4.0]),
            constant_test_curve("Bone01", constant_vec3(0.0, 0.0, 0.0), &[0.0, 4.0]),
            constant_test_curve("Bone00", constant_vec3(0.0, -0.75, 0.0), &[0.0, 4.0]),
            constant_test_curve("Bone01", constant_vec3(0.0, -0.5, 0.0), &[0.0, 4.0]),
        ]);
    });
    clips[1].body.get_mut("m_PositionCurves").map(|curves| {
        *curves = Array(vec![
            constant_test_curve("Bone00", constant_vec3(0.0, -0.75, 0.0), &[0.0, 3.0]),
            constant_test_curve("Bone01", constant_vec3(0.0, -0.5, 0.0), &[0.0, 3.0]),
        ]);
    });
    let hierarchy = json!({"nodes": [{"path": "model/Bone00"}, {"path": "model/Bone01"}]});
    let (mut animations, plans) = decode_constant_sibling_fixture(&clips);
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0].source_indices, [0, 2]);
    assert_eq!(plans[1].source_indices, [1, 3]);
    validate_exact_animation_source(&hierarchy, &animations).unwrap();
    animations[0]["animationData"]["curveRecoveries"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert!(validate_exact_animation_source(&hierarchy, &animations)
        .unwrap_err()
        .contains("source order"));
}

#[test]
fn conflicting_constant_curve_stays_blocked_with_two_matching_siblings() {
    let (mut raw_clips, hierarchy) = exact_constant_sibling_fixture();
    let mut second_sibling = raw_clips[1].clone();
    second_sibling.path_id = 102;
    if let Some(name) = second_sibling.body.get_mut("m_Name") {
        *name = fusionforge::UnityValue::String("stand3".to_string());
    }
    raw_clips.push(second_sibling);

    let catalog = build_constant_curve_recovery_catalog(&raw_clips);
    let plans = recover_conflicting_constant_curves(&raw_clips[0], &catalog);
    assert!(
        plans.is_empty(),
        "ambiguous sibling authority must not recover"
    );
    let target = decode_animation_clip(
        &raw_clips[0].asset_name,
        raw_clips[0].path_id,
        &raw_clips[0].body,
        Vec::new(),
    )
    .preview;
    let error = validate_exact_animation_source(&hierarchy, &[target]).unwrap_err();
    assert!(error.contains("undecoded source curves"), "{error}");
    assert!(error.contains("conflicting"), "{error}");
}

#[test]
fn duplicate_curve_metadata_rejects_tampered_keys() {
    let (mut clip, hierarchy) = duplicate_translation_regression_fixture(false);
    clip["animationData"]["duplicateTrsBindings"][0]["keys"][0]["value"][0] = json!(-999.0);
    let error = validate_exact_animation_source(&hierarchy, &[clip]).unwrap_err();
    assert!(
        error.contains("does not exactly match its canonical track"),
        "{error}"
    );
}

#[test]
fn partially_invalid_curve_is_not_published_with_one_key_missing() {
    use fusionforge::UnityValue::{Array, Float, Object, String as UnityString};

    let vec3 = || {
        Object(BTreeMap::from([
            ("x".to_string(), Float(0.0)),
            ("y".to_string(), Float(0.0)),
            ("z".to_string(), Float(0.0)),
        ]))
    };
    let curve = Object(BTreeMap::from([
        ("path".to_string(), UnityString("Bip01".to_string())),
        (
            "curve".to_string(),
            Object(BTreeMap::from([(
                "m_Curve".to_string(),
                Array(vec![
                    Object(BTreeMap::from([
                        ("time".to_string(), Float(0.0)),
                        ("value".to_string(), vec3()),
                    ])),
                    Object(BTreeMap::from([("time".to_string(), Float(1.0))])),
                ]),
            )])),
        ),
    ]));
    let body = Object(BTreeMap::from([
        ("m_Name".to_string(), UnityString("broken".to_string())),
        ("m_PositionCurves".to_string(), Array(vec![curve])),
        ("m_RotationCurves".to_string(), Array(Vec::new())),
        ("m_CompressedRotationCurves".to_string(), Array(Vec::new())),
        ("m_EulerCurves".to_string(), Array(Vec::new())),
        ("m_ScaleCurves".to_string(), Array(Vec::new())),
        ("m_FloatCurves".to_string(), Array(Vec::new())),
        ("m_PPtrCurves".to_string(), Array(Vec::new())),
        ("m_Events".to_string(), Array(Vec::new())),
    ]));
    let clip = decode_animation_clip("broken-asset", 77, &body, Vec::new()).preview;
    assert!(clip
        .pointer("/animationData/translations")
        .and_then(JsonValue::as_array)
        .is_none_or(Vec::is_empty));
    assert!(clip["decodeWarnings"][0]
        .as_str()
        .is_some_and(|warning| warning.contains("key[1] has no finite value")));

    let hierarchy = json!({ "nodes": [{ "path": "model/Bip01" }] });
    let error = validate_exact_animation_source(&hierarchy, &[clip]).unwrap_err();
    assert!(error.contains("undecoded source curves"), "{error}");
    assert!(error.contains("key[1]"), "{error}");
}

#[test]
fn packed_quaternion_decoder_matches_rex_legacy_curve() {
    // First packed value of Character_Rex stand3/Bip01 NonAccum.
    let value = unpack_legacy_compressed_quaternion(0x25df_f7ff);
    assert!((value[0] - -0.001956947).abs() < 1.0e-6);
    assert!((value[1] - -0.000977517).abs() < 1.0e-6);
    assert!((value[2] - -0.704789834).abs() < 1.0e-6);
    assert!((value[3] - -0.709412789).abs() < 1.0e-6);
    assert!((value.iter().map(|part| part * part).sum::<f64>() - 1.0).abs() < 1.0e-9);
}

#[test]
fn compressed_skin_decoder_reconstructs_implicit_fourth_weight() {
    let (indices, weights, warning) =
        decode_compressed_skin_values(&[10, 8, 7], &[1, 2, 3, 4], 1, 8)
            .expect("compressed skin");
    assert_eq!(indices, vec![[1, 2, 3, 4]]);
    assert!((weights[0][0] - 10.0 / 31.0).abs() < 1.0e-9);
    assert!((weights[0][3] - 6.0 / 31.0).abs() < 1.0e-9);
    assert!(warning.is_none());
}

#[test]
fn exact_larry_end_pointer_is_classified_as_legacy_unused_only_for_proven_identity() {
    let event = |object_path_id| {
        json!({
            "time": 4.730000019073486,
            "functionName": "end",
            "stringParameter": "",
            "floatParameter": 1.7443726419255867e28_f64,
            "intParameter": 0,
            "messageOptions": 13_156,
            "objectParameter": {
                "sourceAssetIndex": 5,
                "fileId": 1,
                "pathId": object_path_id,
            },
            "objectParameterProvenance": {
                "presence": "serialized-pointer",
                "sourceAssetIndex": 5,
                "fileId": 1,
                "pathId": object_path_id,
                "interpretation": "non-null-unresolved",
            },
        })
    };
    let mut animations = vec![json!({
        "asset": "CustomAssetBundle-574ca8c2fed89492582a3fbd49965c12",
        "pathId": 152,
        "name": "stand2",
        "events": [event(6)],
    })];
    classify_metadata_only_sound_event_pointers(&mut animations)
        .expect("exact primary Larry event classification");
    assert_eq!(
        animations[0]["events"][0]["objectParameterProvenance"]["interpretation"],
        json!("non-null-legacy-unused")
    );

    let mut tampered = vec![json!({
        "asset": "CustomAssetBundle-574ca8c2fed89492582a3fbd49965c12",
        "pathId": 152,
        "name": "stand2",
        "events": [event(7)],
    })];
    classify_metadata_only_sound_event_pointers(&mut tampered)
        .expect("unknown pointer remains explicit");
    assert_eq!(
        tampered[0]["events"][0]["objectParameterProvenance"]["interpretation"],
        json!("non-null-unresolved")
    );
}

#[test]
fn character_root_scoring_prefers_the_requested_fusion_variant() {
    let preferred = BTreeSet::from(["mob/fusion_chowder.kfm".to_string()]);
    let fusion = CharacterRootCandidate {
        meshes: BTreeSet::new(),
        searchable_names: BTreeSet::from(["fusion_chowder".to_string(), "Box04:0".to_string()]),
    };
    let ordinary = CharacterRootCandidate {
        meshes: BTreeSet::new(),
        searchable_names: BTreeSet::from(["npc_chowder".to_string()]),
    };
    let unrelated = CharacterRootCandidate {
        meshes: BTreeSet::new(),
        searchable_names: BTreeSet::from(["npc_dracula".to_string()]),
    };

    let fusion_score = character_root_preference_score(&fusion, &preferred);
    assert!(fusion_score > character_root_preference_score(&ordinary, &preferred));
    assert!(fusion_score > character_root_preference_score(&unrelated, &preferred));
}

#[test]
fn character_root_scoring_matches_reordered_fusion_name_tokens() {
    let preferred = BTreeSet::from(["mob/fusion_mandy.kfm".to_string()]);
    let fusion = CharacterRootCandidate {
        meshes: BTreeSet::new(),
        searchable_names: BTreeSet::from(["N_MandyFusion".to_string()]),
    };
    let ordinary = CharacterRootCandidate {
        meshes: BTreeSet::new(),
        searchable_names: BTreeSet::from(["N_Mandy".to_string()]),
    };

    assert!(
        character_root_preference_score(&fusion, &preferred)
            > character_root_preference_score(&ordinary, &preferred)
    );
}

#[test]
fn live_retro_wound_recovers_only_from_exact_woundupper_sibling() {
    let repo_root = crate::repository_root()
        .parent()
        .expect("workspace root");
    let asset_path = repo_root
        .join("builds")
        .join("retrobution-20260613.ffclient")
        .join("cache")
        .join("extracted-bundles")
        .join("572d29197cb2a42b")
        .join("CustomAssetBundle-Retro_shared");
    if !asset_path.is_file() {
        eprintln!(
            "skipping live wound recovery test: fixture not found at {}",
            asset_path.display()
        );
        return;
    }
    let env = fusionforge::UnityEnvironment::from_paths(&[asset_path]);
    let asset = env.assets.first().expect("Retro_shared asset");
    let raw_clips = [5245_i64, 5262_i64]
        .into_iter()
        .map(|path_id| {
            let info = asset.objects.get(&path_id).expect("AnimationClip path id");
            RawAnimationClip {
                asset_name: asset.name.clone(),
                path_id,
                body: asset.read_object(0, info).expect("AnimationClip body"),
            }
        })
        .collect::<Vec<_>>();
    let catalog = build_curve_time_recovery_catalog(&raw_clips);
    let (upper_body, upper_recoveries) =
        recover_non_strict_curve_times(&raw_clips[0], &catalog);
    assert!(upper_recoveries.is_empty());
    let upper = decode_animation_clip(
        &raw_clips[0].asset_name,
        raw_clips[0].path_id,
        &upper_body,
        upper_recoveries,
    )
    .preview;

    let (wound_body, wound_recoveries) =
        recover_non_strict_curve_times(&raw_clips[1], &catalog);
    assert_eq!(wound_recoveries.len(), 1);
    let recovery = &wound_recoveries[0];
    assert_eq!(recovery["kind"], json!("scale"));
    assert_eq!(recovery["sourceIndex"], json!(19));
    assert_eq!(recovery["reference"]["pathId"], json!(5245));
    assert_eq!(recovery["reference"]["clipName"], json!("woundupper"));
    assert_eq!(recovery["originalTimes"], json!(vec![0.0; 11]));
    assert_eq!(
        recovery["recoveredTimes"],
        json!([
            0.0,
            0.03333333507180214,
            0.10000000894069672,
            0.13333334028720856,
            0.36666667461395264,
            0.4000000059604645,
            0.4333333373069763,
            0.46666666865348816,
            0.5,
            0.5666667222976685,
            0.7666668891906738,
        ])
    );
    let wound = decode_animation_clip(
        &raw_clips[1].asset_name,
        raw_clips[1].path_id,
        &wound_body,
        wound_recoveries,
    )
    .preview;
    assert!(wound.get("decodeWarnings").is_none());

    let animations = vec![upper, wound];
    let paths = animations
        .iter()
        .filter_map(|animation| animation.get("animationData"))
        .flat_map(|data| {
            ["translations", "rotations", "scales"]
                .into_iter()
                .flat_map(|kind| {
                    data.get(kind)
                        .and_then(JsonValue::as_array)
                        .into_iter()
                        .flatten()
                })
                .chain(
                    data.get("emptyTrsBindings")
                        .and_then(JsonValue::as_array)
                        .into_iter()
                        .flatten(),
                )
        })
        .filter_map(|track| track.get("path").and_then(JsonValue::as_str))
        .map(|path| format!("model/{path}"))
        .collect::<BTreeSet<_>>();
    let hierarchy = json!({
        "nodes": paths
            .into_iter()
            .map(|path| json!({ "path": path }))
            .collect::<Vec<_>>()
    });
    validate_exact_animation_source(&hierarchy, &animations)
        .expect("exact sibling time recovery validates without loss");
}

#[test]
fn live_bloodgnat_offsets_recover_only_from_exact_constant_siblings() {
    let repo_root = crate::repository_root()
        .parent()
        .expect("workspace root");
    let asset_path = repo_root
        .join("builds")
        .join("retrobution-20260613.ffclient")
        .join("cache")
        .join("extracted-bundles")
        .join("572d29197cb2a42b")
        .join("CustomAssetBundle-Retro_shared");
    if !asset_path.is_file() {
        eprintln!(
            "skipping live Blood Gnat recovery test: fixture not found at {}",
            asset_path.display()
        );
        return;
    }
    let env = fusionforge::UnityEnvironment::from_paths(&[asset_path]);
    let asset = env.assets.first().expect("Retro_shared asset");
    let raw_clips = [4145_i64, 4146_i64, 4234_i64, 4235_i64]
        .into_iter()
        .map(|path_id| {
            let info = asset.objects.get(&path_id).expect("AnimationClip path id");
            RawAnimationClip {
                asset_name: asset.name.clone(),
                path_id,
                body: asset.read_object(0, info).expect("AnimationClip body"),
            }
        })
        .collect::<Vec<_>>();
    let catalog = build_constant_curve_recovery_catalog(&raw_clips);
    let animations = raw_clips
        .iter()
        .map(|clip| {
            let plans = recover_conflicting_constant_curves(clip, &catalog);
            match clip.path_id {
                4145 => {
                    assert_eq!(plans.len(), 1, "Queen stand1 exact recovery");
                    assert_eq!(plans[0].kind, "translation");
                    assert_eq!(plans[0].path, "Bone00");
                    assert_eq!(plans[0].canonical_source_index, 67);
                    assert_eq!(plans[0].reference.path_id, 4146);
                }
                4234 => {
                    assert_eq!(plans.len(), 1, "Killer Blood Gnat stand1 exact recovery");
                    assert_eq!(plans[0].kind, "translation");
                    assert_eq!(plans[0].path, "Bone00");
                    assert_eq!(plans[0].canonical_source_index, 67);
                    assert_eq!(plans[0].reference.path_id, 4235);
                }
                4146 | 4235 => assert!(plans.is_empty(), "authority clip is not recovered"),
                _ => unreachable!(),
            }
            decode_animation_clip_with_curve_recoveries(
                &clip.asset_name,
                clip.path_id,
                &clip.body,
                Vec::new(),
                &plans,
            )
            .preview
        })
        .collect::<Vec<_>>();
    for (path_id, reference_path_id) in [(4145_i64, 4146_i64), (4234_i64, 4235_i64)] {
        let animation = animations
            .iter()
            .find(|animation| animation["pathId"] == json!(path_id))
            .expect("recovered stand1");
        assert!(animation.get("decodeWarnings").is_none());
        let recovery = &animation["animationData"]["curveRecoveries"][0];
        assert_eq!(recovery["source"]["sourceIndices"], json!([33, 67]));
        assert_eq!(recovery["canonical"]["sourceIndex"], json!(67));
        assert_eq!(recovery["rejected"][0]["sourceIndex"], json!(33));
        assert_eq!(recovery["reference"]["pathId"], json!(reference_path_id));
    }

    let paths = animations
        .iter()
        .filter_map(|animation| animation.get("animationData"))
        .flat_map(|data| {
            ["translations", "rotations", "scales", "emptyTrsBindings"]
                .into_iter()
                .flat_map(|field| {
                    data.get(field)
                        .and_then(JsonValue::as_array)
                        .into_iter()
                        .flatten()
                })
                .chain(
                    data.get("duplicateTrsBindings")
                        .and_then(JsonValue::as_array)
                        .into_iter()
                        .flatten(),
                )
        })
        .filter_map(|track| track.get("path").and_then(JsonValue::as_str))
        .map(|path| format!("model/{path}"))
        .collect::<BTreeSet<_>>();
    let hierarchy = json!({
        "nodes": paths
            .into_iter()
            .map(|path| json!({ "path": path }))
            .collect::<Vec<_>>()
    });
    validate_exact_animation_source(&hierarchy, &animations)
        .expect("both exact constant sibling recoveries validate without loss");
}

#[test]
fn live_fusion_chowder_eye_attachment_uses_the_rendered_instance_transform() {
    let repo_root = crate::repository_root()
        .parent()
        .expect("workspace root");
    let project = crate::repository_root()
        .join("work/sources/previous");
    let bundle = repo_root
        .join("builds")
        .join("retrobution-20260613")
        .join("Retro_shared.resourceFile");
    if !bundle.is_file() {
        eprintln!(
            "skipping live Fusion Chowder attachment test: fixture not found at {}",
            bundle.display()
        );
        return;
    }
    let preview = crate::preview_bundle_container_model(
        bundle.to_string_lossy().to_string(),
        Some(project.to_string_lossy().to_string()),
        vec!["mob/fusion_chowder.kfm".to_string()],
    )
    .expect("preview Fusion Chowder");
    let meshes = preview["meshes"].as_array().expect("meshes");
    let mesh_names = meshes
        .iter()
        .filter_map(|mesh| mesh["name"].as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        mesh_names,
        BTreeSet::from(["Box04:0", "Box04:1", "fusion_chowder_eye"]),
        "the shared preload range must not leak the Dracula or ordinary Chowder rig"
    );
    let joints = preview
        .pointer("/skeleton/joints")
        .and_then(JsonValue::as_array)
        .expect("Fusion Chowder joints");
    let joint_paths = joints
        .iter()
        .filter_map(|joint| joint["path"].as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(joints.len(), joint_paths.len());
    assert!(preview["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .any(|warning| warning.contains("unrelated character roots")));
    let eye = meshes
        .iter()
        .find(|mesh| mesh["name"] == "fusion_chowder_eye")
        .expect("Fusion Chowder eye mesh");
    assert_eq!(
        eye.pointer("/skin/source").and_then(JsonValue::as_str),
        Some("unity-rigid-mesh-attachment")
    );
    assert!(eye
        .pointer("/skin/jointPaths/0")
        .and_then(JsonValue::as_str)
        .is_some_and(|path| path.ends_with("Bip01 Head/fusion_chowder_eye")));

    let sampler = crate::npc_animation::NpcAnimationSampler::from_preview(&preview)
        .expect("Fusion Chowder must have one coherent skeleton");
    assert!(sampler.has_complete_skinning());
    assert!(sampler.has_sampleable_clip("stand1"));
    let pose = sampler.sample("stand1", 0.25).expect("sample stand1");
    let eye_pose = pose
        .meshes
        .iter()
        .find(|mesh| mesh.id == eye["id"].as_str().unwrap_or_default())
        .expect("sampled Fusion Chowder eye");
    assert!(eye_pose.skinned);
    assert!(eye_pose
        .positions
        .iter()
        .flatten()
        .all(|value| value.is_finite()));
}
