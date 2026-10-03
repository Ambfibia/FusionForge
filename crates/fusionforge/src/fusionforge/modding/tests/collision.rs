use super::*;

#[test]
fn apply_mesh_import_writes_collision_triangles_from_source_indices() {
    let mesh = ImportedMesh {
        name: Some("otto".to_string()),
        vertices: vec![
            (0.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
            (0.0, 0.0, 1.0),
        ],
        normals: vec![(0.0, 0.0, 1.0); 4],
        uvs: vec![(0.0, 0.0); 4],
        indices: vec![0, 1, 2, 2, 1, 3],
        submeshes: Vec::new(),
        skin: Vec::new(),
        bind_poses: Vec::new(),
        joint_names: Vec::new(),
    };
    let mut target = UnityValue::Object(BTreeMap::new());

    apply_mesh_import(&mut target, mesh).unwrap();

    let object = target.as_object().unwrap();
    let collision_triangles = object
        .get("m_CollisionTriangles")
        .and_then(UnityValue::as_array)
        .unwrap()
        .iter()
        .filter_map(UnityValue::as_i64)
        .collect::<Vec<_>>();
    assert_eq!(collision_triangles, vec![0, 1, 2, 2, 1, 3]);
    assert_eq!(
        object
            .get("m_CollisionVertexCount")
            .and_then(UnityValue::as_i64),
        Some(4)
    );
}

#[test]
fn collision_mesh_indices_weld_duplicate_render_positions() {
    let mesh = ImportedMesh {
        name: None,
        vertices: vec![
            (0.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
            (0.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
        ],
        normals: vec![(0.0, 0.0, 1.0); 6],
        uvs: vec![(0.0, 0.0); 6],
        indices: vec![0, 1, 2, 3, 4, 5],
        submeshes: vec![ImportedSubMesh {
            first_index: 0,
            index_count: 6,
        }],
        skin: Vec::new(),
        bind_poses: Vec::new(),
        joint_names: Vec::new(),
    };

    let (collision_indices, collision_vertex_count) = collision_mesh_indices(&mesh);

    assert_eq!(collision_indices, vec![0, 1, 2, 0, 1, 2]);
    assert_eq!(collision_vertex_count, 3);
}

#[test]
fn npc_otto_collision_mesh_welds_duplicate_render_positions() {
    let repo_root = crate::repository_root().to_path_buf()
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let mesh = ImportedMesh::from_gltf_path(&path, None).expect("otto mesh");

    let (collision_indices, collision_vertex_count) = collision_mesh_indices(&mesh);

    assert_eq!(collision_indices.len(), mesh.indices.len());
    assert!(collision_vertex_count > 0);
    assert!(collision_vertex_count <= mesh.vertices.len());
    assert!(
        collision_vertex_count < mesh.vertices.len(),
        "expected Otto collision mesh to weld duplicate render positions"
    );
}

#[test]
fn append_triangle_strips_roundtrips_simple_triangles() {
    let triangles = vec![0, 1, 2, 2, 1, 3];
    let mut strip = Vec::new();
    append_triangle_strips(&triangles, &mut strip);

    assert_eq!(strip_to_triangles_u16(&strip), triangles);
}

#[test]
fn npc_otto_triangle_strip_roundtrips_source_triangles() {
    let repo_root = crate::repository_root().to_path_buf()
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let mesh = ImportedMesh::from_gltf_path(&path, None).expect("otto mesh");
    let mut strip = Vec::new();
    append_triangle_strips(&mesh.indices, &mut strip);
    let actual = strip_to_triangles_u16(&strip).chunks_exact(3).fold(
        BTreeMap::<[u16; 3], usize>::new(),
        |mut counts, triangle| {
            let mut key = [triangle[0], triangle[1], triangle[2]];
            key.sort_unstable();
            *counts.entry(key).or_default() += 1;
            counts
        },
    );
    let expected = mesh.indices.chunks_exact(3).fold(
        BTreeMap::<[u16; 3], usize>::new(),
        |mut counts, triangle| {
            let mut key = [triangle[0], triangle[1], triangle[2]];
            key.sort_unstable();
            *counts.entry(key).or_default() += 1;
            counts
        },
    );

    assert_eq!(actual, expected);
}

#[test]
fn npc_otto_triangle_strips_report_segment_count() {
    let repo_root = crate::repository_root().to_path_buf()
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let mesh = ImportedMesh::from_gltf_path(&path, None).expect("otto mesh");

    let strips = build_triangle_strips(&mesh.indices);
    let triangle_total = strips
        .iter()
        .map(|strip| strip_to_triangles_u16(strip).len() / 3)
        .sum::<usize>();

    assert_eq!(triangle_total, mesh.indices.len() / 3);
    assert!(!strips.is_empty());
    println!(
        "otto strips={} maxStripTriangles={} avgStripTriangles={:.2}",
        strips.len(),
        strips
            .iter()
            .map(|strip| strip_to_triangles_u16(strip).len() / 3)
            .max()
            .unwrap_or(0),
        triangle_total as f64 / strips.len() as f64
    );
}
