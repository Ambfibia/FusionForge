use super::*;

#[test]
fn exact_float_validation_preserves_zero_key_material_binding() {
    let tracks = vec![json!({
        "path": "splash_back",
        "property": "_MainTex.offset.x",
        "classId": 21,
        "script": { "fileId": 0, "pathId": 0 },
        "preInfinity": 2,
        "postInfinity": 2,
        "interpolation": "CUBICSPLINE",
        "keys": [],
        "duplicateKeys": [],
        "sourceKeyCount": 0,
        "sourceIndex": 0,
        "sourceEncoding": "plain"
    })];
    let hierarchy = vec![
        "npc_gubbie".to_string(),
        "npc_gubbie/splash_back".to_string(),
    ];

    assert_eq!(
        validate_sampleable_float_tracks(&tracks, 1, &hierarchy, "stand3", 3.875)
            .expect("zero-key material binding is exact metadata"),
        None
    );
}
