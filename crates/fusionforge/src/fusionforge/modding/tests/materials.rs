use super::*;

#[test]
fn npc_otto_gltf_import_dedupes_render_vertices() {
    let repo_root = crate::repository_root().to_path_buf()
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let mesh = ImportedMesh::from_gltf_path(&path, None).expect("otto mesh");

    assert_eq!(mesh.vertices.len(), 1737);
    assert_eq!(mesh.normals.len(), 1737);
    assert_eq!(mesh.uvs.len(), 1737);
    assert_eq!(mesh.skin.len(), 1737);
}

#[test]
fn npc_otto_reports_quantized_render_vertex_uniques() {
    let repo_root = crate::repository_root().to_path_buf()
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let mesh = ImportedMesh::from_gltf_path(&path, None).expect("otto mesh");

    let mut exact = BTreeSet::new();
    let mut quantized = BTreeSet::new();
    for index in 0..mesh.vertices.len() {
        exact.insert((
            mesh.vertices[index].0.to_bits(),
            mesh.vertices[index].1.to_bits(),
            mesh.vertices[index].2.to_bits(),
            mesh.normals[index].0.to_bits(),
            mesh.normals[index].1.to_bits(),
            mesh.normals[index].2.to_bits(),
            mesh.uvs[index].0.to_bits(),
            mesh.uvs[index].1.to_bits(),
        ));
        quantized.insert(quantized_vertex_key(&mesh, index));
    }

    println!(
        "otto exact_render_unique={} quantized_render_unique={} savings={}",
        exact.len(),
        quantized.len(),
        exact.len().saturating_sub(quantized.len())
    );
    assert!(!quantized.is_empty());
}
