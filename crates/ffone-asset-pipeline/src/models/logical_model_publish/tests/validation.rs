use super::*;

pub(super) fn publish_error(value: &Value) -> PipelineError {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.json");
        fs::write(&source, serde_json::to_vec_pretty(value).unwrap()).unwrap();
        publish_logical_model(&LogicalModelPublishOptions::new(
            &source,
            "npc",
            temp.path().join("output"),
        ))
        .unwrap_err()
    }
