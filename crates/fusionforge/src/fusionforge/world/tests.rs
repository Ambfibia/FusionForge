use std::collections::BTreeMap;

use super::*;

fn vec3_value(value: super::super::coordinates::Vec3) -> UnityValue {
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

#[test]
fn scene_transform_component_serializes_native_rotation_without_gameplay_facing() {
    let body = UnityValue::Object(BTreeMap::from([
        ("m_LocalPosition".into(), vec3_value((2.0, -3.0, 4.0))),
        (
            "m_LocalRotation".into(),
            quaternion_value((0.1, 0.2, -0.3, 0.9)),
        ),
        ("m_LocalScale".into(), vec3_value((0.5, 1.5, 2.5))),
        ("m_Children".into(), UnityValue::Array(Vec::new())),
    ]));
    let local = compose_matrix(
        converted_position(body.get("m_LocalPosition")),
        converted_quaternion(body.get("m_LocalRotation")),
        converted_scale(body.get("m_LocalScale")),
    );
    let summary = transform_component_json(&body, local);

    assert_eq!(summary["coordinateSpace"], json!("native"));
    assert_eq!(
        summary["coordinateContract"]["gameplayFacingRotationApplied"],
        json!(false)
    );
    assert_eq!(
        summary["localPosition"],
        json!({ "x": -2.0, "y": -3.0, "z": 4.0 })
    );
    assert_eq!(
        summary["localRotation"],
        json!({ "x": 0.1, "y": -0.2, "z": 0.3, "w": 0.9 })
    );
    assert_eq!(
        summary["localScale"],
        json!({ "x": 0.5, "y": 1.5, "z": 2.5 })
    );
}

#[test]
fn map_00_01_authored_root_and_terrain_origin_golden_use_native_axes() {
    // Values recovered from the serialized Map_00_01 hierarchy. This is a
    // coordinate-contract golden, not an auto-centering adjustment.
    let map_root_unity = (0.0, 0.0, 512.0);
    let terrain_local_unity = (0.0, -300.0, 0.0);
    let identity_rotation = (0.0, 0.0, 0.0, 1.0);
    let unit_scale = (1.0, 1.0, 1.0);
    let map_root_native = super::super::coordinates::unity_to_native_vec3(map_root_unity);
    let terrain_local_native =
        super::super::coordinates::unity_to_native_vec3(terrain_local_unity);
    let world = mat_mul_safe(
        compose_matrix(map_root_native, identity_rotation, unit_scale),
        compose_matrix(terrain_local_native, identity_rotation, unit_scale),
    );

    assert_eq!(map_root_native, (0.0, 0.0, 512.0));
    assert_eq!(terrain_local_native, (0.0, -300.0, 0.0));
    assert_eq!(
        transform_point(world, (0.0, 0.0, 0.0)),
        (0.0, -300.0, 512.0)
    );
    assert_eq!(
        super::super::coordinates::unity_to_native_vec3((1.0, 0.0, 0.0)),
        (-1.0, 0.0, 0.0)
    );
    assert_eq!(
        super::super::coordinates::unity_to_native_vec3((0.0, 1.0, 0.0)),
        (0.0, 1.0, 0.0)
    );
    assert_eq!(
        super::super::coordinates::unity_to_native_vec3((0.0, 0.0, 1.0)),
        (0.0, 0.0, 1.0)
    );
}

#[test]
fn map_bundle_coordinates_are_signed_decimal_not_hexadecimal() {
    let parsed = parse_map_bundle_name(Path::new("Map_01_11.unity3d"));
    assert_eq!(parsed, Some((1, 11, 2, 2)));
    assert_eq!(map_tile_id(1, 10, 2, 2), "Map_01_10");
    assert_eq!(map_tile_id(-1, 10, 2, 2), "Map_-01_10");
    assert_eq!(parse_map_bundle_name(Path::new("Map_01_0a.unity3d")), None);
}

#[test]
fn inspects_compressed_example_bundle_natively() {
    let repo_root = crate::repository_root().to_path_buf();
    let build_root = repo_root
        .join("vendor/ffbuildtool")
        .join("example_builds")
        .join("compressed")
        .join("good");
    let result = inspect_world_bundles(WorldInspectOptions {
        repo_root,
        map_bundle: Some(build_root.join("Map_00_00.unity3d")),
        resource_bundle: Some(build_root.join("DongResources_00_09.resourceFile")),
        build_root: Some(build_root),
        neighbor_radius: 0,
    })
    .expect("native world inspection");

    assert!(result["map"]["buildtool"]["extractedFiles"]
        .as_array()
        .is_some_and(|files| !files.is_empty()));
    assert!(result["scenePreview"].is_object());
}
