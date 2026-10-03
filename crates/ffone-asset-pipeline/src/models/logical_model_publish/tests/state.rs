use super::*;

#[test]
    fn preserves_empty_named_stale_null_property_as_non_runtime_provenance() {
        let mut value = fixture();
        value["materials"]["fixture:mat-body"]["savedProperties"]["textureEnvs"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "slot": 1,
                "name": "",
                "unassignedSlot": true,
                "textureId": null,
                "texturePointer": pointer(0),
                "scale": {"x":1.0,"y":0.0},
                "offset": {"x":0.0,"y":0.0},
                "pivot": null,
                "rotation": null
            }));

        let (temp, report) = publish_fixture(&value);
        let glb = fs::read(temp.path().join("output").join(&report.contract.output_glb)).unwrap();
        let document = glb_json(&glb);
        let bindings = document["materials"][0]["extras"]["ffone"]["textureBindings"]
            .as_array()
            .unwrap();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[1]["slot"], "");
        assert_eq!(bindings[1]["unassignedStaleNull"], true);
        assert!(bindings[1]["texture"].is_null());
        let audit = crate::audit_logical_model_tree(&temp.path().join("output")).unwrap();
        assert!(audit.passed, "{:#?}", audit.violations);
    }

#[test]
    fn direct_conversion_writes_only_runtime_files_and_replays() {
        let root = tempfile::tempdir().unwrap();
        let source = serde_json::to_vec(&fixture()).unwrap();
        let options = LogicalModelPublishOptions::new("in-memory", "npc", root.path()).with_semantic_root_layout();
        let report = convert_logical_model_bytes(&options, &source).unwrap();
        assert!(root.path().join(&report.contract.output_glb).is_file());
        assert!(!root.path().join(&report.report_path).exists());
        convert_logical_model_bytes(&options, &source).unwrap();
        assert!(!root.path().join(&report.report_path).exists());
    }
