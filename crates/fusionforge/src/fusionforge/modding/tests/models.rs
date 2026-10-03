use super::*;

pub(super) fn temp_gltf_path() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!(
        "ffclienteditor_animation_import_{}_{}.gltf",
        std::process::id(),
        nonce
    ))
}

pub(super) fn write_skin_bindpose_gltf(
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
) -> PathBuf {
    let path = temp_gltf_path();
    let bin_path = path.with_extension("bin");

    let local = trs_matrix4_f32(translation, rotation, scale);
    let inverse = invert_affine_matrix4_f32(local);

    let mut bytes = Vec::new();
    for col in 0..4 {
        for row in 0..4 {
            push_f32(&mut bytes, inverse[row][col]);
        }
    }
    fs::write(&bin_path, &bytes).unwrap();

    let json = json!({
        "asset": { "version": "2.0" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{
            "name": "Joint",
            "translation": translation,
            "rotation": rotation,
            "scale": scale
        }],
        "skins": [{
            "joints": [0],
            "inverseBindMatrices": 0
        }],
        "buffers": [{
            "byteLength": bytes.len(),
            "uri": bin_path.file_name().unwrap().to_string_lossy()
        }],
        "bufferViews": [{
            "buffer": 0,
            "byteOffset": 0,
            "byteLength": bytes.len()
        }],
        "accessors": [{
            "bufferView": 0,
            "byteOffset": 0,
            "componentType": 5126,
            "count": 1,
            "type": "MAT4"
        }]
    });
    fs::write(&path, serde_json::to_vec_pretty(&json).unwrap()).unwrap();
    path
}

#[test]
fn npc_otto_compressed_skin_stream_matches_glb_influence_histogram() {
    let repo_root = crate::repository_root().to_path_buf()
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let mesh = ImportedMesh::from_gltf_path(&path, None).expect("otto mesh");

    let mut rigid_vertices = 0usize;
    let mut blended_vertices = 0usize;
    let mut expected_weights = 0usize;
    let mut expected_bone_indices = 0usize;
    for skin in &mesh.skin {
        let influences = quantized_skin_influences(skin);
        assert!((1..=4).contains(&influences.len()));
        assert_eq!(
            influences.iter().map(|(_, weight)| *weight).sum::<u32>(),
            31
        );
        expected_bone_indices += influences.len();
        // Weights are stored for the first three influences; only a 4th
        // influence's weight is implicit (31 - sum) in the legacy format.
        expected_weights += influences.len().min(3);
        if influences.len() <= 1 {
            rigid_vertices += 1;
        } else {
            blended_vertices += 1;
        }
    }

    let (weights, bone_indices) = compressed_skin_streams(&mesh);

    assert_eq!(mesh.skin.len(), 1737);
    assert_eq!(rigid_vertices, 1411);
    assert_eq!(blended_vertices, 326);
    assert_eq!(expected_bone_indices, 2078);
    assert_eq!(bone_indices.len(), expected_bone_indices);
    assert_eq!(weights.len(), expected_weights);

    // Simulate the legacy web player decoder: it must consume both
    // streams exactly, never running out of weights before the last
    // vertex (a desync garbles the skin or crashes the old runtime).
    let mut wi = 0usize;
    let mut ii = 0usize;
    for _ in 0..mesh.skin.len() {
        let mut sum = 0u32;
        let mut j = 0usize;
        loop {
            if j == 3 {
                ii += 1;
                break;
            }
            sum += weights[wi];
            wi += 1;
            ii += 1;
            j += 1;
            if sum >= 31 {
                break;
            }
        }
    }
    assert_eq!(wi, weights.len());
    assert_eq!(ii, bone_indices.len());
}

#[test]
fn dedupe_imported_vertex_streams_merges_identical_full_vertices() {
    let mut vertices = vec![
        (0.0, 0.0, 0.0),
        (1.0, 0.0, 0.0),
        (0.0, 1.0, 0.0),
        (0.0, 0.0, 0.0),
        (1.0, 0.0, 0.0),
        (0.0, 1.0, 0.0),
    ];
    let mut normals = vec![(0.0, 0.0, 1.0); 6];
    let mut uvs = vec![
        (0.0, 0.0),
        (1.0, 0.0),
        (0.0, 1.0),
        (0.0, 0.0),
        (1.0, 0.0),
        (0.0, 1.0),
    ];
    let mut skin = vec![
        ImportedBoneWeight {
            weights: [1.0, 0.0, 0.0, 0.0],
            bone_indices: [0, 0, 0, 0]
        };
        6
    ];
    let mut indices = vec![0, 1, 2, 3, 4, 5];

    dedupe_imported_vertex_streams(
        &mut vertices,
        &mut normals,
        &mut uvs,
        &mut skin,
        &mut indices,
    );

    assert_eq!(vertices.len(), 3);
    assert_eq!(normals.len(), 3);
    assert_eq!(uvs.len(), 3);
    assert_eq!(skin.len(), 3);
    assert_eq!(indices, vec![0, 1, 2, 0, 1, 2]);
}
