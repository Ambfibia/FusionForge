use super::*;

#[test]
fn tabledata_internal_fields_are_not_exported_or_saved() {
    for field_path in [
        "m_pFilterTable.m_pWhiteFilterData[1].m_strText",
        "m_pFilterTable.m_pBlackFilterData[1].m_strText",
        "m_pFilterTable.m_pNameFilterData[1].m_strText",
        "m_pHelpTable.m_pHelpPageString[1].m_strComment",
        "m_pHelpTable.m_pHelpPageString[1].m_strComment1",
        "m_pHelpTable.m_pHelpPageString[1].m_strComment2",
        "m_pFirstUseTable.m_pFirstUseString[1].m_strComment1",
        "m_pFirstUseTable.m_pFirstUseString[1].m_strComment2",
        "m_pRulesTable.m_pRulesString[1].m_strComment",
        "m_pRulesTable.m_pRulesString[1].m_strComment1",
        "m_pRulesTable.m_pRulesString[1].m_strComment2",
    ] {
        assert!(
            !should_export_unity_field_path(field_path),
            "{field_path} should not be exported"
        );
    }

    let mut document = json!({
        "entries": [
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pFilterTable.m_pWhiteFilterData[1].m_strText",
                "source": "assist",
                "translation": "assist ru"
            },
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pHelpTable.m_pHelpPageString[1].m_strComment1",
                "source": "Caption",
                "translation": "caption ru"
            },
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pFirstUseTable.m_pFirstUseString[1].m_strComment1",
                "source": "Computress_FirstUse01",
                "translation": "sound ru"
            },
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pRulesTable.m_pRulesString[1].m_strComment",
                "source": "rule_vehicle_1",
                "translation": "icon ru"
            },
            {
                "kind": "unity.objectString",
                "fieldPath": "m_pHelpTable.m_pHelpPageString[1].m_strName",
                "source": "Visible help text.",
                "translation": "visible ru"
            }
        ]
    });

    normalize_translation_document(&mut document);

    let entries = document["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0]["fieldPath"],
        json!("m_pHelpTable.m_pHelpPageString[1].m_strName")
    );
}
