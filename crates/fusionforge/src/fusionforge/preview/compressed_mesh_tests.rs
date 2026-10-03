use super::primary_compressed_uv_channel;

#[test]
fn packed_normals_reconstruct_signed_unit_z_and_quantization_overflow() {
    use super::*;
    use std::collections::BTreeMap;
    let packed = |data:Vec<u8>| UnityValue::Object(BTreeMap::from([
        ("m_NumItems".into(),UnityValue::Int(data.len() as i64)),
        ("m_BitSize".into(),UnityValue::Int(8)),
        ("m_Data".into(),UnityValue::Bytes(data)),
        ("m_Range".into(),UnityValue::Float(1.0)),
        ("m_Start".into(),UnityValue::Float(0.0)),
    ]));
    let values=read_normals(&packed(vec![153,0,153,0,255,255]),&packed(vec![1,0,1]));
    assert!((values[2]-0.8).abs()<1e-12);
    assert!((values[5]+0.8).abs()<1e-12);
    for normal in values.chunks_exact(3) {
        assert!((normal.iter().map(|v|v*v).sum::<f64>()-1.0).abs()<1e-12);
    }
}

#[test]
fn primary_uv_channel_excludes_appended_secondary_channel() {
    let packed_uvs = [
        0.0, 0.1, 0.2, 0.3, // UV0 for two vertices
        0.4, 0.5, 0.6, 0.7, // UV1 for the same vertices
    ];

    assert_eq!(
        primary_compressed_uv_channel(&packed_uvs, 2),
        &[0.0, 0.1, 0.2, 0.3]
    );
}
