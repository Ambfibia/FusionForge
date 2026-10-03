use super::*;
#[test]
fn accepted_banker_normals_are_scoped_and_check_current_coordinates() {
    let mesh = json!({"m_CompressedMesh":{"m_Normals":{"m_BitSize":8,"m_NumItems":2,"m_Range":1.0,"m_Start":0.0,"m_Data":{"base64":STANDARD.encode([153,0])}},"m_NormalSigns":{"m_BitSize":1,"m_NumItems":1,"m_Data":{"base64":STANDARD.encode([1])}}}});
    let values = accepted_normals(&mesh, &json!([-0.6, 0.0, 0.8])).unwrap();
    assert!((values[2].as_f64().unwrap() - 0.64).abs() < 1e-12);
    assert!(accepted_normals(&mesh, &json!([0.6, 0.0, 0.8])).is_err());
}
#[test]
fn full_mip_and_sampler_contract_required() {
    let a = json!({"colorSpace":"srgb","sampler":{"descriptor":{"name":"a","wrapS":"repeat"}},"mipLevels":[{"width":2,"height":2,"pngSha256":"base"},{"width":1,"height":1,"pngSha256":"lower"}]});
    let mut b = a.clone();
    b["sampler"]["descriptor"]["name"] = json!("b");
    assert_eq!(signature(&a).unwrap(), signature(&b).unwrap());
    b["mipLevels"][1]["pngSha256"] = json!("different");
    assert_ne!(signature(&a).unwrap(), signature(&b).unwrap());
    b = a.clone();
    b["sampler"]["descriptor"]["wrapS"] = json!("clamp");
    assert_ne!(signature(&a).unwrap(), signature(&b).unwrap());
}
