use super::*;

#[test]
    #[ignore = "requires FFONE_LOGICAL_SOURCE_ROOT pointing at an external exact-source batch"]
    fn preflights_every_external_source_conversion_before_transactional_publish() {
        let root = PathBuf::from(
            std::env::var_os("FFONE_LOGICAL_SOURCE_ROOT")
                .expect("FFONE_LOGICAL_SOURCE_ROOT must be set"),
        );
        let mut files = Vec::new();
        external_source_files(&root, &mut files);
        assert!(!files.is_empty(), "external source root is empty");

        let mut passed = 0_usize;
        let mut failures = Vec::new();
        for file in &files {
            let bytes = fs::read(file).unwrap();
            let source: SourceDocument = match serde_json::from_slice(&bytes) {
                Ok(source) => source,
                Err(error) => {
                    failures.push(format!("{} | JSON: {error}", file.display()));
                    continue;
                }
            };
            match convert_source(&source) {
                Ok(_) => passed += 1,
                Err(error) => failures.push(format!(
                    "{} | logicalName={:?} | {error}",
                    file.display(),
                    source.logical_name
                )),
            }
        }
        eprintln!(
            "full conversion preflight: sources={}, passed={}, blocked={}",
            files.len(),
            passed,
            failures.len()
        );
        assert!(
            failures.is_empty(),
            "external conversion preflight failed:\n{}",
            failures.join("\n")
        );
    }

pub(super) fn publish_fixture(value: &Value) -> (TempDir, LogicalModelPublishReport) {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.json");
        fs::write(&source, serde_json::to_vec_pretty(value).unwrap()).unwrap();
        let output = temp.path().join("output");
        let report =
            publish_logical_model(&LogicalModelPublishOptions::new(&source, "npc", &output))
                .unwrap();
        (temp, report)
    }

#[test]
    fn preserves_but_does_not_publish_unbound_zero_key_metadata() {
        let mut value = fixture_with_empty_trs_binding();
        value["animations"][0]["animationData"]["emptyTrsBindings"][0]["path"] =
            json!("SiblingRig/Bip01");
        value["animations"][0]["animationData"]["emptyTrsBindings"][0]["unboundModelTarget"] =
            json!(true);

        let source: SourceDocument = serde_json::from_value(value).unwrap();
        let converted = convert_source(&source).unwrap();
        assert!(
            converted.model.animations[0]
                .metadata
                .empty_trs_bindings
                .is_empty()
        );
        assert_eq!(converted.source_counts.empty_trs_bindings, 0);
    }
