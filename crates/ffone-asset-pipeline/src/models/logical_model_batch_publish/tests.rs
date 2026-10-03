use super::*;
use serde_json::{Value, json};
use tempfile::TempDir;

fn fixture() -> Value {
    crate::logical_model_publish::logical_model_source_test_fixture()
}

fn write_source(root: &Path, relative: &str, value: &Value) {
    let path = relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn stage_entries(parent: &Path, output_name: &str) -> Vec<String> {
    fs::read_dir(parent)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .filter(|name| name.contains(&format!(".{output_name}.ffone-logical-model-batch.")))
        .collect()
}

#[test]
fn publishes_ordered_family_and_semantic_mappings_with_one_final_tree() {
    let temp = TempDir::new().unwrap();
    let sources = temp.path().join("sources");
    write_source(&sources, "npc/route_b/True Hero.source.json", &fixture());
    write_source(&sources, "mob/route_a/True Hero.source.json", &fixture());
    let output = temp.path().join("candidate");
    let report =
        publish_logical_model_batch(&LogicalModelBatchPublishOptions::new(&sources, &output))
            .unwrap();

    assert_eq!(report.counts.sources, 2);
    assert_eq!(report.counts.families, 2);
    assert_eq!(report.counts.glbs, 2);
    assert_eq!(report.counts.pngs, 4);
    assert_eq!(
        report.coordinate_status,
        "artifact-space-proven-runtime-spawn-policy-pending"
    );
    assert_eq!(report.coordinate_artifact_space_proven, 2);
    assert_eq!(report.coordinate_runtime_spawn_policy_pending, 2);
    assert_eq!(report.coordinate_mismatches, 0);
    assert_eq!(report.skinning_basis_parity_max_error, Some(0.0));
    assert!(report.current_pose_bind_identity_deviation_max.is_some());
    assert!(report.models.iter().all(|model| {
        model.coordinate_status == "artifact-space-proven-runtime-spawn-policy-pending"
            && model.runtime_spawn_policy == "runtime-archetype-dependent-pending"
            && model.skinning_basis_parity_max_error == Some(0.0)
            && model.current_pose_bind_identity_deviation_max.is_some()
    }));
    assert_eq!(
        report
            .models
            .iter()
            .map(|model| model.source.as_str())
            .collect::<Vec<_>>(),
        [
            "mob/route_a/True Hero.source.json",
            "npc/route_b/True Hero.source.json"
        ]
    );
    assert!(output.join("models/mob/route_a/True Hero.glb").is_file());
    assert!(output.join("models/npc/route_b/True Hero.glb").is_file());
    assert!(output.join(LOGICAL_MODEL_BATCH_REPORT_FILE).is_file());
    assert!(audit_logical_model_tree(&output).unwrap().passed);
    assert!(stage_entries(temp.path(), "candidate").is_empty());
}

#[test]
fn blocks_only_the_model_with_an_invalid_exact_texture_slot_and_publishes_its_sibling() {
    let temp = TempDir::new().unwrap();
    let sources = temp.path().join("sources");
    write_source(&sources, "npc/proven/True Hero.source.json", &fixture());
    let mut blocked = fixture();
    blocked["materials"]["fixture:mat-body"]["savedProperties"]["textureEnvs"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "slot": 1,
            "name": "",
            "textureId": null,
            "texturePointer": {
                "sourceAssetIndex": 0,
                "fileId": 0,
                "pathId": 0,
                "isNull": true
            },
            "scale": {"x": 1.0, "y": 0.0},
            "offset": {"x": 0.0, "y": 0.0},
            "pivot": null,
            "rotation": null
        }));
    write_source(&sources, "npc/blocked/True Hero.source.json", &blocked);

    let output = temp.path().join("candidate");
    let report =
        publish_logical_model_batch(&LogicalModelBatchPublishOptions::new(&sources, &output))
            .unwrap();
    assert_eq!(report.counts.planned_sources, 2);
    assert_eq!(report.counts.sources, 1);
    assert_eq!(report.counts.blocked_sources, 1);
    assert_eq!(report.models.len(), 1);
    assert_eq!(report.blockers.len(), 1);
    assert!(report.status.starts_with("complete-with-blocked"));
    let blocker = &report.blockers[0];
    assert_eq!(blocker.code, "invalidSerializedTextureSlotName");
    assert_eq!(blocker.source, "npc/blocked/True Hero.source.json");
    assert_eq!(blocker.disposition, "blocked-no-fallback-glb");
    assert_eq!(
        blocker.evidence["invalidProperties"][0]["materialSourcePathId"],
        json!(300)
    );
    assert_eq!(
        blocker.evidence["invalidProperties"][0]["invalidSavedTextureArrayIndex"],
        json!(1)
    );
    assert_eq!(
        blocker.evidence["invalidProperties"][0]["invalidSerializedName"],
        json!("")
    );
    assert!(output.join("models/npc/proven/True Hero.glb").is_file());
    assert!(!output.join("models/npc/blocked/True Hero.glb").exists());
    assert!(audit_logical_model_tree(&output).unwrap().passed);
}

#[test]
fn publishes_an_explicit_unassigned_stale_null_property_without_naming_it() {
    let temp = TempDir::new().unwrap();
    let sources = temp.path().join("sources");
    let mut value = fixture();
    value["materials"]["fixture:mat-body"]["savedProperties"]["textureEnvs"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "slot": 1,
            "name": "",
            "unassignedSlot": true,
            "textureId": null,
            "texturePointer": {
                "sourceAssetIndex": 0,
                "fileId": 0,
                "pathId": 0,
                "isNull": true
            },
            "scale": {"x": 1.0, "y": 0.0},
            "offset": {"x": 0.0, "y": 0.0},
            "pivot": null,
            "rotation": null
        }));
    write_source(&sources, "npc/proven/True Hero.source.json", &value);

    let output = temp.path().join("candidate");
    let report =
        publish_logical_model_batch(&LogicalModelBatchPublishOptions::new(&sources, &output))
            .unwrap();
    assert_eq!(report.counts.sources, 1);
    assert_eq!(report.counts.blocked_sources, 0);
    assert!(report.blockers.is_empty());
    assert!(audit_logical_model_tree(&output).unwrap().passed);
}

#[test]
fn rejects_case_folded_semantic_directory_aliases_before_staging() {
    let mut registry = PortableOutputRegistry::default();
    registry
        .register(Path::new("models/npc/Route/Hero.glb"), "first")
        .unwrap();
    let error = registry
        .register(Path::new("models/npc/route/Other.glb"), "second")
        .unwrap_err();
    assert!(matches!(error, PipelineError::OutputCollision { .. }));
}

#[test]
fn rejects_unicode_normalization_aliases_before_staging() {
    let mut registry = PortableOutputRegistry::default();
    registry
        .register(Path::new("models/npc/Caf\u{e9}/Hero.glb"), "first")
        .unwrap();
    let error = registry
        .register(Path::new("models/npc/Cafe\u{301}/Other.glb"), "second")
        .unwrap_err();
    assert!(matches!(error, PipelineError::OutputCollision { .. }));
}

#[test]
fn rejects_texture_sidecar_collisions_in_global_preflight() {
    let temp = TempDir::new().unwrap();
    let sources = temp.path().join("sources");
    let mut value = fixture();
    value["textures"]["fixture:tex-body"]["name"] = json!("Color:A.dds");
    value["textures"]["fixture:tex-sword"]["name"] = json!("Color?A.dds");
    write_source(&sources, "npc/route/True Hero.source.json", &value);
    let output = temp.path().join("candidate");
    let error =
        publish_logical_model_batch(&LogicalModelBatchPublishOptions::new(&sources, &output))
            .unwrap_err();
    assert!(error.to_string().contains("same output path"));
    assert!(!output.exists());
    assert!(stage_entries(temp.path(), "candidate").is_empty());
}

#[test]
fn late_model_validation_error_removes_the_entire_staging_tree() {
    let temp = TempDir::new().unwrap();
    let sources = temp.path().join("sources");
    write_source(&sources, "npc/a/True Hero.source.json", &fixture());
    let mut invalid = fixture();
    invalid["animations"][0]["curveCounts"]["euler"] = json!(1);
    write_source(&sources, "npc/b/True Hero.source.json", &invalid);
    let output = temp.path().join("candidate");
    let error =
        publish_logical_model_batch(&LogicalModelBatchPublishOptions::new(&sources, &output))
            .unwrap_err();
    assert!(error.to_string().contains("unsupported Euler/PPtr"));
    assert!(!output.exists());
    assert!(stage_entries(temp.path(), "candidate").is_empty());
}

#[test]
fn rejects_generated_identity_and_non_true_source_filename() {
    let temp = TempDir::new().unwrap();
    let sources = temp.path().join("sources");
    write_source(&sources, "npc/PathID_123/renamed.source.json", &fixture());
    let output = temp.path().join("candidate");
    let error =
        publish_logical_model_batch(&LogicalModelBatchPublishOptions::new(&sources, &output))
            .unwrap_err();
    assert!(error.to_string().contains("generated/hash/PathID"));
    assert!(!output.exists());
}

#[test]
fn refuses_existing_destination_without_mutation() {
    let temp = TempDir::new().unwrap();
    let sources = temp.path().join("sources");
    write_source(&sources, "npc/route/True Hero.source.json", &fixture());
    let output = temp.path().join("candidate");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("keep.txt"), b"keep").unwrap();
    let error =
        publish_logical_model_batch(&LogicalModelBatchPublishOptions::new(&sources, &output))
            .unwrap_err();
    assert!(matches!(error, PipelineError::OutputExists(_)));
    assert_eq!(fs::read(output.join("keep.txt")).unwrap(), b"keep");
}

#[test]
fn rejects_unexpected_files_and_missing_semantic_owner_directory() {
    let temp = TempDir::new().unwrap();
    let sources = temp.path().join("sources");
    fs::create_dir_all(sources.join("npc")).unwrap();
    fs::write(sources.join("npc/notes.txt"), b"not a source").unwrap();
    let output = temp.path().join("candidate");
    let error =
        publish_logical_model_batch(&LogicalModelBatchPublishOptions::new(&sources, &output))
            .unwrap_err();
    assert!(error.to_string().contains("only .source.json is allowed"));

    fs::remove_file(sources.join("npc/notes.txt")).unwrap();
    write_source(&sources, "npc/True Hero.source.json", &fixture());
    let error =
        publish_logical_model_batch(&LogicalModelBatchPublishOptions::new(&sources, &output))
            .unwrap_err();
    assert!(error.to_string().contains("exact-route-stem"));
}
