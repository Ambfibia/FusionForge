use super::*;
use serde_json::json;

#[test]
fn legacy_additive_tracks_are_rebased_against_the_first_clip_sample() {
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let reference_rotation = [0.0, half, 0.0, half];
    let source_rotation = [0.0, half, 0.0, half];
    let source_rotation_2 = quat_multiply([0.0, 0.0, half, half], source_rotation);
    let source_tangent = [0.25, -0.5, 0.75, -1.0];
    let nodes = [PlayerRigNode {
        actor_bone_index: 0,
        source_transform_path_id: 1,
        source_game_object_path_id: 2,
        true_name: "bone".to_owned(),
        full_path: "m/bone".to_owned(),
        parent_actor_bone_index: None,
        translation: [10.0, 20.0, 30.0],
        rotation: reference_rotation,
        scale: [1.0; 3],
    }];
    let mut clip = AnimationClip {
        name: "height_Add".to_owned(),
        duration: 1.0,
        declared_duration: None,
        keyed_duration: Some(1.0),
        event_duration: None,
        sample_rate: Some(30.0),
        wrap_mode: None,
        looped: false,
        channels: vec![
            AnimationChannel {
                target_node: 0,
                source_index: 0,
                source_encoding: EmptyTrsSourceEncoding::Plain,
                source_key_count: 2,
                source_key_indices: vec![0, 1],
                duplicate_keys: Vec::new(),
                interpolation: Interpolation::CubicSpline,
                times: vec![0.0, 1.0],
                values: TrackValues::Translation(vec![[11.0, 22.0, 33.0], [12.0, 24.0, 36.0]]),
                in_tangents: Some(TrackValues::Translation(vec![[4.0, 5.0, 6.0]])),
                out_tangents: Some(TrackValues::Translation(vec![[7.0, 8.0, 9.0]])),
                tangent_modes: Vec::new(),
            },
            AnimationChannel {
                target_node: 0,
                source_index: 0,
                source_encoding: EmptyTrsSourceEncoding::Plain,
                source_key_count: 2,
                source_key_indices: vec![0, 1],
                duplicate_keys: Vec::new(),
                interpolation: Interpolation::CubicSpline,
                times: vec![0.0, 1.0],
                values: TrackValues::Rotation(vec![source_rotation, source_rotation_2]),
                in_tangents: Some(TrackValues::Rotation(vec![source_tangent])),
                out_tangents: Some(TrackValues::Rotation(vec![source_tangent])),
                tangent_modes: Vec::new(),
            },
        ],
        metadata: AnimationMetadata::default(),
    };

    rebase_legacy_additive_clip(&mut clip, &nodes).unwrap();

    let TrackValues::Translation(values) = &clip.channels[0].values else {
        panic!("translation channel changed kind");
    };
    assert_eq!(values, &[[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]]);
    let Some(TrackValues::Translation(tangents)) = &clip.channels[0].in_tangents else {
        panic!("translation tangent changed kind");
    };
    assert_eq!(tangents, &[[4.0, 5.0, 6.0]]);

    let TrackValues::Rotation(values) = &clip.channels[1].values else {
        panic!("rotation channel changed kind");
    };
    let restored = quat_multiply(values[1], source_rotation);
    for (actual, expected) in restored.into_iter().zip(source_rotation_2) {
        assert!((actual - expected).abs() < 1.0e-12);
    }
    let Some(TrackValues::Rotation(tangents)) = &clip.channels[1].in_tangents else {
        panic!("rotation tangent changed kind");
    };
    let restored = quat_multiply(tangents[0], reference_rotation);
    for (actual, expected) in restored.into_iter().zip(source_tangent) {
        assert!((actual - expected).abs() < 1.0e-12);
    }
}

#[test]
fn legacy_packed_quaternion_decoder_matches_editor_reference() {
    // First packed value of Character_Rex stand3/Bip01 NonAccum. This is
    // the same immutable vector used by FusionForge's proven decoder.
    let decoded = unpack_legacy_compressed_quaternion(0x25df_f7ff);
    let expected = [-0.001956947, -0.000977517, -0.704789834, -0.709412789];
    for (actual, expected) in decoded.into_iter().zip(expected) {
        assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
    }
}

#[test]
fn packed_times_are_little_endian_bit_stream_deltas() {
    let value = json!({
        "m_NumItems": 2,
        "m_BitSize": 7,
        "m_Data": {"base64": "ADI=", "bytes": 2}
    });
    assert_eq!(read_packed_bits(&value).unwrap(), [0, 100]);
}

#[test]
fn compressed_rotation_keeps_exact_smooth_quaternion_slopes() {
    // Male stand1, Bip01/.../Bip01 Spine, PathID 237023.
    let body = json!({
        "m_CompressedRotationCurves": [{
            "m_Path": "Bip01/Bip01 NonAccum/Bip01 Pelvis/Bip01 Spine",
            "m_Times": {
                "m_NumItems": 4,
                "m_BitSize": 8,
                "m_Data": {"base64": "AJVyRg==", "bytes": 4}
            },
            "m_Values": {
                "m_NumItems": 4,
                "m_Data": {
                    "base64": "//fff+fnn38v6B+B//fffw==",
                    "bytes": 16
                }
            },
            "m_Slopes": {
                "m_NumItems": 16,
                "m_Range": 0.088789202272892,
                "m_Start": -0.03333768621087074,
                "m_BitSize": 6,
                "m_Data": {"base64": "QLdc/0RaQ0dZgLVc", "bytes": 12}
            }
        }]
    });
    let paths = BTreeMap::from([(
        "Bip01/Bip01 NonAccum/Bip01 Pelvis/Bip01 Spine".to_owned(),
        7,
    )]);
    let mut channels = Vec::new();
    decode_compressed_rotation_curves(&body, &paths, &mut channels).unwrap();
    assert_eq!(channels.len(), 1);
    let channel = &channels[0];
    assert!(matches!(channel.interpolation, Interpolation::CubicSpline));
    assert_eq!(channel.times, [0.0, 1.49, 2.63, 3.33]);
    let TrackValues::Rotation(in_tangents) = channel.in_tangents.as_ref().expect("in tangents")
    else {
        panic!("compressed quaternion in tangents changed type");
    };
    let TrackValues::Rotation(out_tangents) =
        channel.out_tangents.as_ref().expect("out tangents")
    else {
        panic!("compressed quaternion out tangents changed type");
    };
    assert_eq!(in_tangents, out_tangents);
    let expected = [
        -0.03333768621087074,
        -0.007533533883000182,
        0.017834809623540393,
        -0.000922580619180012,
    ];
    for (actual, expected) in in_tangents[0].into_iter().zip(expected) {
        assert!((actual - expected).abs() < 1.0e-12);
    }
}

#[test]
fn gender_contract_uses_exact_legacy_routes_and_no_unity_runtime() {
    assert_eq!(MALE.route, "actor/m.kfm");
    assert_eq!(FEMALE.route, "actor/w.kfm");
    assert_eq!(MALE.clips[0], ("stand1", 237_023));
    assert_eq!(FEMALE.clips[0], ("stand1", 237_027));
    assert_eq!(MALE.transform_indices_field, "transformIndicesM");
    assert_eq!(FEMALE.transform_indices_field, "transformIndicesF");
}

#[test]
fn runtime_clip_contract_has_exact_tutorial_player_sources() {
    assert_eq!(
        &MALE_RUNTIME_CLIPS[8..16],
        &[
            ("run", 34_268, "male-run.json"),
            ("staying", 34_269, "male-staying.json"),
            ("standup", 34_404, "male-standup.json"),
            ("runback", 34_249, "male-runback.json"),
            ("jumpstart", 34_390, "male-jumpstart.json"),
            ("jump", 34_314, "male-jump.json"),
            ("jumpend", 34_224, "male-jumpend.json"),
            ("jumplandrun", 34_256, "male-jumplandrun.json"),
        ]
    );
    assert_eq!(
        &FEMALE_RUNTIME_CLIPS[8..16],
        &[
            ("run", 34_191, "female-run.json"),
            ("staying", 34_194, "female-staying.json"),
            ("standup", 34_533, "female-standup.json"),
            ("runback", 34_611, "female-runback.json"),
            ("jumpstart", 34_557, "female-jumpstart.json"),
            ("jump", 34_605, "female-jump.json"),
            ("jumpend", 34_626, "female-jumpend.json"),
            ("jumplandrun", 34_567, "female-jumplandrun.json"),
        ]
    );
    assert_eq!(
        &MALE_RUNTIME_CLIPS[16..18],
        &[
            ("die", 34_531, "male-die.json"),
            ("death", 34_462, "male-death.json"),
        ]
    );
    assert_eq!(
        &FEMALE_RUNTIME_CLIPS[16..18],
        &[
            ("die", 34_444, "female-die.json"),
            ("death", 34_537, "female-death.json"),
        ]
    );
    assert_eq!(
        &MALE_RUNTIME_CLIPS[18..31],
        &[
            ("rifleready", 34_367, "male-rifleready.json"),
            ("stickattack1", 34_332, "male-stickattack1.json"),
            ("stickattack1upper", 34_412, "male-stickattack1upper.json"),
            ("pistolattack1", 34_380, "male-pistolattack1.json"),
            ("pistolattack1upper", 34_240, "male-pistolattack1upper.json",),
            ("rifleattack1", 34_555, "male-rifleattack1.json"),
            ("rifleattack1upper", 34_305, "male-rifleattack1upper.json"),
            ("riflerun", 34_321, "male-riflerun.json"),
            ("riflerunback", 34_331, "male-riflerunback.json"),
            ("riflejumpstart", 34_354, "male-riflejumpstart.json"),
            ("riflejump", 34_387, "male-riflejump.json"),
            ("riflejumpend", 34_415, "male-riflejumpend.json"),
            ("riflejumplandrun", 34_365, "male-riflejumplandrun.json"),
        ]
    );
    assert_eq!(
        &FEMALE_RUNTIME_CLIPS[18..31],
        &[
            ("rifleready", 34_595, "female-rifleready.json"),
            ("stickattack1", 34_579, "female-stickattack1.json"),
            ("stickattack1upper", 34_315, "female-stickattack1upper.json",),
            ("pistolattack1", 34_414, "female-pistolattack1.json"),
            (
                "pistolattack1upper",
                34_286,
                "female-pistolattack1upper.json",
            ),
            ("rifleattack1", 34_445, "female-rifleattack1.json"),
            ("rifleattack1upper", 34_466, "female-rifleattack1upper.json"),
            ("riflerun", 34_585, "female-riflerun.json"),
            ("riflerunback", 34_578, "female-riflerunback.json"),
            ("riflejumpstart", 34_603, "female-riflejumpstart.json"),
            ("riflejump", 34_627, "female-riflejump.json"),
            ("riflejumpend", 34_571, "female-riflejumpend.json"),
            ("riflejumplandrun", 34_559, "female-riflejumplandrun.json"),
        ]
    );
    assert_eq!(
        &MALE_RUNTIME_CLIPS[31..35],
        &[
            ("turnleft", 34_346, "male-turnleft.json"),
            ("turnright", 34_384, "male-turnright.json"),
            ("rifleturnleft", 34_231, "male-rifleturnleft.json"),
            ("rifleturnright", 34_216, "male-rifleturnright.json"),
        ]
    );
    assert_eq!(
        &FEMALE_RUNTIME_CLIPS[31..35],
        &[
            ("turnleft", 34_469, "female-turnleft.json"),
            ("turnright", 34_619, "female-turnright.json"),
            ("rifleturnleft", 34_622, "female-rifleturnleft.json"),
            ("rifleturnright", 34_612, "female-rifleturnright.json"),
        ]
    );
    assert_eq!(
        &MALE_RUNTIME_CLIPS[35..40],
        &[
            ("swim", 34_486, "male-swim.json"),
            ("swimback", 34_453, "male-swimback.json"),
            ("swimidle", 34_524, "male-swimidle.json"),
            ("swimleft", 34_535, "male-swimleft.json"),
            ("swimright", 34_514, "male-swimright.json"),
        ]
    );
    assert_eq!(
        &FEMALE_RUNTIME_CLIPS[35..40],
        &[
            ("swim", 34_185, "female-swim.json"),
            ("swimback", 34_165, "female-swimback.json"),
            ("swimidle", 34_512, "female-swimidle.json"),
            ("swimleft", 34_503, "female-swimleft.json"),
            ("swimright", 34_498, "female-swimright.json"),
        ]
    );
    assert!(runtime_clip_loops("run"));
    assert!(runtime_clip_loops("runback"));
    assert!(runtime_clip_loops("jump"));
    assert!(runtime_clip_loops("death"));
    assert!(runtime_clip_loops("swim"));
    assert!(runtime_clip_loops("swimidle"));
    assert!(runtime_clip_loops("stickstand1"));
    assert!(runtime_clip_loops("stickready"));
    assert!(runtime_clip_loops("pistolstand1"));
    assert!(runtime_clip_loops("pistolrun"));
    assert!(runtime_clip_loops("bombstand1"));
    assert!(runtime_clip_loops("bombrun"));
    assert!(runtime_clip_loops("rocketstand1"));
    assert!(runtime_clip_loops("rocketrun"));
    assert!(!runtime_clip_loops("staying"));
    assert!(!runtime_clip_loops("standup"));
    assert!(!runtime_clip_loops("jumpstart"));
    assert!(!runtime_clip_loops("jumpend"));
    assert!(!runtime_clip_loops("jumplandrun"));
    assert!(!runtime_clip_loops("die"));
    assert_eq!(runtime_clip_status("die"), "native-bevy-ready");
    assert_eq!(runtime_clip_status("death"), "native-bevy-ready");
    assert_eq!(runtime_clip_status("stand1"), "native-bevy-ready");
    assert_eq!(runtime_clip_status("run"), "native-bevy-ready");
    assert_eq!(runtime_clip_status("swimright"), "native-bevy-ready");
    assert_eq!(runtime_clip_status("stickjump"), "native-bevy-ready");
    assert_eq!(runtime_clip_status("pistoljumpend"), "native-bevy-ready");
    assert_eq!(runtime_clip_status("bombjumpend"), "native-bevy-ready");
    assert_eq!(runtime_clip_status("rocketjumpend"), "native-bevy-ready");
    assert_eq!(
        &MALE_RUNTIME_CLIPS[86..88],
        &[
            ("attack1", 34_235, "male-attack1.json"),
            ("attack1upper", 34_213, "male-attack1upper.json"),
        ]
    );
    assert_eq!(
        &FEMALE_RUNTIME_CLIPS[86..88],
        &[
            ("attack1", 34_171, "female-attack1.json"),
            ("attack1upper", 34_174, "female-attack1upper.json"),
        ]
    );
    assert_eq!(runtime_clip_status("attack1"), "native-bevy-ready");
    assert!(!runtime_clip_loops("attack1"));
    let expected_emotes = [
        "cry", "angry", "shocked", "hello", "thank", "dance1", "kiss", "agree", "laugh", "no",
        "flex", "tease", "ok", "applaud", "cheer", "dance2", "dance3", "dance4", "dance5",
        "goodbye", "beach1", "beach2", "beach3",
    ];
    for runtime_clips in [&MALE_RUNTIME_CLIPS, &FEMALE_RUNTIME_CLIPS] {
        assert_eq!(
            runtime_clips[88..111]
                .iter()
                .map(|(name, _, _)| *name)
                .collect::<Vec<_>>(),
            expected_emotes
        );
        assert!(runtime_clips[88..111].iter().all(|(name, _, _)| {
            runtime_clip_status(name) == "native-bevy-ready" && !runtime_clip_loops(name)
        }));
    }
    assert_eq!(
        &MALE_RUNTIME_CLIPS[111..],
        &[
            ("mount1", 34_330, "male-mount1.json"),
            ("mount2", 34_338, "male-mount2.json"),
        ]
    );
    assert_eq!(
        &FEMALE_RUNTIME_CLIPS[111..],
        &[
            ("mount1", 34_280, "female-mount1.json"),
            ("mount2", 34_294, "female-mount2.json"),
        ]
    );
    for name in ["mount1", "mount2"] {
        assert_eq!(runtime_clip_status(name), "native-bevy-ready");
        assert!(runtime_clip_loops(name));
    }
    assert!(MALE_RUNTIME_CLIPS.contains(&("bombstand1", 34_401, "male-bombstand1.json")));
    assert!(FEMALE_RUNTIME_CLIPS.contains(&(
        "rocketstand1",
        34_546,
        "female-rocketstand1.json"
    )));
    assert_eq!(
        runtime_clip_status("height_Add"),
        "native-bevy-additive-delta-ready"
    );
    assert_eq!(
        runtime_clip_status("height"),
        "native-bevy-static-scale-ready"
    );
    assert_eq!(
        runtime_clip_status("rifleturnleft"),
        "native-bevy-additive-delta-ready"
    );
    assert_eq!(runtime_clip_status("staying"), runtime_clip_status("run"));
    assert_eq!(runtime_clip_status("standup"), runtime_clip_status("run"));
    assert_eq!(runtime_clip_status("runback"), runtime_clip_status("run"));
    assert_eq!(runtime_clip_status("jumpstart"), runtime_clip_status("run"));
    assert_eq!(runtime_clip_status("jump"), runtime_clip_status("run"));
    assert_eq!(runtime_clip_status("jumpend"), runtime_clip_status("run"));
    assert_eq!(
        runtime_clip_status("jumplandrun"),
        runtime_clip_status("run")
    );
}

#[test]
fn ffr_custom_extension_is_additive_and_never_replaces_primary_clips() {
    assert_eq!(CUSTOM_RUNTIME_CLIPS.len(), 14);
    let names = CUSTOM_RUNTIME_CLIPS
        .iter()
        .map(|spec| spec.semantic_name)
        .collect::<BTreeSet<_>>();
    assert_eq!(names.len(), CUSTOM_RUNTIME_CLIPS.len());
    assert!(names.iter().all(|name| name.starts_with("ffr_")));
    assert!(names.iter().all(|name| !name.contains("run")));
    assert!(
        CUSTOM_RUNTIME_CLIPS
            .iter()
            .all(|spec| runtime_clip_status(spec.semantic_name) == "native-bevy-ready")
    );

    let bully = CUSTOM_RUNTIME_CLIPS
        .iter()
        .find(|spec| spec.semantic_name == "ffr_dance_bully")
        .expect("bully extension");
    assert_eq!(
        bully.source_for(PlayerRigGender::Male),
        ("m_dance_bully", 642, "m_dance_bully.json", 132)
    );
    assert_eq!(
        bully.source_for(PlayerRigGender::Female),
        ("f_dance_bully", 645, "f_dance_bully.json", 102)
    );
}

#[test]
fn runtime_animation_transaction_restores_every_original_after_injected_failure() {
    let temp = tempfile::tempdir().unwrap();
    let outputs = [
        (temp.path().join("male.glb"), b"new-male".to_vec()),
        (temp.path().join("female.glb"), b"new-female".to_vec()),
        (temp.path().join("contract.json"), b"new-contract".to_vec()),
        (
            temp.path().join("asset-manifest.json"),
            b"new-manifest".to_vec(),
        ),
    ];
    let originals = [
        b"old-male".as_slice(),
        b"old-female",
        b"old-contract",
        b"old-manifest",
    ];
    for ((path, _), bytes) in outputs.iter().zip(originals) {
        fs::write(path, bytes).unwrap();
    }

    let result = commit_player_rig_animation_outputs_with_gate(&outputs, |installed, _| {
        if installed == 1 {
            rig_error("injected transaction failure")
        } else {
            Ok(())
        }
    });
    assert!(result.is_err());
    for ((path, _), expected) in outputs.iter().zip(originals) {
        assert_eq!(fs::read(path).unwrap(), expected);
        assert!(
            !player_rig_animation_sidecar(path, PLAYER_RIG_ANIMATION_STAGE_SUFFIX)
                .unwrap()
                .exists()
        );
        assert!(
            !player_rig_animation_sidecar(path, PLAYER_RIG_ANIMATION_BACKUP_SUFFIX)
                .unwrap()
                .exists()
        );
    }
}

#[test]
fn runtime_animation_manifest_refreshes_exact_existing_entries() {
    let temp = tempfile::tempdir().unwrap();
    let asset_root = temp.path();
    let outputs = [
        (
            asset_root.join(MALE_SHARED_SKELETON_GLB_PATH),
            b"new-male".to_vec(),
        ),
        (
            asset_root.join(FEMALE_SHARED_SKELETON_GLB_PATH),
            b"new-female".to_vec(),
        ),
        (
            asset_root.join(PLAYER_SHARED_RIG_CONTRACT_PATH),
            b"new-contract".to_vec(),
        ),
    ];
    let manifest = ProjectAssetManifest {
        schema: PROJECT_ASSET_SCHEMA.to_owned(),
        protocol: 4,
        locale: "ru".to_owned(),
        source_pack: crate::SourcePackIdentity {
            schema: "source-pack-test".to_owned(),
            manifest_blake3: "source-hash".to_owned(),
        },
        files: vec![
            ProjectAssetFile {
                source_path: "male-source".to_owned(),
                path: MALE_SHARED_SKELETON_GLB_PATH.to_owned(),
                kind: ProjectAssetKind::Model,
                bytes: 1,
                blake3: "old-male".to_owned(),
            },
            ProjectAssetFile {
                source_path: "female-source".to_owned(),
                path: FEMALE_SHARED_SKELETON_GLB_PATH.to_owned(),
                kind: ProjectAssetKind::Model,
                bytes: 1,
                blake3: "old-female".to_owned(),
            },
            ProjectAssetFile {
                source_path: "contract-source".to_owned(),
                path: PLAYER_SHARED_RIG_CONTRACT_PATH.to_owned(),
                kind: ProjectAssetKind::Data,
                bytes: 1,
                blake3: "old-contract".to_owned(),
            },
        ],
    };
    fs::write(
        asset_root.join(ASSET_MANIFEST_FILE),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let (path, bytes) = prepare_player_rig_animation_manifest(asset_root, &outputs).unwrap();
    assert_eq!(path, asset_root.join(ASSET_MANIFEST_FILE));
    assert_eq!(bytes.last(), Some(&b'\n'));
    let refreshed: ProjectAssetManifest = serde_json::from_slice(&bytes).unwrap();
    for (destination, output_bytes) in &outputs {
        let relative = destination
            .strip_prefix(asset_root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let matches = refreshed
            .files
            .iter()
            .filter(|entry| entry.path == relative)
            .collect::<Vec<_>>();
        let [entry] = matches.as_slice() else {
            panic!("expected one refreshed entry for {relative:?}")
        };
        assert_eq!(entry.bytes, output_bytes.len() as u64);
        assert_eq!(
            entry.blake3,
            blake3::hash(output_bytes).to_hex().to_string()
        );
    }
    assert_eq!(refreshed.files[0].source_path, "male-source");
    assert_eq!(refreshed.files[1].source_path, "female-source");
    assert_eq!(refreshed.files[2].source_path, "contract-source");
}

#[test]
fn runtime_animation_transaction_cleans_stages_when_later_parent_fails() {
    let temp = tempfile::tempdir().unwrap();
    let valid = temp.path().join("valid.glb");
    let blocked_parent = temp.path().join("not-a-directory");
    fs::write(&blocked_parent, b"blocking-file").unwrap();
    let blocked = blocked_parent.join("blocked.glb");
    let outputs = [
        (valid.clone(), b"new-valid".to_vec()),
        (blocked, b"new-blocked".to_vec()),
    ];

    assert!(commit_player_rig_animation_outputs(&outputs).is_err());
    assert!(!valid.exists());
    assert!(
        !player_rig_animation_sidecar(&valid, PLAYER_RIG_ANIMATION_STAGE_SUFFIX)
            .unwrap()
            .exists()
    );
    assert!(
        !player_rig_animation_sidecar(&valid, PLAYER_RIG_ANIMATION_BACKUP_SUFFIX)
            .unwrap()
            .exists()
    );
    assert_eq!(fs::read(blocked_parent).unwrap(), b"blocking-file");
}

#[test]
fn actor_skin_combiner_clothes_slots_match_cn_avatar_status() {
    assert_eq!(
        [
            CharacterAppearanceCategory::Shoes,
            CharacterAppearanceCategory::Pants,
            CharacterAppearanceCategory::Shirt,
            CharacterAppearanceCategory::Face,
            CharacterAppearanceCategory::Hair,
        ]
        .map(actor_skin_combiner_clothes_index),
        [0, 1, 2, 3, 4]
    );
}

#[test]
fn player_rig_part_contract_serializes_the_clothes_slot_in_camel_case() {
    let value = serde_json::to_value(PlayerRigPartContract {
        exact_route: "wear/m_face_001_type01.nif".to_owned(),
        true_name: "m_face_001_type01".to_owned(),
        glb: "characters/player/equipment/mask/m_face_001_type01/m_face_001_type01.glb"
            .to_owned(),
        actor_skin_combiner_clothes_index: 3,
        skins: Vec::new(),
    })
    .unwrap();
    assert_eq!(
        value
            .get("actorSkinCombinerClothesIndex")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert!(value.get("actor_skin_combiner_clothes_index").is_none());
}
