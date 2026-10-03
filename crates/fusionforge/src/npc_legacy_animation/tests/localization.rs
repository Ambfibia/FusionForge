use super::*;

pub(super) fn duplicate_translation_regression_fixture(conflicting: bool) -> (JsonValue, JsonValue) {
    use fusionforge::UnityValue::{Array, Float, Object, String as UnityString};

    let vec3 = |x| {
        Object(BTreeMap::from([
            ("x".to_string(), Float(x)),
            ("y".to_string(), Float(2.0)),
            ("z".to_string(), Float(3.0)),
        ]))
    };
    let body = Object(BTreeMap::from([
        ("m_Name".to_string(), UnityString("death".to_string())),
        (
            "m_PositionCurves".to_string(),
            Array(vec![
                plain_test_curve("Bip01".to_string(), Some(vec3(1.0))),
                plain_test_curve(
                    "Bip01".to_string(),
                    Some(vec3(if conflicting { 9.0 } else { 1.0 })),
                ),
            ]),
        ),
        ("m_RotationCurves".to_string(), Array(Vec::new())),
        ("m_CompressedRotationCurves".to_string(), Array(Vec::new())),
        ("m_EulerCurves".to_string(), Array(Vec::new())),
        ("m_ScaleCurves".to_string(), Array(Vec::new())),
        ("m_FloatCurves".to_string(), Array(Vec::new())),
        ("m_PPtrCurves".to_string(), Array(Vec::new())),
        ("m_Events".to_string(), Array(Vec::new())),
    ]));
    (
        decode_animation_clip("CustomAssetBundle-Retro_shared", 5248, &body, Vec::new())
            .preview,
        json!({ "nodes": [{ "path": "npc/Bip01" }] }),
    )
}
