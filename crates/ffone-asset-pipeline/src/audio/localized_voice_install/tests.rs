use super::*;
use tempfile::tempdir;

#[test]
fn stale_transaction_from_any_previous_process_fails_closed() {
    for (name, directory) in [
        (".localized-voice-stage-123-456", true),
        (".localized-voice-backup-123-456", true),
        (".asset-manifest.localized-voice-next-123-456", false),
        (".asset-manifest.localized-voice-backup-123-456", false),
    ] {
        let root = tempdir().unwrap();
        let artifact = root.path().join(name);
        if directory {
            fs::create_dir(&artifact).unwrap();
        } else {
            fs::write(&artifact, b"stale").unwrap();
        }
        let error = reject_stale_transactions(root.path())
            .expect_err("stale transaction artifact must fail closed")
            .to_string();
        assert!(error.contains("stale localized-voice transaction artifact"));
        assert!(error.contains(name));
    }
}

fn asset(
    true_name: &str,
    native_key: &str,
    container: &str,
    path_id: u64,
) -> LocalizedAudioAsset {
    LocalizedAudioAsset {
        true_name: true_name.to_owned(),
        path: format!("audio/voice/shared/{}.ogg", folded(true_name)),
        category: SemanticAudioCategory::Voice,
        owner: "shared".to_owned(),
        classification: SemanticAudioClassificationProof {
            category_rule: "test".to_owned(),
            owner_rule: "test".to_owned(),
            certainty: ClassificationCertainty::SourceProvenance,
            evidence: vec!["test".to_owned()],
            reason: "test".to_owned(),
            uncertain: false,
        },
        variant: None,
        source_manifest_path: "audio/source.ogg".to_owned(),
        source_manifest_source_path: "audio/source.ogg".to_owned(),
        source_bytes: 8,
        source_blake3: "0".repeat(64),
        native_key: native_key.to_owned(),
        native_path: "audio/source.ogg".to_owned(),
        source_provenance: vec![BTreeMap::from([
            (
                "file".to_owned(),
                serde_json::Value::String(format!(
                    "Bundle.resourceFile/{container}/{path_id}__{true_name}.ogg"
                )),
            ),
            (
                "pathId".to_owned(),
                serde_json::Value::Number(path_id.into()),
            ),
        ])],
        locale_variants: Vec::new(),
    }
}

fn source(container: &str, path_id: u64, true_name: &str) -> RussianSourceFile {
    RussianSourceFile {
        absolute_path: PathBuf::from(format!("{path_id}__{true_name}.ogg")),
        relative_path: format!("{container}/{path_id}__{true_name}.ogg"),
        container: Some(container.to_owned()),
        path_id: Some(path_id),
        true_name: Some(true_name.to_owned()),
    }
}

#[test]
fn strict_primary_then_casefold_fallback_and_explicit_gaps() {
    let assets = vec![
        asset("Dexter_CCHello", "a", "CustomAssetBundle-one", 10),
        asset("Ben_QGreeting", "b", "CustomAssetBundle-two", 20),
        asset("Ben_QGreeting", "c", "CustomAssetBundle-two", 21),
    ];
    let sources = vec![
        source("CustomAssetBundle-one", 10, "Dexter_CCHello"),
        source("customassetbundle-one", 999, "dexter_cchello"),
        source("CustomAssetBundle-two", 999, "BEN_QGREETING"),
        source("CustomAssetBundle-missing", 1, "Nobody"),
    ];
    let (matched, unmatched, ambiguous) = match_russian_sources(&assets, sources).unwrap();
    assert_eq!(matched.len(), 2);
    assert_eq!(matched[0].match_mode, LocalizedVoiceMatchMode::Primary);
    assert_eq!(
        matched[1].match_mode,
        LocalizedVoiceMatchMode::ContainerCasefoldNameFallback
    );
    assert_eq!(unmatched.len(), 1);
    assert_eq!(ambiguous.len(), 1);
    assert_eq!(ambiguous[0].candidate_native_keys, ["b", "c"]);
}

#[test]
fn repeated_ogg_suffix_is_not_silently_repaired() {
    let parsed = parse_ogg_filename("597__M_UrbRgr5_Armor_Greeting02.ogg.ogg").unwrap();
    assert_eq!(parsed.0, 597);
    assert_eq!(parsed.1, "M_UrbRgr5_Armor_Greeting02.ogg");
}
