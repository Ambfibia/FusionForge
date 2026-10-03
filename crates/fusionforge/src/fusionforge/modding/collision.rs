use super::*;

/// Flips triangles whose geometric (winding) normal opposes the authored
/// vertex normals. Blender's double-sided viewport hides inverted-winding
/// islands, so exported GLBs regularly contain them; the legacy toon shaders
/// cull back faces in the main pass and paint front-culled geometry with the
/// black outline pass, so every inverted triangle shows up in-game as a black
/// face / hole.
pub(super) fn align_triangle_winding_with_normals(
    vertices: &[(f64, f64, f64)],
    normals: &[(f64, f64, f64)],
    indices: &mut [u16],
) {
    if normals.len() != vertices.len() {
        return;
    }
    let mut flipped = 0_usize;
    for triangle in indices.chunks_exact_mut(3) {
        let [a, b, c] = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        if a >= vertices.len() || b >= vertices.len() || c >= vertices.len() {
            continue;
        }
        let (pa, pb, pc) = (vertices[a], vertices[b], vertices[c]);
        let e1 = (pb.0 - pa.0, pb.1 - pa.1, pb.2 - pa.2);
        let e2 = (pc.0 - pa.0, pc.1 - pa.1, pc.2 - pa.2);
        let geometric = (
            e1.1 * e2.2 - e1.2 * e2.1,
            e1.2 * e2.0 - e1.0 * e2.2,
            e1.0 * e2.1 - e1.1 * e2.0,
        );
        let (na, nb, nc) = (normals[a], normals[b], normals[c]);
        let average = (na.0 + nb.0 + nc.0, na.1 + nb.1 + nc.1, na.2 + nb.2 + nc.2);
        let geometric_len =
            (geometric.0 * geometric.0 + geometric.1 * geometric.1 + geometric.2 * geometric.2)
                .sqrt();
        let average_len =
            (average.0 * average.0 + average.1 * average.1 + average.2 * average.2).sqrt();
        if geometric_len < 1.0e-12 || average_len < 1.0e-9 {
            continue;
        }
        let alignment =
            (geometric.0 * average.0 + geometric.1 * average.1 + geometric.2 * average.2)
                / (geometric_len * average_len);
        if alignment < -0.05 {
            triangle.swap(0, 1);
            flipped += 1;
        }
    }
    if flipped > 0 {
        eprintln!(
            "Aligned winding of {flipped} triangle(s) with the authored vertex normals to avoid black back-culled faces."
        );
    }
}

pub(super) fn collision_mesh_indices(mesh: &ImportedMesh) -> (Vec<u16>, usize) {
    let mut remapped = Vec::with_capacity(mesh.indices.len());
    let mut by_position = BTreeMap::<(u64, u64, u64), u16>::new();
    for &source_index in &mesh.indices {
        let Some(&(x, y, z)) = mesh.vertices.get(source_index as usize) else {
            continue;
        };
        let key = (x.to_bits(), y.to_bits(), z.to_bits());
        let collision_index = if let Some(index) = by_position.get(&key) {
            *index
        } else {
            let index = by_position.len() as u16;
            by_position.insert(key, index);
            index
        };
        remapped.push(collision_index);
    }
    (remapped, by_position.len())
}

pub(super) fn append_triangle_strips(triangle_indices: &[u16], out: &mut Vec<u16>) {
    for strip in build_triangle_strips(triangle_indices) {
        append_strip(&strip, out);
    }
}

pub(super) fn build_triangle_strips(triangle_indices: &[u16]) -> Vec<Vec<u16>> {
    let triangles = triangle_indices
        .chunks_exact(3)
        .map(|triangle| [triangle[0], triangle[1], triangle[2]])
        .collect::<Vec<_>>();
    let adjacency = build_triangle_strip_adjacency(&triangles);
    let mut used = vec![false; triangles.len()];
    let mut strips = Vec::new();

    while let Some(strip) = best_unused_triangle_strip(&triangles, &adjacency, &used) {
        mark_strip_triangles_used(&strip, &triangles, &mut used);
        strips.push(strip);
    }
    strips
}

pub(super) fn best_unused_triangle_strip(
    triangles: &[[u16; 3]],
    adjacency: &BTreeMap<(u16, u16), Vec<usize>>,
    used: &[bool],
) -> Option<Vec<u16>> {
    let mut best = None::<Vec<u16>>;
    for start_index in 0..triangles.len() {
        if used.get(start_index).copied().unwrap_or(true) {
            continue;
        }
        let candidate = build_best_triangle_strip(start_index, triangles, adjacency, used);
        if best
            .as_ref()
            .is_none_or(|current| candidate.len() > current.len())
        {
            best = Some(candidate);
        }
    }
    best
}

pub(super) fn build_triangle_strip_adjacency(triangles: &[[u16; 3]]) -> BTreeMap<(u16, u16), Vec<usize>> {
    let mut adjacency = BTreeMap::<(u16, u16), Vec<usize>>::new();
    for (index, triangle) in triangles.iter().enumerate() {
        for &(a, b) in &[
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let key = if a <= b { (a, b) } else { (b, a) };
            adjacency.entry(key).or_default().push(index);
        }
    }
    adjacency
}

pub(super) fn build_best_triangle_strip(
    start_index: usize,
    triangles: &[[u16; 3]],
    adjacency: &BTreeMap<(u16, u16), Vec<usize>>,
    used: &[bool],
) -> Vec<u16> {
    let triangle = triangles[start_index];
    let rotations = [
        [triangle[0], triangle[1], triangle[2]],
        [triangle[1], triangle[2], triangle[0]],
        [triangle[2], triangle[0], triangle[1]],
    ];
    let mut best = rotations[0].to_vec();
    for rotation in rotations {
        let candidate =
            build_triangle_strip_from_seed(start_index, rotation, triangles, adjacency, used);
        if candidate.len() > best.len() {
            best = candidate;
        }
    }
    best
}

pub(super) fn build_triangle_strip_from_seed(
    start_index: usize,
    seed: [u16; 3],
    triangles: &[[u16; 3]],
    adjacency: &BTreeMap<(u16, u16), Vec<usize>>,
    used: &[bool],
) -> Vec<u16> {
    let mut strip = seed.to_vec();
    let mut local_used = BTreeSet::from([start_index]);
    // Grow the strip at the tail only. Head growth via
    // reverse-append-reverse can never preserve triangle winding: reversing
    // an N-vertex strip keeps winding only for even N, while un-reversing
    // the extended strip needs odd N, so every head extension flipped the
    // whole strip and the back-culling toon passes rendered those triangles
    // as black faces/holes in-game.
    loop {
        let Some((triangle_index, next_vertex, _)) =
            find_next_strip_vertex(&strip, triangles, adjacency, used, &local_used)
        else {
            break;
        };
        local_used.insert(triangle_index);
        strip.push(next_vertex);

        if strip.len() > triangles.len().saturating_mul(3) {
            break;
        }
    }
    strip
}

pub(super) fn triangle_strip_neighbor_count(
    triangle_index: usize,
    triangles: &[[u16; 3]],
    adjacency: &BTreeMap<(u16, u16), Vec<usize>>,
    used: &[bool],
) -> usize {
    let triangle = triangles[triangle_index];
    [
        (triangle[0], triangle[1]),
        (triangle[1], triangle[2]),
        (triangle[2], triangle[0]),
    ]
    .into_iter()
    .map(|(a, b)| {
        adjacency
            .get(&undirected_edge_key(a, b))
            .map(|indices| {
                indices
                    .iter()
                    .filter(|&&index| index != triangle_index && !used[index])
                    .count()
            })
            .unwrap_or(0)
    })
    .sum()
}

pub(super) fn same_triangle_vertices(left: [u16; 3], right: [u16; 3]) -> bool {
    let mut left_sorted = left;
    left_sorted.sort_unstable();
    let mut right_sorted = right;
    right_sorted.sort_unstable();
    left_sorted == right_sorted
}
