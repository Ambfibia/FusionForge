use super::*;
use tempfile::TempDir;

fn fixture() -> Value {
    crate::logical_model_publish::logical_model_source_test_fixture()
}

fn write_source(
    root: &Path,
    category: &str,
    name: &str,
    source: &Value,
) -> EquipmentSourceExport {
    let relative = format!("characters/player/equipment/{category}/{name}/{name}.source.json");
    let path = relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut bytes = serde_json::to_vec_pretty(source).unwrap();
    bytes.push(b'\n');
    fs::write(&path, &bytes).unwrap();
    EquipmentSourceExport {
        category: category.to_owned(),
        true_name: name.to_owned(),
        safe_true_name: name.to_owned(),
        route_qualifier: None,
        source_relative_path: relative,
        source_byte_length: bytes.len() as u64,
        source_sha256: sha256_hex(&bytes),
        canonical_route: format!("wear/{}.nif", name.to_ascii_lowercase().replace(' ', "_")),
        normalized_route: format!("wear/{}.nif", name.to_ascii_lowercase().replace(' ', "_")),
        proven_alias_routes: Vec::new(),
        table_rows: vec![json!({"entityId": "equipment:test"})],
        owner: json!({"bundlePath": "fixture.resourceFile"}),
        physical_target: json!({
            "assetIndex": 0,
            "assetName": "fixture",
            "pathId": 1,
            "objectType": "GameObject",
        }),
        facts: json!({"fixture": true}),
    }
}

fn write_manifest(source_root: &Path, exported: &[EquipmentSourceExport], blockers: &[Value]) {
    let path = sibling_manifest_path(source_root).unwrap();
    let manifest = json!({
        "schema": EQUIPMENT_SOURCE_BATCH_SCHEMA,
        "status": if blockers.is_empty() { "complete" } else { "complete-with-typed-blockers" },
        "sourceRoot": slash_path(source_root),
        "benchmarkRouteLimit": Value::Null,
        "counts": {
            "equipmentEntities": exported.len(),
            "entitiesWithModelRoutes": exported.len(),
            "uniqueRoutes": exported.len(),
            "resolvedRoutes": exported.len(),
            "selectedResolvedRoutes": exported.len(),
            "exportedPhysicalModels": exported.len(),
            "totalBlockers": blockers.len(),
        },
        "bundles": [],
        "exported": exported,
        "blockers": blockers,
    });
    fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

#[test]
fn publishes_exact_equipment_taxonomy_without_models_prefix() {
    let temp = TempDir::new().unwrap();
    let source_root = temp.path().join("equipment-model-sources-full-v1");
    fs::create_dir(&source_root).unwrap();
    let export = write_source(&source_root, "back", "True Hero", &fixture());
    write_manifest(&source_root, &[export], &[]);
    let output = temp.path().join("equipment-logical-models-full-v1");

    let report = publish_equipment_logical_model_batch(
        &EquipmentLogicalModelBatchPublishOptions::new(&source_root, &output),
    )
    .unwrap();

    let expected = output.join("characters/player/equipment/back/True Hero/True Hero.glb");
    assert!(expected.is_file());
    assert!(!output.join("models").exists());
    assert_eq!(report.counts.published_models, 1);
    assert_eq!(
        report.models[0].output_glb,
        "characters/player/equipment/back/True Hero/True Hero.glb"
    );
    assert!(audit_logical_model_tree(&output).unwrap().passed);
}

#[test]
fn native_preparation_failure_is_typed_and_does_not_abort_good_model() {
    let temp = TempDir::new().unwrap();
    let source_root = temp.path().join("equipment-model-sources-full-v1");
    fs::create_dir(&source_root).unwrap();
    let good = write_source(&source_root, "back", "True Hero", &fixture());
    let mut bad_source = fixture();
    bad_source["logicalName"] = json!("Broken Hero");
    bad_source["modelHierarchy"]["roots"][0]["name"] = json!("Broken Hero");
    bad_source["modelHierarchy"]["roots"][0]["path"] = json!("Broken Hero");
    bad_source["modelHierarchy"]["nodes"][0]["name"] = json!("Broken Hero");
    bad_source["modelHierarchy"]["nodes"][0]["path"] = json!("Broken Hero");
    bad_source["warnings"] = json!(["fixture warning must remain blocking"]);
    let bad = write_source(&source_root, "hat", "Broken Hero", &bad_source);
    write_manifest(&source_root, &[good, bad], &[]);
    let output = temp.path().join("equipment-logical-models-full-v1");

    let report = publish_equipment_logical_model_batch(
        &EquipmentLogicalModelBatchPublishOptions::new(&source_root, &output),
    )
    .unwrap();

    assert_eq!(report.counts.published_models, 1);
    assert_eq!(report.counts.native_preparation_blocked_sources, 1);
    assert!(report.blockers.iter().any(|blocker| {
        blocker.code == "nativeLogicalModelPreparationFailed"
            && blocker.true_name.as_deref() == Some("Broken Hero")
    }));
    assert!(
        !output
            .join("characters/player/equipment/hat/Broken Hero/Broken Hero.glb")
            .exists()
    );
    assert!(audit_logical_model_tree(&output).unwrap().passed);
}

#[test]
fn portable_case_folded_collision_blocks_every_owner_without_suffixes() {
    let plans = vec![
        test_plan(
            "vehicle",
            "HoverCar",
            "characters/player/equipment/vehicle/HoverCar/HoverCar.glb",
        ),
        test_plan(
            "vehicle",
            "hovercar",
            "characters/player/equipment/vehicle/hovercar/hovercar.glb",
        ),
    ];
    let blockers = global_collision_blockers(&plans).unwrap();
    assert_eq!(blockers.len(), 2);
    assert!(blockers.values().all(|blocker| {
        blocker.code == "portableSemanticOutputCollision"
            && blocker.disposition == "blocked-no-overwrite-or-generated-suffix"
    }));
}

fn test_plan(category: &str, name: &str, output: &str) -> EquipmentPlan {
    EquipmentPlan {
        export: EquipmentSourceExport {
            category: category.to_owned(),
            true_name: name.to_owned(),
            safe_true_name: name.to_owned(),
            route_qualifier: None,
            source_relative_path: format!(
                "characters/player/equipment/{category}/{name}/{name}.source.json"
            ),
            source_byte_length: 1,
            source_sha256: "00".repeat(32),
            canonical_route: format!("wear/{name}.nif"),
            normalized_route: format!("wear/{}.nif", name.to_ascii_lowercase()),
            proven_alias_routes: Vec::new(),
            table_rows: vec![json!({"entityId": "fixture"})],
            owner: json!({}),
            physical_target: json!({}),
            facts: json!({}),
        },
        source: PathBuf::from("fixture"),
        output_glb: PathBuf::from(output),
        output_files: vec![PathBuf::from(output)],
    }
}
