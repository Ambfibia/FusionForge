use super::*;

fn ogg_page(payload: &[u8], serial: u32, sequence: u32, checksum: u32) -> Vec<u8> {
    assert!(payload.len() < 255);
    let mut page = vec![0_u8; 27];
    page[..4].copy_from_slice(b"OggS");
    page[4] = 0;
    page[5] = 2;
    page[6..14].copy_from_slice(&123_u64.to_le_bytes());
    page[14..18].copy_from_slice(&serial.to_le_bytes());
    page[18..22].copy_from_slice(&sequence.to_le_bytes());
    page[22..26].copy_from_slice(&checksum.to_le_bytes());
    page[26] = 1;
    page.push(payload.len() as u8);
    page.extend_from_slice(payload);
    page
}

fn file(locale: Option<&str>) -> StrictAudioFile {
    StrictAudioFile {
        locale: locale.map(str::to_owned),
        path: "audio/voice/en/test/test.ogg".to_owned(),
        bytes: 4,
        blake3: "0".repeat(64),
        language_neutral: false,
    }
}

fn voice(owner: &str, true_name: &str) -> StrictAudioAsset {
    StrictAudioAsset {
        logical_key: format!(
            "voice/{owner}/{}",
            portable_component(true_name, "test").unwrap()
        ),
        aliases: Vec::new(),
        true_name: true_name.to_owned(),
        category: SemanticAudioCategory::Voice,
        owner: owner.to_owned(),
        scope: None,
        files: vec![file(Some("en"))],
    }
}

#[test]
fn incremental_commit_moves_only_changed_audio_and_authorities() {
    let temp = tempfile::tempdir().unwrap();
    let asset_root = temp.path().join("assets/game");
    let source = asset_root.join("audio/sfx/shared/computress_line.ogg");
    let untouched = asset_root.join("audio/sfx/shared/keep.ogg");
    let catalog = asset_root.join(STRICT_AUDIO_CATALOG_PATH);
    let manifest = asset_root.join(ASSET_MANIFEST_FILE);
    for path in [&source, &untouched, &catalog, &manifest] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
    }
    fs::write(&source, b"voice").unwrap();
    fs::write(&untouched, b"keep").unwrap();
    fs::write(&catalog, b"old catalog").unwrap();
    fs::write(&manifest, b"old manifest").unwrap();

    let stage = asset_root.join(".stage");
    let target_relative = "audio/voice/en/computress/computress_line.ogg";
    let staged_target = stage.join(native_path(target_relative));
    let staged_catalog = stage.join(native_path(STRICT_AUDIO_CATALOG_PATH));
    let staged_manifest = stage.join(ASSET_MANIFEST_FILE);
    for path in [&staged_target, &staged_catalog, &staged_manifest] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
    }
    fs::write(&staged_target, b"voice").unwrap();
    fs::write(&staged_catalog, b"new catalog").unwrap();
    fs::write(&staged_manifest, b"new manifest").unwrap();

    let files = vec![PlannedFile {
        source_absolute: source.clone(),
        source_path: "legacy/computress_line.ogg".to_owned(),
        target_path: target_relative.to_owned(),
        bytes: 5,
        blake3: blake3::hash(b"voice").to_hex().to_string(),
    }];
    let backup = asset_root.join(".backup");
    commit_plan(&asset_root, &stage, &backup, &files).unwrap();

    assert!(!source.exists());
    assert_eq!(
        fs::read(asset_root.join(native_path(target_relative))).unwrap(),
        b"voice"
    );
    assert_eq!(fs::read(&untouched).unwrap(), b"keep");
    assert_eq!(fs::read(&catalog).unwrap(), b"new catalog");
    assert_eq!(fs::read(&manifest).unwrap(), b"new manifest");
    assert!(!stage.exists());
    assert!(!backup.exists());
}

#[test]
fn nano_identity_requires_both_proven_owner_and_true_name_family() {
    assert_eq!(
        classify_existing(&voice("bloo", "Bloo_NanFlex02")).unwrap(),
        Route::NanoVoice {
            canonical_owner: "bloo".to_owned()
        }
    );
    assert_eq!(
        classify_existing(&voice("btrcup", "Buttercup_NanDismiss01")).unwrap(),
        Route::NanoVoice {
            canonical_owner: "buttercup".to_owned()
        }
    );
    assert_eq!(
        classify_existing(&voice("bloo", "Bloo_Tut01")).unwrap(),
        Route::Preserve
    );
    assert_eq!(
        classify_existing(&voice("unknown", "Bloo_NanFlex02")).unwrap(),
        Route::Preserve
    );
}

#[test]
fn nano_skill_policy_does_not_capture_similar_world_or_mob_audio() {
    assert!(matches!(
        classify_existing(&voice("damage_aku", "Damage_Aku_WizardStorm")).unwrap(),
        Route::NanoSkill { .. }
    ));
    assert_eq!(
        classify_existing(&voice("recall_warp", "Recall_Warp_ENTER")).unwrap(),
        Route::Preserve
    );
    assert_eq!(
        classify_existing(&voice("vampire_bb", "Vampire_BB_Attack01")).unwrap(),
        Route::Preserve
    );
}

#[test]
fn computress_groups_have_stable_sections() {
    assert_eq!(
        classify_existing(&voice("computer", "Computer_Summon01")).unwrap(),
        Route::Computress {
            section: "summon".to_owned()
        }
    );
    assert_eq!(
        classify_existing(&voice("computress_skill2", "Computress_skill2_01")).unwrap(),
        Route::Computress {
            section: "skills/skill2".to_owned()
        }
    );
    assert_eq!(
        classify_existing(&voice("computress", "Computress_NanDismiss01")).unwrap(),
        Route::Computress {
            section: "nano_events".to_owned()
        }
    );

    for section in [
        "dialogue",
        "summon",
        "nano_events",
        "skills/skill2",
        "skills/skill3",
    ] {
        let mut migrated = voice("computress", "Computress_Line01");
        migrated.logical_key = format!("voice/npcs/computress/{section}/line01");
        assert_eq!(
            classify_existing(&migrated).unwrap(),
            Route::Computress {
                section: section.to_owned()
            }
        );
    }

    for (logical_key, true_name, blake3) in PROVEN_COMPUTRESS_DIALOGUE_SFX {
        let mut fallback = StrictAudioAsset {
            logical_key: (*logical_key).to_owned(),
            aliases: Vec::new(),
            true_name: (*true_name).to_owned(),
            category: SemanticAudioCategory::Sfx,
            owner: "shared".to_owned(),
            scope: None,
            files: vec![file(None)],
        };
        fallback.files[0].blake3 = (*blake3).to_owned();
        assert_eq!(
            classify_existing(&fallback).unwrap(),
            Route::Computress {
                section: "dialogue".to_owned()
            }
        );
        fallback.files[0].blake3 = "0".repeat(64);
        assert_eq!(classify_existing(&fallback).unwrap(), Route::Preserve);
    }
}

#[test]
fn recovery_policy_accepts_only_proven_identity_and_exact_skills() {
    assert_eq!(
        nano_owner_from_true_name("Jack_O_Lantern_NanHappy01").as_deref(),
        Some("jackolantern")
    );
    assert_eq!(
        nano_owner_from_true_name("BelladonnaNan_PwrTC01").as_deref(),
        Some("belladonna")
    );
    assert!(nano_owner_from_true_name("Computress_NanDismiss01").is_none());
    assert!(nano_owner_from_true_name("Recall_Warp_ENTER").is_none());

    let mut legacy = voice("bloo", "Bloo_NanFlex02");
    legacy.scope = Some("retrobution_nano_recovery".to_owned());
    assert!(normalize_recovery_scope(&mut legacy));
    assert_eq!(legacy.scope.as_deref(), Some("nano_recovery"));
    assert!(!normalize_recovery_scope(&mut legacy));
    assert_eq!(
        normalize_recovery_source_path("retrobution-nano-recovery/42__Line.ogg"),
        ("nano-recovery/42__Line.ogg".to_owned(), true)
    );
}

#[test]
fn every_audited_current_nano_owner_has_a_true_name_family() {
    let cases = [
        ("aku", "Aku_NanHappy01"),
        ("billy", "Billy_NanHappy01"),
        ("bloo", "Bloo_NanHappy01"),
        ("blossom", "Blossom_NanHappy01"),
        ("btrcup", "Buttercup_NanHappy01"),
        ("buttercup", "Buttercup_NanHappy01"),
        ("bubbles", "Bubbles_NanHappy01"),
        ("cheese", "Cheese_NanHappy01"),
        ("coco", "Coco_NanHappy01"),
        ("courage", "Courage_NanHappy01"),
        ("deedee", "DeeDee_NanHappy01"),
        ("demongo", "Demongo_NanHappy01"),
        ("dexter", "Dexter_NanHappy01"),
        ("ed", "Ed_NanHappy01"),
        ("edd", "Edd_NanHappy01"),
        ("eddy", "Eddy_NanHappy01"),
        ("eduardo", "Eduardo_NanHappy01"),
        ("finn", "Finn_NanHappy01"),
        ("fourarms", "Fourarms_NanHappy01"),
        ("grim", "Grim_NanHappy01"),
        ("hex", "Hex_NanHappy01"),
        ("him", "Him_NanHappy01"),
        ("humongo", "Humongo_NanHappy01"),
        ("jbravo", "JBravo_NanHappy01"),
        ("juniper", "Juniper_NanHappy01"),
        ("mac", "Mac_NanHappy01"),
        ("mandark", "Mandark_NanHappy01"),
        ("mandy", "Mandy_NanHappy01"),
        ("megas", "Megas_NanHappy01"),
        ("mojo", "Mojo_NanHappy01"),
        ("numfive", "NumFive_NanHappy01"),
        ("numfour", "NumFour_NanHappy01"),
        ("numone", "NumOne_NanHappy01"),
        ("numthree", "NumThree_NanHappy01"),
        ("numtwo", "NumTwo_NanHappy01"),
        ("sjack", "SJack_NanHappy01"),
        ("swampfire", "Swampfire_NanHappy01"),
        ("utonium", "Utonium_NanHappy01"),
        ("vilgax", "Vilgax_NanHappy01"),
        ("wilt", "Wilt_NanHappy01"),
    ];
    assert_eq!(cases.len(), 40);
    for (legacy_owner, true_name) in cases {
        assert_eq!(
            nano_owner_from_true_name(true_name).as_deref(),
            canonical_current_nano_owner(legacy_owner),
            "missing closed Nano family for {legacy_owner}/{true_name}"
        );
    }
}

#[test]
fn unnamed_recovery_stem_keeps_path_id_for_payload_deduplication() {
    assert_eq!(
        parse_named_ogg_stem("10612__"),
        Some((10_612, String::new()))
    );
}

#[test]
fn ogg_packet_identity_ignores_container_fields_but_not_packet_payload() {
    let first = ogg_page(b"vorbis-packet", 1, 0, 0x1111_1111);
    let repacked = ogg_page(b"vorbis-packet", 99, 42, 0xfeed_beef);
    let changed = ogg_page(b"vorbis-packet!", 99, 42, 0xfeed_beef);
    assert_eq!(
        ogg_packet_hash_bytes(&first).unwrap(),
        ogg_packet_hash_bytes(&repacked).unwrap()
    );
    assert_ne!(
        ogg_packet_hash_bytes(&first).unwrap(),
        ogg_packet_hash_bytes(&changed).unwrap()
    );
}

#[test]
fn key_registration_rejects_primary_alias_collisions_case_insensitively() {
    let mut keys = BTreeSet::new();
    let mut first = voice("bloo", "Bloo_NanFlex02");
    first.aliases.push("voice/legacy/line".to_owned());
    register_keys(&first, &mut keys).unwrap();
    let mut second = voice("billy", "Billy_NanFlex02");
    second.logical_key = "VOICE/LEGACY/LINE".to_owned();
    assert!(register_keys(&second, &mut keys).is_err());
}
