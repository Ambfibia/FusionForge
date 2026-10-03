use super::*;

#[test]
fn npc_otto_bundle_triangle_strip_indices_fit_render_vertices() {
    let repo_root = crate::repository_root().parent().unwrap();
    let dump_path = repo_root.join("work").join("otto_after_render_dedupe.json");
    let mesh = dump_object_value(&dump_path, "Mesh");

    let render_vertex_count = fusionforge::mesh_vertex_count(&mesh);
    let triangle_indices = fusionforge::read_packed_bits(
        mesh.get("m_CompressedMesh")
            .and_then(|value| value.get("m_Triangles"))
            .expect("triangle indices"),
    );
    let max_triangle_index = triangle_indices.iter().copied().max().unwrap_or(0) as usize;

    assert!(
        render_vertex_count > 0,
        "otto render vertex count was empty"
    );
    assert!(
        max_triangle_index < render_vertex_count,
        "otto bundle max triangle index {max_triangle_index} exceeded render vertex count {render_vertex_count}"
    );
}
