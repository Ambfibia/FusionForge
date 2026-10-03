use super::*;

#[test]
fn voice_owner_keeps_gender_and_archetype_together() {
    assert_eq!(
        infer_voice_owner("F_Pirate2_Gooluck01").unwrap(),
        "f_pirate2"
    );
    assert_eq!(
        infer_voice_owner("M_Banker1_Greeting02").unwrap(),
        "m_banker1"
    );
    assert_eq!(infer_voice_owner("Dexter_CCHello").unwrap(), "dexter");
}

#[test]
fn strict_catalog_rejects_inferred_variant_directories() {
    let catalog = StrictAudioCatalog {
        schema: STRICT_AUDIO_CATALOG_SCHEMA.to_owned(),
        fallback_locale: "en".to_owned(),
        locale_fallbacks: BTreeMap::from([("ru".to_owned(), "en".to_owned())]),
        counts: StrictAudioCatalogCounts {
            assets: 1,
            files: 1,
            file_bytes: 4,
            music: 0,
            ambient: 0,
            voice: 1,
            sfx: 0,
            english_voice_files: 1,
            russian_voice_files: 0,
            translated_voice_assets: 0,
            fallback_only_voice_assets: 1,
            language_neutral_voice_assets: 0,
        },
        assets: vec![StrictAudioAsset {
            logical_key: "voice/dexter/dexter_cchello".to_owned(),
            aliases: Vec::new(),
            true_name: "Dexter_CCHello".to_owned(),
            category: SemanticAudioCategory::Voice,
            owner: "dexter".to_owned(),
            scope: None,
            files: vec![StrictAudioFile {
                locale: Some("en".to_owned()),
                path: "audio/voice/en/dexter/variants/variant_01/dexter_cchello.ogg".to_owned(),
                bytes: 4,
                blake3: "a".repeat(64),
                language_neutral: false,
            }],
        }],
    };
    assert!(validate_strict_catalog(&catalog).is_err());
}

#[test]
fn strict_catalog_rejects_equal_localized_payloads() {
    let hash = "a".repeat(64);
    let mut asset = StrictAudioAsset {
        logical_key: "voice/ben/ben_tut01".to_owned(),
        aliases: Vec::new(),
        true_name: "Ben_Tut01".to_owned(),
        category: SemanticAudioCategory::Voice,
        owner: "ben".to_owned(),
        scope: None,
        files: vec![
            StrictAudioFile {
                locale: Some("en".to_owned()),
                path: "audio/voice/en/ben/ben_tut01.ogg".to_owned(),
                bytes: 4,
                blake3: hash.clone(),
                language_neutral: false,
            },
            StrictAudioFile {
                locale: Some("ru".to_owned()),
                path: "audio/voice/ru/ben/ben_tut01.ogg".to_owned(),
                bytes: 4,
                blake3: hash,
                language_neutral: false,
            },
        ],
    };
    let counts = strict_catalog_counts(std::slice::from_ref(&asset)).unwrap();
    let catalog = StrictAudioCatalog {
        schema: STRICT_AUDIO_CATALOG_SCHEMA.to_owned(),
        fallback_locale: "en".to_owned(),
        locale_fallbacks: BTreeMap::from([("ru".to_owned(), "en".to_owned())]),
        counts,
        assets: vec![asset.clone()],
    };
    assert!(validate_strict_catalog(&catalog).is_err());
    for file in &mut asset.files {
        file.language_neutral = true;
    }
    let counts = strict_catalog_counts(std::slice::from_ref(&asset)).unwrap();
    let catalog = StrictAudioCatalog {
        schema: STRICT_AUDIO_CATALOG_SCHEMA.to_owned(),
        fallback_locale: "en".to_owned(),
        locale_fallbacks: BTreeMap::from([("ru".to_owned(), "en".to_owned())]),
        counts,
        assets: vec![asset],
    };
    assert!(validate_strict_catalog(&catalog).is_ok());
}

#[test]
fn runtime_v5_serialization_omits_audio_integrity_fields() {
    let asset = StrictAudioAsset {
        logical_key: "voice/ben/ben_tut01".to_owned(),
        aliases: Vec::new(),
        true_name: "Ben_Tut01".to_owned(),
        category: SemanticAudioCategory::Voice,
        owner: "ben".to_owned(),
        scope: None,
        files: vec![StrictAudioFile {
            locale: Some("en".to_owned()),
            path: "audio/voice/en/ben/ben_tut01.ogg".to_owned(),
            bytes: 123,
            blake3: "a".repeat(64),
            language_neutral: false,
        }],
    };
    let catalog = StrictAudioCatalog {
        schema: STRICT_AUDIO_CATALOG_SCHEMA.to_owned(),
        fallback_locale: "en".to_owned(),
        locale_fallbacks: BTreeMap::new(),
        counts: strict_catalog_counts(std::slice::from_ref(&asset)).unwrap(),
        assets: vec![asset],
    };
    let json =
        String::from_utf8(serialize_editable_runtime_catalog(&catalog).unwrap()).unwrap();
    assert!(json.contains(STRICT_AUDIO_CATALOG_SCHEMA));
    for forbidden in ["\"bytes\"", "\"blake3\"", "\"fileBytes\""] {
        assert!(!json.contains(forbidden), "found {forbidden} in {json}");
    }
}
