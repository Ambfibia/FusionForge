use super::*;

pub(super) fn test_audio_value(payload_field: &str, payload: &[u8], pointer: Pointer) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        (
            payload_field.to_string(),
            UnityValue::Bytes(payload.to_vec()),
        ),
        (
            "m_Name".to_string(),
            UnityValue::String("voice_exact".to_string()),
        ),
        ("m_Format".to_string(), UnityValue::Int(1)),
        ("m_Frequency".to_string(), UnityValue::Int(44_100)),
        ("m_Length".to_string(), UnityValue::Float(1.25)),
        ("m_Size".to_string(), UnityValue::Int(payload.len() as i64)),
        ("testPointer".to_string(), UnityValue::Pointer(pointer)),
    ]))
}

#[test]
fn semantic_audio_hash_ignores_local_pointer_ids_but_preserves_exact_content() {
    let first = test_audio_value(
        "audio data",
        b"OggS-content",
        Pointer {
            source_asset: 1,
            file_id: 2,
            path_id: 3,
        },
    );
    let second = test_audio_value(
        "audio data",
        b"OggS-content",
        Pointer {
            source_asset: 99,
            file_id: 77,
            path_id: 55,
        },
    );
    assert_eq!(
        semantic_value_sha1("AudioClip", &first),
        semantic_value_sha1("AudioClip", &second)
    );

    let mut changed_payload = second.clone();
    let UnityValue::Bytes(bytes) = changed_payload.get_mut("audio data").unwrap() else {
        panic!("test AudioClip payload is not bytes");
    };
    *bytes.last_mut().unwrap() ^= 1;
    assert_ne!(
        semantic_value_sha1("AudioClip", &first),
        semantic_value_sha1("AudioClip", &changed_payload)
    );

    let mut changed_name = second;
    changed_name.as_object_mut().unwrap().insert(
        "m_Name".to_string(),
        UnityValue::String("Voice_exact".to_string()),
    );
    assert_ne!(
        semantic_value_sha1("AudioClip", &first),
        semantic_value_sha1("AudioClip", &changed_name)
    );
}

#[test]
fn classifier_separates_audio_domains() {
    let cfg = cfg();
    assert_eq!(
        classify_root(
            "vo/blossom_001.wav",
            "DongResources_05_04.resourceFile",
            &cfg
        ),
        Family::NpcVoice
    );
    assert_eq!(
        classify_root("tut sound/tutorial_001.wav", "Tutorial.resourceFile", &cfg),
        Family::TutorialAudio
    );
    assert_eq!(
        classify_root("ui sound/click.wav", "CharacterCreation.resourceFile", &cfg),
        Family::UiAudio
    );
    assert_eq!(
        classify_root("sound/nano_attack.wav", "FutureNano.resourceFile", &cfg),
        Family::Nano
    );
}
