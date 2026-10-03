use super::*;

#[test]
fn plain_curve_keeps_hermite_slopes_and_tangent_mode() {
    use fusionforge::UnityValue::{Array, Float, Int, Object, String as UnityString};

    let vec3 = |x, y, z| {
        Object(BTreeMap::from([
            ("x".to_string(), Float(x)),
            ("y".to_string(), Float(y)),
            ("z".to_string(), Float(z)),
        ]))
    };
    let curve = Object(BTreeMap::from([
        ("path".to_string(), UnityString("Bip01/Pelvis".to_string())),
        (
            "curve".to_string(),
            Object(BTreeMap::from([(
                "m_Curve".to_string(),
                Array(vec![Object(BTreeMap::from([
                    ("time".to_string(), Float(0.25)),
                    ("value".to_string(), vec3(1.0, 2.0, 3.0)),
                    ("inSlope".to_string(), vec3(4.0, 5.0, 6.0)),
                    ("outSlope".to_string(), vec3(7.0, 8.0, 9.0)),
                    ("tangentMode".to_string(), Int(17)),
                ]))]),
            )])),
        ),
    ]));
    let convert = |value: &fusionforge::UnityValue| {
        let value = unity_vec3(Some(value), [f64::NAN; 3]);
        value.iter().all(|part| part.is_finite()).then_some(value)
    };
    let (path, samples) =
        decode_plain_curve::<[f64; 3], _, _>(&curve, convert, convert).expect("curve");

    assert_eq!(path, "Bip01/Pelvis");
    assert_eq!(samples.keys[0]["value"], json!([1.0, 2.0, 3.0]));
    assert_eq!(samples.keys[0]["inTangent"], json!([4.0, 5.0, 6.0]));
    assert_eq!(samples.keys[0]["outTangent"], json!([7.0, 8.0, 9.0]));
    assert_eq!(samples.keys[0]["tangentMode"], json!(17));
    assert_eq!(samples.keys[0]["sourceKeyIndex"], json!(0));
    assert!(samples.duplicate_keys.is_empty());
    assert_eq!(samples.source_key_count, 1);
}
