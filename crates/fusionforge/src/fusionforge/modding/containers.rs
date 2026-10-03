use super::*;

pub(super) fn packed_float_object(
    count: i64,
    bit_size: u32,
    start: f64,
    range: f64,
    data: Vec<u8>,
) -> BTreeMap<String, UnityValue> {
    BTreeMap::from([
        ("m_NumItems".to_string(), UnityValue::Int(count)),
        ("m_Range".to_string(), UnityValue::Float(range)),
        ("m_Start".to_string(), UnityValue::Float(start)),
        ("m_Data".to_string(), UnityValue::Bytes(data)),
        ("m_BitSize".to_string(), UnityValue::Int(bit_size as i64)),
    ])
}

pub(super) fn vector3_object(value: (f64, f64, f64)) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        ("x".to_string(), UnityValue::Float(value.0)),
        ("y".to_string(), UnityValue::Float(value.1)),
        ("z".to_string(), UnityValue::Float(value.2)),
    ]))
}
