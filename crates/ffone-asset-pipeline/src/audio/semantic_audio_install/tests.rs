use super::*;
use crate::{PROJECT_ASSET_SCHEMA, SourcePackIdentity};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn exact_20260613_profile_is_uuid_and_provenance_guarded_without_changing_legacy() {
    let options = SemanticAudioInstallOptions::new("unused", "unused", "retrobution-20260613");
    let mut manifest = ProjectAssetManifest {
        schema: PROJECT_ASSET_SCHEMA.to_owned(),
        protocol: 104,
        locale: "ru-RU".to_owned(),
        source_pack: SourcePackIdentity {
            schema: CONTENT_PACK_SCHEMA.to_owned(),
            manifest_blake3: RETROBUTION_EXACT_20260613_PACK_MANIFEST_BLAKE3.to_owned(),
        },
        files: Vec::new(),
    };
    let mut report = CookReport {
        schema: COOK_REPORT_SCHEMA.to_owned(),
        build_uuid: RETROBUTION_EXACT_20260613_BUILD_UUID.to_owned(),
        locale: "ru-RU".to_owned(),
        counts: CookCounts {
            audio: RETROBUTION_EXACT_20260613_AUDIO_ASSET_COUNT as u64,
        },
        mappings: Vec::new(),
    };

    assert_eq!(
        select_audio_profile(
            &options,
            &manifest,
            &report,
            RETROBUTION_EXACT_20260613_COOK_REPORT_SHA256,
        )
        .unwrap(),
        RETROBUTION_EXACT_20260613_AUDIO_ASSET_COUNT
    );

    manifest.source_pack.manifest_blake3 = "0".repeat(64);
    let error = select_audio_profile(
        &options,
        &manifest,
        &report,
        RETROBUTION_EXACT_20260613_COOK_REPORT_SHA256,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("semantic-audio profile provenance mismatch")
    );

    report.build_uuid = "legacy-retrobution-build".to_owned();
    assert_eq!(
        select_audio_profile(&options, &manifest, &report, "not-used-by-legacy").unwrap(),
        RETROBUTION_AUDIO_ASSET_COUNT
    );
}

#[test]
fn classification_is_evidence_ordered_and_never_silently_guesses() {
    let ambient = classify(
        "01 Ambient KND Training",
        &[BTreeMap::from([(
            "bundle".to_owned(),
            json!("FutureMusic.resourceFile"),
        )])],
    )
    .unwrap();
    assert_eq!(ambient.category, SemanticAudioCategory::Ambient);
    assert_eq!(ambient.owner, "shared");
    assert!(!ambient.proof.uncertain);

    let voice = classify(
        "M_KNDOp1_Greeting02",
        &[BTreeMap::from([(
            "bundle".to_owned(),
            json!("NpcVoiceShared_004.resourceFile"),
        )])],
    )
    .unwrap();
    assert_eq!(voice.category, SemanticAudioCategory::Voice);
    assert_eq!(voice.owner, "m_kndop1");
    assert!(!voice.proof.uncertain);

    let fallback = classify(
        "Unresolved Clip",
        &[BTreeMap::from([(
            "bundle".to_owned(),
            json!("CoreShared.resourceFile"),
        )])],
    )
    .unwrap();
    assert_eq!(fallback.category, SemanticAudioCategory::Sfx);
    assert_eq!(fallback.owner, "shared");
    assert!(fallback.proof.uncertain);
    assert_eq!(
        fallback.proof.certainty,
        ClassificationCertainty::ExplicitSharedFallback
    );
}

#[test]
fn duplicate_true_names_receive_provenance_sorted_hash_free_variants() {
    let mut prepared = vec![
        prepared_audio(
            "Same Name",
            "audio/same_name--1111111111111111.ogg",
            "1111111111111111111111111111111111111111111111111111111111111111",
            "ZBundle.resourceFile",
        ),
        prepared_audio(
            "Same Name",
            "audio/same_name--2222222222222222.ogg",
            "2222222222222222222222222222222222222222222222222222222222222222",
            "ABundle.resourceFile",
        ),
    ];
    assign_destinations(&mut prepared).unwrap();
    let first = prepared
        .iter()
        .find(|audio| audio.provenance[0]["bundle"] == "ABundle.resourceFile")
        .unwrap();
    let second = prepared
        .iter()
        .find(|audio| audio.provenance[0]["bundle"] == "ZBundle.resourceFile")
        .unwrap();
    assert!(
        first
            .destination
            .contains("variants/variant_01/same_name.ogg")
    );
    assert!(
        second
            .destination
            .contains("variants/variant_02/same_name.ogg")
    );
    assert!(!contains_hash_suffix(&first.destination));
    assert!(!contains_hash_suffix(&second.destination));
}

#[test]
fn transactional_install_keeps_originals_and_preserves_every_provenance_record() {
    let fixture = Fixture::new(&[
        ("00 Main Theme", "LobbyMusic.resourceFile", b"OggSmusic"),
        (
            "Eddy_QGreeting",
            "NpcVoiceShared_004.resourceFile",
            b"OggSvoice",
        ),
        ("Mystery", "CoreShared.resourceFile", b"OggSmystery"),
    ]);
    let options = SemanticAudioInstallOptions::new(
        &fixture.asset_root,
        &fixture.cook_report,
        "retrobution-test",
    )
    .with_expected_audio_assets(3);
    let report = install_semantic_audio(&options).unwrap();
    assert_eq!(report.installed_audio_files, 3);
    for entry in &fixture.sources {
        assert!(fixture.asset_root.join(&entry.path).is_file());
    }
    assert!(
        fixture
            .asset_root
            .join("audio/music/00_main_theme.ogg")
            .is_file()
    );
    assert!(
        fixture
            .asset_root
            .join("audio/voice/eddy/eddy_qgreeting.ogg")
            .is_file()
    );
    assert!(
        fixture
            .asset_root
            .join("audio/sfx/shared/mystery.ogg")
            .is_file()
    );

    let catalog: SemanticAudioCatalog =
        read_json(&fixture.asset_root.join("audio/catalog.json"));
    assert_eq!(catalog.assets.len(), 3);
    assert_eq!(catalog.counts.explicit_shared_fallbacks, 1);
    assert_eq!(catalog.assets[0].source_provenance.len(), 2);
    let manifest: ProjectAssetManifest =
        read_json(&fixture.asset_root.join(ASSET_MANIFEST_FILE));
    assert_eq!(manifest.files.len(), 7);
    assert_eq!(
        manifest
            .files
            .iter()
            .filter(|entry| is_original_hashed_ogg(entry))
            .count(),
        3
    );

    let second = install_semantic_audio(&options).unwrap();
    assert_eq!(second.installed_audio_files, 3);
    let manifest: ProjectAssetManifest =
        read_json(&fixture.asset_root.join(ASSET_MANIFEST_FILE));
    assert_eq!(manifest.files.len(), 7);
}

#[test]
fn byte_identity_failure_leaves_manifest_and_semantic_tree_untouched() {
    let fixture = Fixture::new(&[("Broken", "CoreShared.resourceFile", b"OggSoriginal")]);
    let manifest_path = fixture.asset_root.join(ASSET_MANIFEST_FILE);
    let before = fs::read(&manifest_path).unwrap();
    fs::write(
        fixture.asset_root.join(&fixture.sources[0].path),
        b"OggSmutated",
    )
    .unwrap();
    let error = install_semantic_audio(
        &SemanticAudioInstallOptions::new(
            &fixture.asset_root,
            &fixture.cook_report,
            "retrobution-test",
        )
        .with_expected_audio_assets(1),
    )
    .unwrap_err();
    assert!(error.to_string().contains("source byte identity mismatch"));
    assert_eq!(fs::read(manifest_path).unwrap(), before);
    for target in ["music", "ambient", "voice", "sfx", "catalog.json"] {
        assert!(!fixture.asset_root.join("audio").join(target).exists());
    }
}

#[test]
fn unmatched_cook_native_path_fails_before_staging() {
    let fixture = Fixture::new(&[("One", "CoreShared.resourceFile", b"OggSone")]);
    let mut report: serde_json::Value = read_json(&fixture.cook_report);
    for mapping in report["mappings"].as_array_mut().unwrap() {
        mapping["nativePath"] = json!("audio/different--aaaaaaaaaaaaaaaa.ogg");
    }
    fs::write(
        &fixture.cook_report,
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    let error = install_semantic_audio(
        &SemanticAudioInstallOptions::new(
            &fixture.asset_root,
            &fixture.cook_report,
            "retrobution-test",
        )
        .with_expected_audio_assets(1),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("has no exact cook mapping nativePath")
    );
    assert!(
        !fixture
            .asset_root
            .join(format!(".semantic-audio-stage-{}", std::process::id()))
            .exists()
    );
}

#[test]
fn stale_transaction_from_any_process_fails_closed() {
    let fixture = Fixture::new(&[("One", "CoreShared.resourceFile", b"OggSone")]);
    fs::create_dir(fixture.asset_root.join(".semantic-audio-stage-99999")).unwrap();
    let error = install_semantic_audio(
        &SemanticAudioInstallOptions::new(
            &fixture.asset_root,
            &fixture.cook_report,
            "retrobution-test",
        )
        .with_expected_audio_assets(1),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("stale semantic-audio transaction artifact")
    );
}

#[test]
fn character_creation_container_closure_routes_exact_speakers_music_and_sfx() {
    assert_eq!(CHARACTER_CREATION_AUDIO_ROUTES.len(), 31);
    let mut names = BTreeSet::new();
    let mut path_ids = BTreeSet::new();
    for route in CHARACTER_CREATION_AUDIO_ROUTES {
        assert!(names.insert(normalized_identity(route.true_name)));
        assert!(path_ids.insert(route.path_id));
        let provenance = character_creation_provenance(route);
        let classification = classify(route.true_name, &provenance).unwrap();
        assert_eq!(
            classification.proof.category_rule,
            "character-creation-assetbundle-container"
        );
        assert!(!classification.proof.uncertain);
        match route.kind {
            CharacterCreationAudioKind::Music => {
                assert_eq!(classification.category, SemanticAudioCategory::Music);
                assert_eq!(classification.owner, "shared");
            }
            CharacterCreationAudioKind::Voice(owner) => {
                assert_eq!(classification.category, SemanticAudioCategory::Voice);
                assert_eq!(classification.owner, owner);
            }
            CharacterCreationAudioKind::Sfx => {
                assert_eq!(classification.category, SemanticAudioCategory::Sfx);
                assert_eq!(classification.owner, "character_creation");
            }
        }
    }
}

#[test]
fn same_name_non_character_creation_variant_is_not_claimed_by_container_route() {
    let route = CHARACTER_CREATION_AUDIO_ROUTES
        .iter()
        .find(|route| route.true_name == "HologramOn")
        .unwrap();
    let provenance = vec![BTreeMap::from([
        ("bundle".to_owned(), json!("TutorialAudio.resourceFile")),
        ("pathId".to_owned(), json!(1281)),
        (
            "file".to_owned(),
            json!(
                "Tutorial.resourceFile/CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a/1281__HologramOn.ogg"
            ),
        ),
    ])];
    let classification = classify(route.true_name, &provenance).unwrap();
    assert_ne!(
        classification.proof.category_rule,
        "character-creation-assetbundle-container"
    );
    assert_eq!(classification.category, SemanticAudioCategory::Sfx);
    assert_eq!(classification.owner, "shared");
}

fn character_creation_provenance(
    route: &CharacterCreationAudioRoute,
) -> Vec<BTreeMap<String, serde_json::Value>> {
    vec![BTreeMap::from([
        ("bundle".to_owned(), json!("CharacterCreation.resourceFile")),
        ("pathId".to_owned(), json!(route.path_id)),
        (
            "file".to_owned(),
            json!(format!(
                "CharacterCreation.resourceFile/CustomAssetBundle-bd5f53480423447d7bcaed95cb2a96c8/{}__{}.ogg",
                route.path_id, route.true_name
            )),
        ),
    ])]
}

fn prepared_audio(name: &str, path: &str, hash: &str, bundle: &str) -> PreparedAudio {
    let provenance = vec![BTreeMap::from([("bundle".to_owned(), json!(bundle))])];
    PreparedAudio {
        source: ProjectAssetFile {
            source_path: path.to_owned(),
            path: path.to_owned(),
            kind: ProjectAssetKind::Audio,
            bytes: 8,
            blake3: hash.to_owned(),
        },
        true_name: name.to_owned(),
        native_key: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_owned(),
        native_path: path.to_owned(),
        classification: classify(name, &provenance).unwrap(),
        provenance,
        variant: None,
        destination: String::new(),
    }
}

struct Fixture {
    _temp: TempDir,
    asset_root: PathBuf,
    cook_report: PathBuf,
    sources: Vec<ProjectAssetFile>,
}

impl Fixture {
    fn new(specs: &[(&str, &str, &[u8])]) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let asset_root = temp.path().join("game");
        fs::create_dir_all(asset_root.join("audio")).unwrap();
        let mut sources = Vec::new();
        let mut mappings = Vec::new();
        for (index, (name, bundle, bytes)) in specs.iter().enumerate() {
            let hash = blake3::hash(bytes).to_hex().to_string();
            let slug = portable_component(name, "fixture").unwrap();
            let path = format!("audio/{slug}--{}.ogg", &hash[..16]);
            fs::write(asset_root.join(&path), bytes).unwrap();
            sources.push(ProjectAssetFile {
                source_path: path.clone(),
                path: path.clone(),
                kind: ProjectAssetKind::Audio,
                bytes: bytes.len() as u64,
                blake3: hash,
            });
            let native_key = format!("{:064x}", index + 1);
            mappings.push(json!({
                "kind": "audio",
                "name": name,
                "nativeKey": native_key,
                "nativePath": path,
                "source": {
                    "bundle": bundle,
                    "context": "AudioClip",
                    "pathId": index + 1
                }
            }));
            mappings.push(json!({
                "kind": "audio",
                "name": name,
                "nativeKey": native_key,
                "nativePath": path,
                "source": {
                    "bundle": bundle,
                    "file": format!("{bundle}/{index}__{name}.ogg"),
                    "pathId": index + 1
                }
            }));
        }
        let manifest = ProjectAssetManifest {
            schema: PROJECT_ASSET_SCHEMA.to_owned(),
            protocol: 104,
            locale: "ru-RU".to_owned(),
            source_pack: SourcePackIdentity {
                schema: "fixture".to_owned(),
                manifest_blake3: "fixture".to_owned(),
            },
            files: sources.clone(),
        };
        fs::write(
            asset_root.join(ASSET_MANIFEST_FILE),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let cook_report = temp.path().join("cook-report.json");
        fs::write(
            &cook_report,
            serde_json::to_vec_pretty(&json!({
                "schema": COOK_REPORT_SCHEMA,
                "buildUuid": "fixture-build",
                "locale": "ru-RU",
                "counts": { "audio": specs.len() },
                "mappings": mappings
            }))
            .unwrap(),
        )
        .unwrap();
        Self {
            _temp: temp,
            asset_root,
            cook_report,
            sources,
        }
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
