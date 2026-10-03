use super::*;
use crate::fusionforge::logical_model_export_plan::{
    LogicalModelRouteProvenance, LogicalModelStandaloneProof,
};

fn temp(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("ff-prop-{label}-{}", unique()));
    fs::create_dir(&path).unwrap();
    path
}

fn reviewed() -> ReviewedLogicalProp {
    ReviewedLogicalProp {
        exact_route: "map/etc/wd_tree001.nif".to_string(),
        normalized_route: "map/etc/wd_tree001.nif".to_string(),
        category: LogicalPropCategory::Tree,
        review: LogicalPropReviewEvidence {
            method: LogicalPropReviewMethod::ManualVisualReview,
            reference: "reviews/tree.png#approved".to_string(),
        },
        expected_standalone_proof: ReviewedStandaloneNifProof {
            sole_catalog_owner: true,
            exact_route_has_no_collision: true,
            not_referenced_by_any_parsed_kfm: true,
            all_kfm_groups_parsed: true,
        },
    }
}

fn standalone() -> LogicalModelStandaloneNif {
    LogicalModelStandaloneNif {
        nif: LogicalModelRouteProvenance {
            exact_route: "map/etc/wd_tree001.nif".to_string(),
            normalized_route: "map/etc/wd_tree001.nif".to_string(),
            owner: LogicalModelOwner {
                bundle_path: "D:/build/DongResources.resourceFile".to_string(),
                bundle_name: "DongResources.resourceFile".to_string(),
                asset_name: "CustomAssetBundle-tree".to_string(),
            },
        },
        proof: LogicalModelStandaloneProof {
            sole_catalog_owner: true,
            exact_route_has_no_collision: true,
            not_referenced_by_any_parsed_kfm: true,
            all_kfm_groups_parsed: true,
        },
    }
}

fn gate<'a>(standalone: &'a [LogicalModelStandaloneNif]) -> PlanGate<'a> {
    PlanGate {
        scope_mode: "full",
        no_requested_kfm: true,
        ownership_complete: true,
        standalone_complete: true,
        standalone_nifs: standalone,
    }
}

fn source() -> Value {
    serde_json::json!({
        "schema": "ffone.logical-model-source.v1",
        "selectionMode": "exact-container-route",
        "status": "ready",
        "logicalName": "WD_Tree001",
        "exactContainerRoute": "map/etc/wd_tree001.nif",
        "containerPaths": ["map/etc/wd_tree001.nif"],
        "matchedPaths": ["map/etc/wd_tree001.nif"],
        "modelHierarchy": {
            "roots": [{"name": "WD_Tree001"}],
            "nodes": [{"name": "WD_Tree001"}]
        },
        "meshes": [{"name": "tree"}],
        "animations": [],
        "kfm": [],
        "warnings": []
    })
}

#[test]
fn current_name_only_candidates_cannot_mutate_output() {
    let root = temp("reject-candidates");
    let cache = root.join("project").join("cache");
    fs::create_dir_all(&cache).unwrap();
    let index = cache.join("bundle-index.json");
    fs::write(&index, b"{}").unwrap();
    let candidates = root.join("retrobution.logical-prop-candidates.json");
    let output = root.join("sources");
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schema": "ffone.logical-prop-candidates.v1",
        "classificationStatus": "inferred_pending_review",
        "verified": [],
        "candidates": []
    }))
    .unwrap();
    fs::write(&candidates, bytes).unwrap();
    let error = export_logical_prop_sources_batch(&index, &candidates, &output).unwrap_err();
    assert!(error.contains("invalid reviewed prop plan"));
    assert!(!output.exists());
    assert!(!manifest_path(&output).unwrap().exists());
    assert!(fs::read_dir(&root).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("staging")));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn incomplete_full_plan_cannot_create_staging() {
    let root = temp("reject-plan");
    let output = root.join("sources");
    let review = LogicalPropReview {
        schema: LOGICAL_PROP_REVIEW_SCHEMA.to_string(),
        status: "verified".to_string(),
        routes: vec![reviewed()],
    };
    let models = vec![standalone()];
    let mut plan = gate(&models);
    plan.ownership_complete = false;
    let error = export_with(
        Path::new("project/cache/bundle-index.json"),
        Path::new("review.json"),
        b"{}",
        &review,
        plan,
        &output,
        |_| panic!("loader ran before proof gate"),
    )
    .unwrap_err();
    assert!(error.contains("ownershipScanComplete=true"));
    assert!(!output.exists());
    assert!(!manifest_path(&output).unwrap().exists());
    assert!(fs::read_dir(&root).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("staging")));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn complete_reviewed_fixture_exports_static_nif_atomically() {
    let root = temp("complete");
    let output = root.join("sources");
    let review = LogicalPropReview {
        schema: LOGICAL_PROP_REVIEW_SCHEMA.to_string(),
        status: "verified".to_string(),
        routes: vec![reviewed()],
    };
    let bytes = serde_json::to_vec_pretty(&review).unwrap();
    let models = vec![standalone()];
    let result = export_with(
        Path::new("project/cache/bundle-index.json"),
        Path::new("review.json"),
        &bytes,
        &review,
        gate(&models),
        &output,
        |_| Ok(source()),
    )
    .unwrap();
    assert_eq!(result.exported_count, 1);
    let relative = "props/vegetation/trees/map/etc/wd_tree001/WD_Tree001.source.json";
    let exported: Value =
        serde_json::from_slice(&fs::read(output.join(relative)).unwrap()).unwrap();
    assert_eq!(exported["kfm"], serde_json::json!([]));
    assert_eq!(exported["animations"], serde_json::json!([]));
    assert!(manifest_path(&output).unwrap().is_file());
    fs::remove_dir_all(root).unwrap();
}
