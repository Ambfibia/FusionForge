use super::*;

#[test]
fn rigid_attachment_rest_transform_keeps_mesh_filter_pivot() {
    let root = (0, 1);
    let eye = (0, 2);
    let transforms = BTreeMap::from([
        (
            root,
            TransformNode {
                key: root,
                game_object: None,
                name: "Head".to_string(),
                parent: None,
                translation: [1.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
            },
        ),
        (
            eye,
            TransformNode {
                key: eye,
                game_object: None,
                name: "Eye".to_string(),
                parent: Some(root),
                translation: [0.0, 0.15, -1.1],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
            },
        ),
    ]);
    let paths = BTreeMap::from([(root, "Head".to_string()), (eye, "Head/Eye".to_string())]);

    let global = rest_global_matrix_in_skeleton_space(
        eye,
        &transforms,
        &paths,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
    .expect("eye rest transform");

    assert!((global[0][3] + 1.0).abs() < 1.0e-9);
    assert!((global[1][3] - 0.15).abs() < 1.0e-9);
    assert!((global[2][3] + 1.1).abs() < 1.0e-9);
}
