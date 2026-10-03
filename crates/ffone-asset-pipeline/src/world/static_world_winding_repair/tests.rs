use super::*;

#[test]
fn glb_repair_swaps_only_index_payload_and_aligns_normals() {
    let source = fixture_glb([0, 1, 2], true);
    let before = analyze_glb(&source, "fixture.glb").unwrap();
    assert_eq!(before.opposed, 1);
    assert_eq!(before.aligned, 0);
    assert!(before.needs_reversal());

    let mut repaired = source.clone();
    reverse_glb_indices(&mut repaired, "fixture.glb").unwrap();
    let after = analyze_glb(&repaired, "fixture.glb").unwrap();
    assert_eq!(after.aligned, 1);
    assert_eq!(after.opposed, 0);
    assert_eq!(source.len(), repaired.len());

    reverse_glb_indices(&mut repaired, "fixture.glb").unwrap();
    assert_eq!(repaired, source);
}

#[test]
fn aligned_and_normal_less_payloads_choose_explicit_policies() {
    let aligned = fixture_glb([0, 2, 1], true);
    assert!(
        !analyze_glb(&aligned, "aligned.glb")
            .unwrap()
            .needs_reversal()
    );
    let normal_less = fixture_glb([0, 2, 1], false);
    assert!(
        analyze_glb(&normal_less, "normal-less.glb")
            .unwrap()
            .needs_reversal()
    );
}

fn fixture_glb(indices: [u16; 3], include_normals: bool) -> Vec<u8> {
    let mut binary = Vec::new();
    for value in [[0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        for component in value {
            binary.extend_from_slice(&component.to_le_bytes());
        }
    }
    let normal_offset = binary.len();
    if include_normals {
        for _ in 0..3 {
            for component in [0.0_f32, 0.0, -1.0] {
                binary.extend_from_slice(&component.to_le_bytes());
            }
        }
    }
    let index_offset = binary.len();
    for index in indices {
        binary.extend_from_slice(&index.to_le_bytes());
    }
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let normal_view = include_normals.then_some(1);
    let index_view = if include_normals { 2 } else { 1 };
    let normal_accessor = include_normals.then_some(1);
    let index_accessor = if include_normals { 2 } else { 1 };
    let mut views = vec![serde_json::json!({
        "buffer": 0, "byteOffset": 0, "byteLength": 36
    })];
    if include_normals {
        views.push(serde_json::json!({
            "buffer": 0, "byteOffset": normal_offset, "byteLength": 36
        }));
    }
    views.push(serde_json::json!({
        "buffer": 0, "byteOffset": index_offset, "byteLength": 6
    }));
    let mut accessors = vec![serde_json::json!({
        "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"
    })];
    if include_normals {
        accessors.push(serde_json::json!({
            "bufferView": normal_view, "componentType": 5126, "count": 3, "type": "VEC3"
        }));
    }
    accessors.push(serde_json::json!({
        "bufferView": index_view, "componentType": 5123, "count": 3, "type": "SCALAR"
    }));
    let mut attributes = JsonMap::new();
    attributes.insert("POSITION".to_owned(), JsonValue::from(0));
    if let Some(normal_accessor) = normal_accessor {
        attributes.insert("NORMAL".to_owned(), JsonValue::from(normal_accessor));
    }
    let document = serde_json::json!({
        "asset": {"version": "2.0"},
        "buffers": [{"byteLength": binary.len()}],
        "bufferViews": views,
        "accessors": accessors,
        "meshes": [{"primitives": [{
            "attributes": attributes,
            "indices": index_accessor,
            "mode": 4
        }]}]
    });
    let mut json = serde_json::to_vec(&document).unwrap();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + binary.len();
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(&GLB_JSON_CHUNK.to_le_bytes());
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(&GLB_BIN_CHUNK.to_le_bytes());
    glb.extend_from_slice(&binary);
    glb
}
