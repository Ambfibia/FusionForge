use std::collections::BTreeMap;

use super::*;

fn vec3_value(value: Vec3) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        ("x".into(), UnityValue::Float(value.0)),
        ("y".into(), UnityValue::Float(value.1)),
        ("z".into(), UnityValue::Float(value.2)),
    ]))
}

fn quaternion_value(value: (f64, f64, f64, f64)) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        ("x".into(), UnityValue::Float(value.0)),
        ("y".into(), UnityValue::Float(value.1)),
        ("z".into(), UnityValue::Float(value.2)),
        ("w".into(), UnityValue::Float(value.3)),
    ]))
}

fn simple_clockwise_triangle_mesh() -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        (
            "m_Vertices".into(),
            UnityValue::Array(vec![
                vec3_value((0.0, 0.0, 0.0)),
                vec3_value((0.0, 1.0, 0.0)),
                vec3_value((1.0, 0.0, 0.0)),
            ]),
        ),
        (
            "m_Normals".into(),
            UnityValue::Array(vec![vec3_value((0.0, 0.0, -1.0)); 3]),
        ),
        (
            "m_IndexBuffer".into(),
            UnityValue::Bytes(vec![0, 0, 1, 0, 2, 0]),
        ),
        (
            "m_SubMeshes".into(),
            UnityValue::Array(vec![UnityValue::Object(BTreeMap::from([
                ("firstByte".into(), UnityValue::Int(0)),
                ("indexCount".into(), UnityValue::Int(3)),
                ("isTriStrip".into(), UnityValue::Int(0)),
            ]))]),
        ),
    ]))
}

fn axis_angle(axis: Vec3, radians: f64) -> (f64, f64, f64, f64) {
    let length = (axis.0 * axis.0 + axis.1 * axis.1 + axis.2 * axis.2).sqrt();
    let half = radians * 0.5;
    let factor = half.sin() / length;
    (
        axis.0 * factor,
        axis.1 * factor,
        axis.2 * factor,
        half.cos(),
    )
}

fn h_matrix() -> Matrix4 {
    [
        [-1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn assert_matrix_close(left: Matrix4, right: Matrix4) {
    for row in 0..4 {
        for col in 0..4 {
            assert!(
                (left[row][col] - right[row][col]).abs() <= 1.0e-10,
                "matrix mismatch [{row}][{col}]: {} != {}",
                left[row][col],
                right[row][col]
            );
        }
    }
}

fn transform_direction(matrix: Matrix4, direction: Vec3) -> Vec3 {
    (
        matrix[0][0] * direction.0 + matrix[0][1] * direction.1 + matrix[0][2] * direction.2,
        matrix[1][0] * direction.0 + matrix[1][1] * direction.1 + matrix[1][2] * direction.2,
        matrix[2][0] * direction.0 + matrix[2][1] * direction.1 + matrix[2][2] * direction.2,
    )
}

fn dot(left: Vec3, right: Vec3) -> f64 {
    left.0 * right.0 + left.1 * right.1 + left.2 * right.2
}

#[test]
fn h_l_h_matches_native_hierarchy_with_rotation_and_nonuniform_scale() {
    let parent_position = (17.0, -9.0, 512.0);
    let parent_rotation = axis_angle((0.0, 1.0, 0.0), 0.73);
    let parent_scale = (2.0, 0.75, 1.5);
    let child_position = (-4.0, 3.0, 8.0);
    let child_rotation = axis_angle((1.0, 2.0, -3.0), -0.41);
    let child_scale = (0.5, 3.0, 1.25);

    let unity_world = mat_mul(
        compose_matrix(parent_position, parent_rotation, parent_scale),
        compose_matrix(child_position, child_rotation, child_scale),
    );
    let native_world = mat_mul(
        compose_matrix(
            unity_to_native_vec3(parent_position),
            unity_to_native_quaternion(parent_rotation),
            unity_to_native_scale(parent_scale),
        ),
        compose_matrix(
            unity_to_native_vec3(child_position),
            unity_to_native_quaternion(child_rotation),
            unity_to_native_scale(child_scale),
        ),
    );
    let expected = mat_mul(mat_mul(h_matrix(), unity_world), h_matrix());

    assert_matrix_close(native_world, expected);
}

#[test]
fn inverse_transpose_normal_stays_perpendicular_after_nonuniform_scale() {
    let matrix = compose_matrix(
        (8.0, -2.0, 5.0),
        axis_angle((1.0, -2.0, 0.5), 0.81),
        (0.5, 3.0, 1.25),
    );
    let normal_length = 3.0_f64.sqrt();
    let normal = (
        1.0 / normal_length,
        1.0 / normal_length,
        1.0 / normal_length,
    );
    let tangent_a = (1.0, -1.0, 0.0);
    let tangent_b = (1.0, 1.0, -2.0);
    let baked_normal = transform_normal(matrix, normal).expect("invertible normal matrix");

    assert!(dot(baked_normal, transform_direction(matrix, tangent_a)).abs() < 1.0e-10);
    assert!(dot(baked_normal, transform_direction(matrix, tangent_b)).abs() < 1.0e-10);
    assert!((dot(baked_normal, baked_normal) - 1.0).abs() < 1.0e-10);
}

#[test]
fn singular_normal_transform_fails_closed() {
    let singular = compose_matrix(
        (0.0, 0.0, 0.0),
        axis_angle((0.0, 1.0, 0.0), 0.3),
        (1.0, 0.0, 2.0),
    );

    assert_eq!(transform_normal(singular, (0.0, 1.0, 0.0)), None);

    let env = UnityEnvironment::from_assets(Vec::new());
    let preview = mesh_to_preview(
        &env,
        &simple_clockwise_triangle_mesh(),
        "test.asset",
        1,
        None,
        singular,
        &[],
        "mesh",
    )
    .expect("diagnostic preview");
    assert_eq!(
        preview["normalTransformStatus"],
        json!("singular-linear-transform-normals-omitted")
    );
    assert_eq!(preview["normals"], json!([]));
}

#[test]
fn h_reflection_converts_unity_clockwise_winding_without_index_swap() {
    let mesh = extract_mesh(&simple_clockwise_triangle_mesh()).expect("triangle mesh");
    let native = mesh.vertices;
    let edge_a = (
        native[1].0 - native[0].0,
        native[1].1 - native[0].1,
        native[1].2 - native[0].2,
    );
    let edge_b = (
        native[2].0 - native[0].0,
        native[2].1 - native[0].1,
        native[2].2 - native[0].2,
    );
    let cross_z = edge_a.0 * edge_b.1 - edge_a.1 * edge_b.0;

    assert!(cross_z > 0.0, "the mirrored native triangle must be CCW");
    assert_eq!(mesh.triangles, vec![vec![0_u32, 1, 2]]);
}

#[test]
fn transform_and_collider_summaries_are_native_and_self_describing() {
    let transform = UnityValue::Object(BTreeMap::from([
        ("m_LocalPosition".into(), vec3_value((2.0, -3.0, 4.0))),
        (
            "m_LocalRotation".into(),
            quaternion_value((0.1, 0.2, -0.3, 0.9)),
        ),
        ("m_LocalScale".into(), vec3_value((0.5, 1.5, 2.5))),
        ("m_Children".into(), UnityValue::Array(Vec::new())),
    ]));
    let collider = UnityValue::Object(BTreeMap::from([
        ("m_Center".into(), vec3_value((6.0, -7.0, 8.0))),
        ("m_Size".into(), vec3_value((1.0, 2.0, 3.0))),
    ]));

    let transform_summary = summarize_transform(10, &transform);
    assert_eq!(transform_summary["coordinateSpace"], json!("native"));
    assert_eq!(
        transform_summary["coordinateContract"]["basis"],
        json!("H=diag(-1,1,1)")
    );
    assert_eq!(
        transform_summary["position"],
        json!({ "x": -2.0, "y": -3.0, "z": 4.0 })
    );
    assert_eq!(
        transform_summary["rotation"],
        json!({ "x": 0.1, "y": -0.2, "z": 0.3, "w": 0.9 })
    );
    assert_eq!(
        transform_summary["scale"],
        json!({ "x": 0.5, "y": 1.5, "z": 2.5 })
    );

    let collider_summary = summarize_collider(11, "BoxCollider", &collider);
    assert_eq!(collider_summary["coordinateSpace"], json!("native"));
    assert_eq!(
        collider_summary["center"],
        json!({ "x": -6.0, "y": -7.0, "z": 8.0 })
    );
    assert_eq!(
        collider_summary["size"],
        json!({ "x": 1.0, "y": 2.0, "z": 3.0 })
    );
}
