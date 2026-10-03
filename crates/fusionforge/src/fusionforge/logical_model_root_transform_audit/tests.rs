use super::*;
use serde_json::json;

fn trs(translation: [f64; 3], rotation: [f64; 4], scale: [f64; 3]) -> RootTrs {
    RootTrs {
        translation,
        rotation,
        scale,
    }
}

fn ready_root(root_type: &str) -> JsonValue {
    json!({
        "status": "ready",
        "kfm": {"exactRoute": "mob/test.kfm", "owner": {
            "bundlePath": "D:/build/Test.resourceFile",
            "assetName": "CustomAssetBundle-owner"
        }},
        "payload": {
            "preloadOwnership": {"dependencyArchives": [{
                "archiveName": "customassetbundle-dependency",
                "candidateBundlePaths": ["D:/build/Dependency.resourceFile"]
            }]},
            "selfContainedGameObject": {
                "rootName": "npc_test",
                "rootTransform": {
                    "assetName": "CustomAssetBundle-dependency",
                    "pathId": 42,
                    "objectType": root_type
                }
            }
        }
    })
}

#[test]
fn unity_bevy_basis_mapping_preserves_authored_scale_and_origin_magnitude() {
    let unity = trs([2.0, -3.0, 4.0], [0.1, 0.2, -0.3, 0.9], [1.35, 1.35, 1.35]);
    let native = unity_to_native_trs(unity);
    assert_eq!(native.translation, [-2.0, -3.0, 4.0]);
    assert_eq!(native.rotation, [0.1, -0.2, 0.3, 0.9]);
    assert_eq!(native.scale, unity.scale);
}

#[test]
fn classification_allows_uniform_nonunit_scale_and_nonzero_origin() {
    let class = classify_root_trs(trs(
        [0.740397, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        [1.3, 1.3, 1.3],
    ));
    assert!(class.finite && class.uniform_scale && class.positive_scale);
    assert!(!class.unit_scale);
    assert!(class.nonzero_authored_origin && class.identity_authored_rotation);
}

#[test]
fn classification_rejects_nonuniform_nonpositive_and_nonfinite_values() {
    let class = classify_root_trs(trs(
        [f64::INFINITY, 0.0, 0.0],
        [0.0, 0.0, 0.0, -1.0],
        [1.0, 0.0, 2.0],
    ));
    assert!(!class.finite && !class.uniform_scale && !class.positive_scale);
    assert!(!class.unit_scale);
    assert!(class.identity_authored_rotation);
}

#[test]
fn dependency_mapping_selects_exact_transform_asset_candidate() {
    let root = parse_ready_root(&ready_root("Transform")).expect("valid root");
    assert_eq!(
        root_candidate_bundle_paths(&root).expect("candidate"),
        vec!["D:/build/Dependency.resourceFile"]
    );
}

#[test]
fn tampered_root_object_type_is_fail_closed() {
    let error =
        parse_ready_root(&ready_root("GameObject")).expect_err("tampered root type must fail");
    assert_eq!(error.code, "malformedReadyRoot");
    assert!(error.detail.contains("expected \"Transform\""));
}

#[test]
fn missing_dependency_mapping_is_fail_closed() {
    let mut root = parse_ready_root(&ready_root("Transform")).expect("valid root");
    root.dependency_archives.clear();
    let error = root_candidate_bundle_paths(&root).expect_err("missing proof must fail");
    assert!(error.contains("no dependency candidate bundle"));
}

#[test]
fn report_pass_requires_complete_zero_error_audit() {
    let mut report = LegacyRootTransformAudit {
        schema: LEGACY_ROOT_TRANSFORM_AUDIT_SCHEMA,
        source_plan: "plan.json".to_string(),
        coordinate_contract: RootCoordinateContract::default(),
        counts: RootTransformAuditCounts {
            planned_ready_roots: 0,
            audited_roots: 0,
            ..RootTransformAuditCounts::default()
        },
        errors: Vec::new(),
        roots: Vec::new(),
    };
    assert!(report.passed());
    report.counts.planned_ready_roots = 1;
    assert!(!report.passed());
}
