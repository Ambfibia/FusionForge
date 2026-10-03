use super::*;

#[test]
fn gui_language_dictionary_entries_are_not_exported_or_saved() {
    assert!(!should_export_unity_field_path(
        "m_pGUILanguageTable.m_pGUILanguageData[100].m_String[0]"
    ));

    let mut document = json!({
        "entries": [
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pGUILanguageTable.m_pGUILanguageData[100].m_String[0]",
                "source": "Common",
                "translation": "Обычный"
            },
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pNpcTable.m_pNpcData[0].szName",
                "source": "Otto",
                "translation": "Отто"
            }
        ]
    });

    normalize_translation_document(&mut document);

    let entries = document["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["source"], json!("Otto"));
}

#[test]
fn normalize_translation_document_repairs_unity_mojibake() {
    let mojibake = "\u{0420}\u{045b}\u{0421}\u{201a}\u{0421}\u{201a}\u{0420}\u{0455}";
    let mut document = json!({
        "entries": [
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pNpcTable.m_pNpcData[0].szName",
                "source": "Otto",
                "translation": mojibake
            }
        ]
    });

    normalize_translation_document(&mut document);

    let entries = document["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["translation"], json!("Отто"));
}

#[test]
fn compact_translation_document_keeps_only_original_and_translation() {
    let document = json!({
        "entries": [
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pNpcTable.m_pNpcData[0].szName",
                "source": "Otto",
                "translation": "Otto RU"
            },
            {
                "kind": "managed.string",
                "token": "0x70000001",
                "source": "Hello",
                "translation": ""
            }
        ]
    });

    let compact = compact_translation_document(&document).expect("compact document");
    let entries = compact["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0], json!({"original": "Hello", "translation": ""}));
    assert_eq!(
        entries[1],
        json!({"original": "Otto", "translation": "Otto RU"})
    );
}

#[test]
fn compact_translation_document_skips_translated_source_duplicates() {
    let document = json!({
        "entries": [
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pMissionTable.m_pMissionStringData[2050].m_pstrNameString",
                "source": "\"Anubis\" has a nice ring to it.",
                "translation": "Anubis RU"
            },
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pMissionTable.m_pMissionStringData[2050].m_pstrNameString",
                "source": "Anubis RU",
                "translation": ""
            }
        ]
    });

    let compact = compact_translation_document(&document).expect("compact document");
    let entries = compact["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0],
        json!({
            "original": "\"Anubis\" has a nice ring to it.",
            "translation": "Anubis RU"
        })
    );
}

#[test]
fn sanitize_translation_entries_removes_only_exact_translated_source_duplicates() {
    let mut entries = vec![
        json!({
            "id": "unity.objectString:TableData.resourceFile:file:7:english",
            "kind": "unity.objectString",
            "container": "TableData.resourceFile",
            "file": "CustomAssetBundle-TableData",
            "pathId": 7,
            "fieldPath": "m_pMissionTable.m_pMissionStringData[2050].m_pstrNameString",
            "source": "\"Anubis\" has a nice ring to it.",
            "translation": "Anubis RU"
        }),
        json!({
            "id": "unity.objectString:TableData.resourceFile:file:7:russian",
            "kind": "unity.objectString",
            "container": "TableData.resourceFile",
            "file": "CustomAssetBundle-TableData",
            "pathId": 7,
            "fieldPath": "m_pMissionTable.m_pMissionStringData[2050].m_pstrNameString",
            "source": "Anubis RU",
            "translation": ""
        }),
        json!({
            "id": "unity.objectString:TableData.resourceFile:file:7:other",
            "kind": "unity.objectString",
            "container": "TableData.resourceFile",
            "file": "CustomAssetBundle-TableData",
            "pathId": 7,
            "fieldPath": "m_pMissionTable.m_pMissionStringData[2051].m_pstrNameString",
            "source": "Anubis RU",
            "translation": ""
        }),
    ];

    sanitize_translation_entries(&mut entries);

    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0]["source"],
        json!("\"Anubis\" has a nice ring to it.")
    );
    assert_eq!(entries[0]["translation"], json!("Anubis RU"));
    assert_eq!(
        entries[1]["fieldPath"],
        json!("m_pMissionTable.m_pMissionStringData[2051].m_pstrNameString")
    );
}

#[test]
fn sanitize_translation_entries_removes_asset_names_and_internal_ids() {
    let mut entries = vec![
        json!({
            "kind": "unity.objectString",
            "fieldPath": "m_pHelpTable.m_pHelpPageString[382].m_strComment",
            "source": "equip.jpg",
            "translation": ""
        }),
        json!({
            "kind": "unity.objectString",
            "fieldPath": "m_pSkillTable.m_pSkillStringData[160].m_strComment",
            "source": "freedom_M",
            "translation": ""
        }),
        json!({
            "kind": "unity.objectString",
            "fieldPath": "m_pFirstUseTable.m_pFirstUseString[11].m_strComment2",
            "source": "fu_computress",
            "translation": ""
        }),
        json!({
            "kind": "unity.objectString",
            "fieldPath": "m_pMissionTable.m_pMissionStringData[2050].m_pstrNameString",
            "source": "Real visible text.",
            "translation": ""
        }),
    ];

    sanitize_translation_entries(&mut entries);

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["source"], json!("Real visible text."));
}

#[test]
fn sanitize_translation_entries_removes_shiny_internal_names_and_managed_code() {
    let long_quest_placeholder = "onetwothree".repeat(45);
    let mut entries = vec![
        json!({
            "kind": "unity.objectString",
            "fieldPath": "m_pShinyTable.m_pShinyStringData[58].m_strName",
            "source": "poison1",
            "translation": "яд1"
        }),
        json!({
            "kind": "unity.objectString",
            "fieldPath": "m_pShinyTable.m_pShinyStringData[103].m_strName",
            "source": "jump*3_10",
            "translation": ""
        }),
        json!({
            "kind": "managed.ldstr",
            "source": "mClickItem.ItemType != (int)eItemType.eItemType_Vehicle",
            "translation": ""
        }),
        json!({
            "kind": "managed.ldstr",
            "source": "ownstatus.iChangeGuideCount * iTargetLv * (int)eMissionConst.MENTOR_CHANGE_BASE_COST) ",
            "translation": ""
        }),
        json!({
            "kind": "unity.objectString",
            "fieldPath": "m_pNpcTable.m_pNpcStringData[3050].m_strName",
            "source": "parrot",
            "translation": ""
        }),
        json!({
            "kind": "unity.objectString",
            "fieldPath": "m_pNpcTable.m_pNpcStringData[3051].m_strName",
            "source": "parrot2",
            "translation": ""
        }),
        json!({
            "kind": "unity.objectString",
            "fieldPath": "m_pMissionTable.m_pMissionStringData[11033].m_pstrNameString",
            "source": long_quest_placeholder,
            "translation": ""
        }),
    ];

    sanitize_translation_entries(&mut entries);

    let sources = entries
        .iter()
        .filter_map(|entry| entry.get("source").and_then(JsonValue::as_str))
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 3);
    assert!(sources.contains(&"parrot"));
    assert!(sources.contains(&"parrot2"));
    assert!(sources
        .iter()
        .any(|source| source.starts_with("onetwothree")));
}

#[test]
fn compact_translation_document_skips_empty_cyrillic_sources() {
    let document = json!({
        "entries": [
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pMissionTable.m_pMissionStringData[2051].m_pstrNameString",
                "source": "Берегись! Это уже переведенный текст.",
                "translation": ""
            },
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pMissionTable.m_pMissionStringData[2052].m_pstrNameString",
                "source": "Still needs translation.",
                "translation": ""
            }
        ]
    });

    let compact = compact_translation_document(&document).expect("compact document");
    let entries = compact["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0],
        json!({"original": "Still needs translation.", "translation": ""})
    );
}

#[test]
fn load_compact_translation_document_merges_by_original_text() {
    let temp = std::env::temp_dir().join(format!(
        "ffclienteditor-compact-translation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).expect("create temp");
    let compact_path = temp.join("translation.compact.json");
    fs::write(
        &compact_path,
        serde_json::to_string_pretty(&json!({
            "format": "fftools.translation.compact.v1",
            "entries": [
                {"original": "Otto", "translation": "Otto RU"},
                {"original": "Dexlabs", "translation": "Dexlabs RU"}
            ]
        }))
        .expect("json"),
    )
    .expect("write compact");
    let base = json!({
        "entries": [
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pNpcTable.m_pNpcData[0].szName",
                "source": "Otto",
                "translation": ""
            },
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pNpcTable.m_pNpcData[1].szName",
                "source": "Dexlabs",
                "translation": "Old"
            },
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pNpcTable.m_pNpcData[2].szName",
                "source": "Computress",
                "translation": ""
            }
        ]
    });

    let result =
        load_compact_translation_document(compact_path.to_string_lossy().to_string(), base)
            .expect("load compact");
    assert_eq!(result["entries"], json!(2));
    assert_eq!(result["updated"], json!(2));
    let entries = result["document"]["entries"].as_array().expect("entries");
    assert_eq!(entries[0]["translation"], json!("Otto RU"));
    assert_eq!(entries[1]["translation"], json!("Dexlabs RU"));
    assert_eq!(entries[2]["translation"], json!(""));
    let _ = fs::remove_dir_all(&temp);
}
