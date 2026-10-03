use super::*;

#[test]
    fn preserves_exact_transform_name_when_legacy_lookup_path_trims_its_component() {
        let mut value = fixture();
        value["modelHierarchy"]["nodes"][1]["name"] = json!("Bip01 ");
        let source: SourceDocument = serde_json::from_value(value).unwrap();
        let converted = convert_source(&source).unwrap();
        assert_eq!(converted.model.nodes[1].name, "Bip01 ");
        assert_eq!(converted.model.nodes[2].parent, Some(1));
    }

#[test]
    fn empty_trs_path_kind_index_encoding_order_and_raw_counts_are_strict() {
        let source = fixture_with_empty_trs_binding();
        let mutations = [
            ("path", json!("missing/Bone"), "no suffix match"),
            ("kind", json!("scale"), "outside raw"),
            ("sourceIndex", json!(2), "outside raw"),
            ("sourceEncoding", json!("compressed"), "uses invalid"),
        ];
        for (field, replacement, expected) in mutations {
            let mut mutation = source.clone();
            mutation["animations"][0]["animationData"]["emptyTrsBindings"][0][field] = replacement;
            let error = publish_error(&mutation);
            assert!(
                error.to_string().contains(expected),
                "{field} mutation produced {error}"
            );
        }

        let mut count = source.clone();
        count["animations"][0]["curveCounts"]["position"] = json!(1);
        let error = publish_error(&count);
        assert!(error.to_string().contains("outside raw"));

        let mut collision = source.clone();
        collision["animations"][0]["animationData"]["emptyTrsBindings"][0]["path"] =
            json!("Bip01/Bone");
        let error = publish_error(&collision);
        assert!(
            error
                .to_string()
                .contains("collides with a non-empty channel")
        );

        let mut order = source;
        order["animations"][0]["curveCounts"]["compressedRotation"] = json!(2);
        order["animations"][0]["animationData"]["emptyTrsBindings"] = json!([{
            "kind": "rotation",
            "path": "Bip01",
            "sourceIndex": 1,
            "sourceEncoding": "compressed"
        }, {
            "kind": "translation",
            "path": "Bip01",
            "sourceIndex": 1,
            "sourceEncoding": "plain"
        }]);
        let error = publish_error(&order);
        assert!(error.to_string().contains("canonical source order"));
    }
