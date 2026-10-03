use super::*;

#[test]
    fn preserves_typed_animation_time_recovery_and_audits_on_wire_sampler_times() {
        let source = fixture_with_time_recovery();
        let (temp, report) = publish_fixture(&source);
        assert_eq!(report.contract.source.animation_time_recoveries, 1);
        assert_eq!(report.contract.published.animation_time_recoveries, 1);
        assert_eq!(report.contract.source.recovered_animation_keyframes, 2);
        assert_eq!(report.contract.published.recovered_animation_keyframes, 2);
        assert_eq!(
            report.semantic_proof.source.animation_metadata_sha256,
            report.semantic_proof.emitted.animation_metadata_sha256
        );

        let output = temp.path().join("output");
        let glb = fs::read(output.join(&report.contract.output_glb)).unwrap();
        let document = glb_json(&glb);
        let recovery = &document["animations"][0]["extras"]["nonTrs"]["timeRecoveries"][0];
        assert_eq!(recovery["targetNode"], json!(2));
        assert_eq!(recovery["targetPath"], json!("Bip01/Bone"));
        assert_eq!(recovery["originalTimes"], json!([0.0, 0.0]));
        assert_eq!(recovery["recoveredTimes"], json!([0.0, 0.1]));
        assert_eq!(
            recovery["proof"]["exactKeyPayloadExcludingTime"],
            json!(true)
        );
        let audit = crate::audit_logical_model_tree(&output).unwrap();
        assert!(audit.passed, "{:#?}", audit.violations);
        assert_eq!(audit.counts.features.animation_time_recoveries, 1);
        assert_eq!(audit.counts.features.recovered_animation_keyframes, 2);
    }
