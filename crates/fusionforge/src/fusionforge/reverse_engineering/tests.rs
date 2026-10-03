use super::*;

#[test]
fn inventory_diff_is_case_aware_and_content_deterministic() {
    let root = tempfile::tempdir().expect("temp directory");
    let left = root.path().join("left.tsv");
    let right = root.path().join("right.tsv");
    fs::write(
        &left,
        "relative_path\tbytes\textension\ttags\nA.resourceFile\t10\t.resourcefile\tbundles,world\nsame.bin\t2\t.bin\tother\n",
    )
    .expect("write left inventory");
    fs::write(
        &right,
        "relative_path\tbytes\textension\ttags\na.resourceFile\t11\t.resourcefile\tbundles,world\nnew.bin\t3\t.bin\tother\n",
    )
    .expect("write right inventory");

    let report = compare_inventories(
        parse_inventory(&left, None).expect("parse left"),
        parse_inventory(&right, None).expect("parse right"),
        None,
    );
    assert_eq!(report.counts.added, 1);
    assert_eq!(report.counts.removed, 1);
    assert_eq!(report.counts.changed, 1);
    assert!(report.changed[0]
        .differences
        .iter()
        .any(|difference| difference == "pathCase"));
}

#[test]
fn focused_json_diff_keys_objects_by_serialized_asset_not_bare_path_id() {
    let left = serde_json::json!({
        "objects": [
            {"serializedAsset": "AssetA", "type": "Mesh", "pathId": 1, "vertices": 10},
            {"serializedAsset": "AssetB", "type": "Mesh", "pathId": 1, "vertices": 20}
        ]
    });
    let right = serde_json::json!({
        "objects": [
            {"serializedAsset": "AssetB", "type": "Mesh", "pathId": 1, "vertices": 21},
            {"serializedAsset": "AssetA", "type": "Mesh", "pathId": 1, "vertices": 10}
        ]
    });

    let diff = structural_diff(&left, &right, 100);
    assert_eq!(diff.total, 1);
    assert!(diff.changes[0].path.contains("AssetB"));
    assert!(identity_for(&serde_json::json!({"type": "Mesh", "pathId": 1})).is_none());
}

#[test]
fn focused_json_diff_uses_managed_script_identity() {
    let left = serde_json::json!([
        {"assembly": "Assembly-CSharp.dll", "namespace": "", "class": "cnOptionMode", "fields": 63}
    ]);
    let right = serde_json::json!([
        {"assembly": "Assembly-CSharp.dll", "namespace": "", "class": "cnOptionMode", "fields": 64}
    ]);

    let diff = structural_diff(&left, &right, 100);
    assert_eq!(diff.total, 1);
    assert!(diff.changes[0].path.contains("cnOptionMode"));
}
