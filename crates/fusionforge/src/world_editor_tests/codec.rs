use super::*;

#[test]
fn exact_kfm_payload_accepts_only_real_textasset_bytes_with_nif_references() {
    let bytes = b"mob/body.nif\0anim/idle.kf\0".to_vec();
    let shader = fusionforge::UnityValue::Object(BTreeMap::from([(
        "m_Script".to_string(),
        fusionforge::UnityValue::Bytes(bytes.clone()),
    )]));
    assert!(exact_kfm_text_asset_payload("Shader", &shader).is_none());

    let text_asset = fusionforge::UnityValue::Object(BTreeMap::from([(
        "m_Script".to_string(),
        fusionforge::UnityValue::Bytes(bytes.clone()),
    )]));
    assert_eq!(
        exact_kfm_text_asset_payload("TextAsset", &text_asset),
        Some(bytes)
    );

    let unrelated_text = fusionforge::UnityValue::Object(BTreeMap::from([(
        "m_Script".to_string(),
        fusionforge::UnityValue::Bytes(b"ShaderLab source".to_vec()),
    )]));
    assert!(exact_kfm_text_asset_payload("TextAsset", &unrelated_text).is_none());
}

#[test]
fn encode_dxt3_rgba_mip_chain_matches_legacy_512_size() {
    let rgba = vec![255_u8; 512 * 512 * 4];
    let (dxt, mip_count) = encode_dxt3_rgba_mip_chain(512, 512, &rgba).expect("mip chain");
    assert_eq!(mip_count, 10);
    assert_eq!(dxt.len(), 349_552);
}
