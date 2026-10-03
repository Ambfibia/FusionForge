use super::*;

#[test]
    fn repairs_an_opposed_triangle_before_native_validation() {
        let mut value = fixture();
        value["meshes"][0]["indices"] = json!([0, 2, 1]);
        let source: SourceDocument = serde_json::from_value(value).unwrap();
        let converted = convert_source(&source).unwrap();
        assert_eq!(converted.model.meshes[0].primitives[0].indices, [0, 1, 2]);
    }
