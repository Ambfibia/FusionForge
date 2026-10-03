use super::*;
#[test]
fn packed_stream_rejects_truncation_instead_of_inventing_zero_bits() {
    assert!(packed(&json!({"m_NumItems":9,"m_BitSize":1,"m_Data":{"base64":"AA=="},"m_Start":0,"m_Range":1})).is_err());
    let v=packed(&json!({"m_NumItems":2,"m_BitSize":4,"m_Data":{"base64":"8A=="},"m_Start":-1,"m_Range":2})).unwrap();
    assert_eq!(v, vec![-1.0, 1.0]);
}
