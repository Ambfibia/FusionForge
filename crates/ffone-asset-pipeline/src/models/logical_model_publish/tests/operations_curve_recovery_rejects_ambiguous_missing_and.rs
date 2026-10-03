use super::*;

#[test]
    fn curve_recovery_rejects_ambiguous_missing_and_tampered_authority() {
        let source = fixture_with_curve_recovery();

        let mut ambiguous = source.clone();
        let mut second_sibling = ambiguous["animations"][1].clone();
        second_sibling["pathId"] = json!(102);
        second_sibling["name"] = json!("stand_reference_2");
        ambiguous["animations"]
            .as_array_mut()
            .unwrap()
            .push(second_sibling);
        assert!(
            publish_error(&ambiguous)
                .to_string()
                .contains("one unique exact sibling authority")
        );

        let mut missing = source.clone();
        missing["animations"][0]["animationData"]["curveRecoveries"] = json!([]);
        assert!(
            publish_error(&missing)
                .to_string()
                .contains("discarded a raw Translation/Plain curve identity")
        );

        let mutations = [
            (
                "/animations/0/animationData/curveRecoveries/0/canonical/keys/0/value/1",
                json!(-99.0),
                "canonical provenance differs",
            ),
            (
                "/animations/0/animationData/curveRecoveries/0/reference/pathId",
                json!(999),
                "one unique exact sibling authority",
            ),
            (
                "/animations/0/animationData/curveRecoveries/0/proof/matchingReferenceCount",
                json!(2),
                "proof is incomplete or ambiguous",
            ),
        ];
        for (pointer, replacement, expected) in mutations {
            let mut tampered = source.clone();
            *tampered.pointer_mut(pointer).unwrap() = replacement;
            let error = publish_error(&tampered).to_string();
            assert!(error.contains(expected), "{pointer}: {error}");
        }

        let mut unknown = source;
        unknown["animations"][0]["animationData"]["curveRecoveries"][0]["invented"] = json!(true);
        assert!(
            publish_error(&unknown)
                .to_string()
                .contains("unknown field")
        );
    }

#[test]
    fn publishes_zero_key_float_binding_as_exact_metadata() {
        let mut value = fixture();
        value["animations"][0]["curveCounts"]["float"] = json!(1);
        value["animations"][0]["animationData"]["floatCurves"] = json!([{
            "path": "Bip01/Bone",
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
        }]);

        let (temp, report) = publish_fixture(&value);
        assert_eq!(report.contract.source.float_curves, 1);
        assert_eq!(report.contract.published.float_curves, 1);
        let glb_path = temp.path().join("output").join(report.contract.output_glb);
        let document = glb_json(&fs::read(glb_path).unwrap());
        let curve = &document["animations"][0]["extras"]["nonTrs"]["floatCurves"][0];
        assert_eq!(curve["sourceKeyCount"], json!(0));
        assert_eq!(curve["times"], json!([]));
        assert_eq!(curve["values"], json!([]));
    }

#[test]
    fn publication_never_overwrites_any_existing_sidecar() {
        let value = fixture();
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.json");
        let output = temp.path().join("output");
        fs::write(&source, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        publish_logical_model(&LogicalModelPublishOptions::new(&source, "npc", &output)).unwrap();
        let before = fs::read(output.join("models/npc/True Hero.glb")).unwrap();
        let error =
            publish_logical_model(&LogicalModelPublishOptions::new(&source, "npc", &output))
                .unwrap_err();
        assert!(error.to_string().contains("never overwrites"));
        assert_eq!(
            fs::read(output.join("models/npc/True Hero.glb")).unwrap(),
            before
        );
    }

#[test]
    fn rejects_logical_name_that_is_not_the_true_root_name() {
        let mut value = fixture();
        value["logicalName"] = json!("hash--01234567");
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.json");
        fs::write(&source, serde_json::to_vec(&value).unwrap()).unwrap();
        let error = publish_logical_model(&LogicalModelPublishOptions::new(
            source,
            "npc",
            temp.path().join("out"),
        ))
        .unwrap_err();
        assert!(error.to_string().contains("logicalName, root summary"));
    }

#[test]
    fn rejects_unsupported_curve_count_instead_of_dropping_it() {
        let mut value = fixture();
        value["animations"][0]["curveCounts"]["euler"] = json!(1);
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.json");
        fs::write(&source, serde_json::to_vec(&value).unwrap()).unwrap();
        let error = publish_logical_model(&LogicalModelPublishOptions::new(
            source,
            "npc",
            temp.path().join("out"),
        ))
        .unwrap_err();
        assert!(error.to_string().contains("unsupported Euler/PPtr"));
    }
