use super::*;

#[test]
fn audio_payload_aliases_match_and_missing_or_empty_payloads_fail_closed() {
    let pointer = Pointer {
        source_asset: 0,
        file_id: 0,
        path_id: 0,
    };
    let legacy = test_audio_value("audio data", b"OggS-payload", pointer.clone());
    let modern = test_audio_value("m_AudioData", b"OggS-payload", pointer);
    assert_eq!(
        audio_object_identity(&legacy).unwrap().payload_sha1,
        audio_object_identity(&modern).unwrap().payload_sha1
    );

    let missing = UnityValue::Object(BTreeMap::from([(
        "m_Name".to_string(),
        UnityValue::String("missing".to_string()),
    )]));
    assert!(audio_object_identity(&missing)
        .unwrap_err()
        .contains("neither"));

    let empty = test_audio_value(
        "audio data",
        b"",
        Pointer {
            source_asset: 0,
            file_id: 0,
            path_id: 0,
        },
    );
    assert!(audio_object_identity(&empty).unwrap_err().contains("empty"));

    // Original Unity 2.x bundles sometimes use m_Size for a legacy codec-specific
    // quantity rather than the embedded OGG byte count. The full semantic hash preserves
    // that metadata, while payload SHA-1 verifies the actual sound bytes.
    let mut legacy_size = legacy;
    legacy_size
        .as_object_mut()
        .unwrap()
        .insert("m_Size".to_string(), UnityValue::Int(1));
    assert!(audio_object_identity(&legacy_size).is_ok());
}

#[test]
fn layout_cache_accepts_only_flat_bundle_payload_names() {
    assert!(safe_layout_cache_name("CoreShared.resourceFile"));
    assert!(safe_layout_cache_name("main.unity3d"));
    assert!(!safe_layout_cache_name(""));
    assert!(!safe_layout_cache_name("receipt.json"));
    assert!(!safe_layout_cache_name("nested/CoreShared.resourceFile"));
    assert!(!safe_layout_cache_name("../CoreShared.resourceFile"));
}

#[test]
fn layout_cache_plan_rejects_duplicate_payload_names() {
    let mut plan = disabled_plan();
    plan.output_bundles = vec!["CoreShared.resourceFile".to_string()];
    plan.rewritten_retained_bundles = vec!["CoreShared.resourceFile".to_string()];
    assert!(cached_plan_file_names(&plan)
        .unwrap_err()
        .contains("duplicate filename"));
}
