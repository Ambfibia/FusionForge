use super::*;
use std::collections::BTreeMap;

#[test]
fn argb4444_and_rgba4444_keep_their_distinct_channel_order() {
    let source = [0x23, 0xf1];
    assert_eq!(decode_argb4444(1, 1, &source).unwrap(), [17, 34, 51, 255]);
    assert_eq!(decode_rgba4444(1, 1, &source).unwrap(), [255, 17, 34, 51]);
}

#[test]
fn complete_rgba32_mip_chain_is_split_without_dropping_source_bytes() {
    let base = (0_u8..16).collect::<Vec<_>>();
    let tail = vec![21_u8, 22, 23, 24];
    let mut encoded = base.clone();
    encoded.extend_from_slice(&tail);
    let texture = UnityValue::Object(BTreeMap::from([
        ("m_Width".to_string(), UnityValue::Int(2)),
        ("m_Height".to_string(), UnityValue::Int(2)),
        ("m_TextureFormat".to_string(), UnityValue::Int(4)),
        ("m_MipMap".to_string(), UnityValue::Bool(true)),
        ("image data".to_string(), UnityValue::Bytes(encoded)),
    ]));
    let env = UnityEnvironment::from_assets(Vec::new());
    let mips = decode_texture_mips(&env, &texture).unwrap();
    assert_eq!(mips.len(), 2);
    assert_eq!((mips[0].width, mips[0].height), (2, 2));
    assert_eq!(mips[0].source_encoded, base);
    assert_eq!((mips[1].width, mips[1].height), (1, 1));
    assert_eq!(mips[1].source_encoded, tail);
}

#[test]
fn exact_mip_chain_rejects_a_trailing_source_byte() {
    let mut encoded = vec![0_u8; 20];
    encoded.push(0xff);
    let texture = UnityValue::Object(BTreeMap::from([
        ("m_Width".to_string(), UnityValue::Int(2)),
        ("m_Height".to_string(), UnityValue::Int(2)),
        ("m_TextureFormat".to_string(), UnityValue::Int(4)),
        ("m_MipMap".to_string(), UnityValue::Bool(true)),
        ("m_CompleteImageSize".to_string(), UnityValue::Int(21)),
        ("image data".to_string(), UnityValue::Bytes(encoded)),
    ]));
    let env = UnityEnvironment::from_assets(Vec::new());
    let error = decode_texture_mips_exact(&env, &texture).unwrap_err();
    assert!(error.contains("consumes 20 bytes"));
    assert!(error.contains("contains 21 bytes"));
}
