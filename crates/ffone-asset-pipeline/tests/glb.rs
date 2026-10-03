use ffone_asset_pipeline::{PipelineError, mesh_json_to_glb};
use serde_json::Value;

const TWO_SUBMESH_MESH: &str = r#"{
  "schema": "ffone.mesh.v1",
  "name": "Native Quad",
  "positions": [[-1.0, 0.0, -2.0], [1.0, 0.0, -2.0], [1.0, 0.0, 2.0], [-1.0, 0.0, 2.0]],
  "normals": [[0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
  "uvs": [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
  "submeshes": [{"indices": [0, 1, 2]}, {"indices": [0, 2, 3]}]
}"#;

#[test]
fn glb_header_chunks_and_primitives_are_golden() {
    let glb = mesh_json_to_glb("meshes/native-quad.json", TWO_SUBMESH_MESH.as_bytes()).unwrap();
    assert_eq!(&glb[0..4], b"glTF");
    assert_eq!(u32_at(&glb, 4), 2);
    assert_eq!(u32_at(&glb, 8) as usize, glb.len());

    let json_len = u32_at(&glb, 12) as usize;
    assert_eq!(json_len % 4, 0);
    assert_eq!(u32_at(&glb, 16), 0x4e4f_534a);
    let json_start = 20;
    let json_end = json_start + json_len;
    let document: Value =
        serde_json::from_slice(trim_json_padding(&glb[json_start..json_end])).unwrap();
    assert_eq!(document["asset"]["version"], "2.0");
    assert_eq!(document["asset"]["generator"], "ffone-asset-pipeline");
    assert_eq!(document["meshes"][0]["name"], "Native Quad");
    assert_eq!(
        document["meshes"][0]["primitives"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        document["meshes"][0]["primitives"][0]["extras"]["ffoneSubmesh"],
        0
    );
    assert_eq!(
        document["meshes"][0]["primitives"][1]["extras"]["ffoneSubmesh"],
        1
    );
    assert_eq!(
        document["meshes"][0]["primitives"][0]["attributes"]["POSITION"],
        0
    );
    assert_eq!(
        document["meshes"][0]["primitives"][0]["attributes"]["NORMAL"],
        1
    );
    assert_eq!(
        document["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_0"],
        2
    );
    assert_eq!(
        document["accessors"][0]["min"],
        serde_json::json!([-1.0, 0.0, -2.0])
    );
    assert_eq!(
        document["accessors"][0]["max"],
        serde_json::json!([1.0, 0.0, 2.0])
    );
    assert_eq!(document["accessors"][3]["componentType"], 5_123);
    assert_eq!(document["accessors"][4]["componentType"], 5_123);
    assert_eq!(document["bufferViews"][0]["target"], 34_962);
    assert_eq!(document["bufferViews"][3]["target"], 34_963);

    let bin_header = json_end;
    let bin_len = u32_at(&glb, bin_header) as usize;
    assert_eq!(bin_len, 144);
    assert_eq!(u32_at(&glb, bin_header + 4), 0x004e_4942);
    assert_eq!(bin_header + 8 + bin_len, glb.len());
    let bin = &glb[bin_header + 8..];
    assert_eq!(f32_at(bin, 0), -1.0);
    assert_eq!(f32_at(bin, 4), 0.0);
    assert_eq!(f32_at(bin, 8), -2.0);
    assert_eq!(&bin[128..134], &[0, 0, 2, 0, 1, 0]);
    assert_eq!(&bin[136..142], &[0, 0, 3, 0, 2, 0]);

    assert_eq!(
        blake3::hash(&glb).to_hex().to_string(),
        "1e2c85973e1bd815eaa41cf95d98568d16f24fd9d97f376bce83aece3f6f9b7f"
    );
}

#[test]
fn converted_winding_geometric_normal_agrees_with_stored_normal() {
    let glb = mesh_json_to_glb("meshes/native-quad.json", TWO_SUBMESH_MESH.as_bytes()).unwrap();
    let json_len = u32_at(&glb, 12) as usize;
    let binary = &glb[20 + json_len + 8..];

    let positions = [
        vec3_at(binary, 0),
        vec3_at(binary, 12),
        vec3_at(binary, 24),
        vec3_at(binary, 36),
    ];
    let stored_normal = vec3_at(binary, 48);
    let indices = [
        u16_at(binary, 128) as usize,
        u16_at(binary, 130) as usize,
        u16_at(binary, 132) as usize,
    ];

    let edge_a = subtract(positions[indices[1]], positions[indices[0]]);
    let edge_b = subtract(positions[indices[2]], positions[indices[0]]);
    let geometric_normal = cross(edge_a, edge_b);
    assert!(
        dot(geometric_normal, stored_normal) > 0.0,
        "converted geometric normal {geometric_normal:?} must face the stored normal \
         {stored_normal:?}; emitted indices were {indices:?}"
    );
}

#[test]
fn cardinality_and_triangle_indices_are_strict() {
    let wrong_normals = TWO_SUBMESH_MESH.replace(
        "[0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0]",
        "[0.0, 1.0, 0.0]",
    );
    assert_invalid_mesh(&wrong_normals, "normal count 1");

    let out_of_range = TWO_SUBMESH_MESH.replace("[0, 2, 3]", "[0, 2, 4]");
    assert_invalid_mesh(&out_of_range, "index 4 is outside vertex count 4");

    let incomplete_triangle = TWO_SUBMESH_MESH.replace("[0, 2, 3]", "[0, 2]");
    assert_invalid_mesh(&incomplete_triangle, "not divisible by three");
}

#[test]
fn finite_f64_that_overflows_f32_is_rejected() {
    let overflow = TWO_SUBMESH_MESH.replacen("-1.0", "1e100", 1);
    assert_invalid_mesh(&overflow, "position 0 contains a non-finite value");
}

#[test]
fn legacy_mesh_path_is_rejected_before_conversion() {
    let error = mesh_json_to_glb("models/source.obj", TWO_SUBMESH_MESH.as_bytes()).unwrap_err();
    assert!(matches!(error, PipelineError::ForbiddenLegacy { .. }));
}

fn assert_invalid_mesh(json: &str, expected: &str) {
    let error = mesh_json_to_glb("meshes/invalid.json", json.as_bytes()).unwrap_err();
    match error {
        PipelineError::InvalidMesh { reason, .. } => assert!(
            reason.contains(expected),
            "expected {expected:?} in {reason:?}"
        ),
        other => panic!("unexpected error: {other}"),
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn f32_at(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn vec3_at(bytes: &[u8], offset: usize) -> [f32; 3] {
    [
        f32_at(bytes, offset),
        f32_at(bytes, offset + 4),
        f32_at(bytes, offset + 8),
    ]
}

fn subtract(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn trim_json_padding(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|byte| *byte != b' ')
        .map_or(0, |index| index + 1);
    &bytes[..end]
}
