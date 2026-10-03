use super::*;

impl ImportedModelPreview {
    pub fn from_model_path(
        path: &Path,
        name: Option<String>,
        sample_rate: f64,
    ) -> Result<Self, String> {
        match path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "obj" => Ok(Self {
                mesh: ImportedMesh::from_obj_path(path, name)?,
                skeleton: ImportedSkeleton::default(),
                animations: Vec::new(),
                materials: Vec::new(),
                submesh_material_indices: Vec::new(),
                warnings: vec![
                    "OBJ preview is static; skeleton and animation preview require GLB/GLTF."
                        .to_string(),
                ],
            }),
            "glb" | "gltf" => Self::from_gltf_path(path, name, sample_rate),
            extension => Err(format!(
                "{}: unsupported model format '{}'; expected OBJ, GLB, or GLTF",
                path.display(),
                if extension.is_empty() {
                    "(none)"
                } else {
                    extension
                }
            )),
        }
    }

    pub fn from_gltf_path(
        path: &Path,
        name: Option<String>,
        sample_rate: f64,
    ) -> Result<Self, String> {
        let (document, buffers, images) =
            gltf::import(path).map_err(|err| format!("{}: {err}", path.display()))?;
        let mesh = ImportedMesh::from_gltf_document(path, name, &document, &buffers)?;
        let skeleton = ImportedSkeleton::from_gltf_document(&document);
        let mut warnings = Vec::new();
        let materials = imported_gltf_materials(&document, &images)?;
        let submesh_material_indices = document
            .meshes()
            .flat_map(|mesh| mesh.primitives())
            .filter(|primitive| {
                primitive.mode() == gltf::mesh::Mode::Triangles
                    && primitive
                        .get(&gltf::Semantic::Positions)
                        .is_some_and(|accessor| accessor.count() > 0)
            })
            .map(|primitive| primitive.material().index())
            .collect::<Vec<_>>();
        let animations =
            match ImportedAnimationClip::from_gltf_document(path, sample_rate, &document, &buffers)
            {
                Ok(animations) => animations,
                Err(error) => {
                    warnings.push(format!("Animation preview is unavailable: {error}"));
                    Vec::new()
                }
            };
        if !mesh.joint_names.is_empty() && skeleton.skin_joint_paths.len() != mesh.joint_names.len()
        {
            warnings.push(format!(
                "GLTF skin palette mismatch: mesh has {} joints, preview skeleton resolved {}.",
                mesh.joint_names.len(),
                skeleton.skin_joint_paths.len()
            ));
        }
        if !mesh.bind_poses.is_empty() && mesh.bind_poses.len() != skeleton.skin_joint_paths.len() {
            warnings.push(format!(
                "GLTF inverse bind matrix count {} does not match preview skin palette {}.",
                mesh.bind_poses.len(),
                skeleton.skin_joint_paths.len()
            ));
        }
        if submesh_material_indices.len() != mesh.submeshes.len() {
            warnings.push(format!(
                "GLTF material binding count {} does not match imported submesh count {}.",
                submesh_material_indices.len(),
                mesh.submeshes.len()
            ));
        }
        Ok(Self {
            mesh,
            skeleton,
            animations,
            materials,
            submesh_material_indices,
            warnings,
        })
    }
}

pub fn apply_mesh_import(target: &mut UnityValue, mesh: ImportedMesh) -> Result<(), String> {
    apply_mesh_import_with_options(target, mesh, true)
}

pub fn apply_mesh_import_with_options(
    target: &mut UnityValue,
    mesh: ImportedMesh,
    allow_skin: bool,
) -> Result<(), String> {
    apply_mesh_import_with_format_options(target, mesh, allow_skin, true)
}

pub fn apply_mesh_import_uncompressed_with_options(
    target: &mut UnityValue,
    mesh: ImportedMesh,
    allow_skin: bool,
) -> Result<(), String> {
    apply_mesh_import_with_format_options(target, mesh, allow_skin, false)
}

pub(super) fn apply_mesh_import_with_format_options(
    target: &mut UnityValue,
    mesh: ImportedMesh,
    allow_skin: bool,
    allow_compressed_mesh: bool,
) -> Result<(), String> {
    let object = target
        .as_object_mut()
        .ok_or_else(|| "target mesh is not an object".to_string())?;
    if let Some(name) = &mesh.name {
        object.insert("m_Name".to_string(), UnityValue::String(name.clone()));
    }
    let use_compressed_mesh = allow_compressed_mesh && object.get("m_CompressedMesh").is_some();
    let use_skin = allow_skin && !mesh.skin.is_empty();
    object.insert(
        "m_MeshCompression".to_string(),
        UnityValue::Int(if use_compressed_mesh { 2 } else { 0 }),
    );
    object.insert("m_Use16BitIndices".to_string(), UnityValue::Bool(true));
    if use_compressed_mesh {
        object.insert("m_Vertices".to_string(), UnityValue::Array(Vec::new()));
        object.insert("m_Normals".to_string(), UnityValue::Array(Vec::new()));
        object.insert("m_UV".to_string(), UnityValue::Array(Vec::new()));
    } else {
        object.insert("m_Vertices".to_string(), vector3_array(&mesh.vertices));
        object.insert("m_Normals".to_string(), vector3_array(&mesh.normals));
        object.insert("m_UV".to_string(), vector2_array(&mesh.uvs));
    }
    if use_compressed_mesh || !use_skin {
        object.insert("m_Skin".to_string(), UnityValue::Array(Vec::new()));
    } else if use_skin {
        object.insert(
            "m_Skin".to_string(),
            UnityValue::Array(mesh.skin.iter().map(bone_weight_value).collect()),
        );
    }
    if use_skin && !mesh.bind_poses.is_empty() {
        object.insert(
            "m_BindPose".to_string(),
            UnityValue::Array(mesh.bind_poses.iter().map(matrix4x4_value).collect()),
        );
    } else {
        object.insert("m_BindPose".to_string(), UnityValue::Array(Vec::new()));
    }
    object.insert(
        "m_IndexBuffer".to_string(),
        UnityValue::Bytes(if use_compressed_mesh {
            Vec::new()
        } else {
            mesh.index_buffer()
        }),
    );
    let mut submeshes = if mesh.submeshes.is_empty() {
        vec![ImportedSubMesh {
            first_index: 0,
            index_count: mesh.indices.len(),
        }]
    } else {
        mesh.submeshes.clone()
    };
    let submesh_triangle_counts = submeshes
        .iter()
        .map(|submesh| submesh.index_count / 3)
        .collect::<Vec<_>>();
    let compressed_indices = if use_compressed_mesh {
        let mut remapped_submeshes = Vec::with_capacity(submeshes.len());
        let mut strip_indices = Vec::new();
        for submesh in &submeshes {
            let start = submesh.first_index.min(mesh.indices.len());
            let end = start
                .saturating_add(submesh.index_count)
                .min(mesh.indices.len());
            let first_index = strip_indices.len();
            append_triangle_strips(&mesh.indices[start..end], &mut strip_indices);
            remapped_submeshes.push(ImportedSubMesh {
                first_index,
                index_count: strip_indices.len().saturating_sub(first_index),
            });
        }
        submeshes = remapped_submeshes;
        strip_indices
    } else {
        Vec::new()
    };
    let (collision_indices, collision_vertex_count) = collision_mesh_indices(&mesh);
    let mesh_aabb = mesh_bounds(&mesh.vertices).unwrap_or(((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)));
    object.insert(
        "m_SubMeshes".to_string(),
        UnityValue::Array(
            submeshes
                .iter()
                .enumerate()
                .map(|(index, submesh)| {
                    let first_byte = submesh.first_index.saturating_mul(2);
                    UnityValue::Object(BTreeMap::from([
                        ("firstByte".to_string(), UnityValue::Int(first_byte as i64)),
                        (
                            "indexCount".to_string(),
                            UnityValue::Int(submesh.index_count as i64),
                        ),
                        ("firstVertex".to_string(), UnityValue::Int(0)),
                        (
                            "isTriStrip".to_string(),
                            UnityValue::Int(if use_compressed_mesh { 2 } else { 0 }),
                        ),
                        (
                            "localAABB".to_string(),
                            UnityValue::Object(BTreeMap::from([
                                ("m_Center".to_string(), vector3_object(mesh_aabb.0)),
                                ("m_Extent".to_string(), vector3_object(mesh_aabb.1)),
                            ])),
                        ),
                        (
                            "triangleCount".to_string(),
                            UnityValue::Int(
                                submesh_triangle_counts.get(index).copied().unwrap_or(0) as i64,
                            ),
                        ),
                        ("topology".to_string(), UnityValue::Int(0)),
                        (
                            "vertexCount".to_string(),
                            UnityValue::Int(mesh.vertices.len() as i64),
                        ),
                    ]))
                })
                .collect(),
        ),
    );
    object.insert(
        "m_CollisionTriangles".to_string(),
        UnityValue::Array(
            collision_indices
                .iter()
                .map(|index| UnityValue::Int(i64::from(*index)))
                .collect(),
        ),
    );
    object.insert(
        "m_CollisionVertexCount".to_string(),
        UnityValue::Int(collision_vertex_count as i64),
    );
    if let Some((center, extent)) = mesh_bounds(&mesh.vertices) {
        object.insert(
            "m_LocalAABB".to_string(),
            UnityValue::Object(BTreeMap::from([
                ("m_Center".to_string(), vector3_object(center)),
                ("m_Extent".to_string(), vector3_object(extent)),
            ])),
        );
    }
    if use_compressed_mesh {
        if let Some(compressed) = object
            .get_mut("m_CompressedMesh")
            .and_then(UnityValue::as_object_mut)
        {
            apply_compressed_mesh_import(compressed, &mesh, use_skin, &compressed_indices)?;
        }
    } else if let Some(compressed) = object
        .get_mut("m_CompressedMesh")
        .and_then(UnityValue::as_object_mut)
    {
        clear_compressed_mesh_import(compressed);
    }
    Ok(())
}

pub(super) fn find_next_strip_vertex(
    strip: &[u16],
    triangles: &[[u16; 3]],
    adjacency: &BTreeMap<(u16, u16), Vec<usize>>,
    used: &[bool],
    local_used: &BTreeSet<usize>,
) -> Option<(usize, u16, usize)> {
    if strip.len() < 2 {
        return None;
    }
    let edge_key = undirected_edge_key(strip[strip.len() - 2], strip[strip.len() - 1]);
    let candidate_indices = adjacency.get(&edge_key)?;
    let expected_edge = if strip.len() % 2 == 0 {
        (strip[strip.len() - 2], strip[strip.len() - 1])
    } else {
        (strip[strip.len() - 1], strip[strip.len() - 2])
    };

    let mut best = None::<(usize, usize, usize, u16)>;
    for &triangle_index in candidate_indices {
        if used.get(triangle_index).copied().unwrap_or(true) || local_used.contains(&triangle_index)
        {
            continue;
        }
        let triangle = triangles[triangle_index];
        let Some(vertex) = orientation_preserving_third_vertex(triangle, expected_edge) else {
            continue;
        };
        let mut hypothetical_strip = strip.to_vec();
        hypothetical_strip.push(vertex);
        let mut hypothetical_used = local_used.clone();
        hypothetical_used.insert(triangle_index);
        let extension_score = estimate_bidirectional_strip_extension(
            &hypothetical_strip,
            triangles,
            adjacency,
            used,
            &hypothetical_used,
            6,
        );
        let score = triangle_strip_neighbor_count(triangle_index, triangles, adjacency, used);
        if best.is_none_or(|(best_extension, best_score, _, _)| {
            extension_score > best_extension
                || (extension_score == best_extension && score > best_score)
        }) {
            best = Some((extension_score, score, triangle_index, vertex));
        }
    }
    best.map(|(extension_score, _, triangle_index, vertex)| {
        (triangle_index, vertex, extension_score)
    })
}

pub(super) fn orientation_preserving_third_vertex(
    triangle: [u16; 3],
    expected_edge: (u16, u16),
) -> Option<u16> {
    let rotations = [
        [triangle[0], triangle[1], triangle[2]],
        [triangle[1], triangle[2], triangle[0]],
        [triangle[2], triangle[0], triangle[1]],
    ];
    rotations
        .into_iter()
        .find(|rotation| (rotation[0], rotation[1]) == expected_edge)
        .map(|rotation| rotation[2])
}

pub(super) fn apply_compressed_mesh_import(
    compressed: &mut BTreeMap<String, UnityValue>,
    mesh: &ImportedMesh,
    use_skin: bool,
    strip_indices: &[u16],
) -> Result<(), String> {
    if let Some(value) = compressed
        .get_mut("m_Vertices")
        .and_then(UnityValue::as_object_mut)
    {
        *value = packed_float_vector(mesh.vertices.iter().flat_map(|(x, y, z)| [*x, *y, *z]), 20);
    }
    if let Some(value) = compressed
        .get_mut("m_UV")
        .and_then(UnityValue::as_object_mut)
    {
        *value = packed_float_vector(mesh.uvs.iter().flat_map(|(u, v)| [*u, *v]), 16);
    }
    if let Some(value) = compressed
        .get_mut("m_Triangles")
        .and_then(UnityValue::as_object_mut)
    {
        let max_index = strip_indices.iter().copied().max().unwrap_or(0) as u32;
        *value = packed_int_vector(
            strip_indices.iter().map(|value| u32::from(*value)),
            bit_size_for(max_index.max(1)),
        );
    }
    if let Some(value) = compressed
        .get_mut("m_Normals")
        .and_then(UnityValue::as_object_mut)
    {
        let normal_xy = mesh.normals.iter().flat_map(|(x, y, _)| [*x, *y]);
        *value = packed_float_vector(normal_xy, 8);
    }
    if let Some(value) = compressed
        .get_mut("m_NormalSigns")
        .and_then(UnityValue::as_object_mut)
    {
        *value = packed_int_vector(
            mesh.normals
                .iter()
                .map(|(_, _, z)| if *z >= 0.0 { 1 } else { 0 }),
            1,
        );
    }
    let (compressed_weights, compressed_bone_indices) = if use_skin {
        compressed_skin_streams(mesh)
    } else {
        (Vec::new(), Vec::new())
    };
    if let Some(value) = compressed
        .get_mut("m_Weights")
        .and_then(UnityValue::as_object_mut)
    {
        if use_skin {
            *value = packed_int_vector(compressed_weights.iter().copied(), 5);
        } else {
            clear_packed_vector(value);
        }
    }
    if let Some(value) = compressed
        .get_mut("m_BoneIndices")
        .and_then(UnityValue::as_object_mut)
    {
        if use_skin {
            *value = packed_int_vector(
                compressed_bone_indices.iter().copied(),
                compressed_bone_index_bits(mesh),
            );
        } else {
            clear_packed_vector(value);
        }
    }
    for key in ["m_BindPoses", "m_Tangents", "m_TangentSigns"] {
        if let Some(value) = compressed.get_mut(key).and_then(UnityValue::as_object_mut) {
            clear_packed_vector(value);
        }
    }
    Ok(())
}

pub(super) fn clear_compressed_mesh_import(compressed: &mut BTreeMap<String, UnityValue>) {
    for key in [
        "m_Vertices",
        "m_UV",
        "m_Triangles",
        "m_Normals",
        "m_NormalSigns",
        "m_Weights",
        "m_BoneIndices",
        "m_BindPoses",
        "m_Tangents",
        "m_TangentSigns",
    ] {
        if let Some(value) = compressed.get_mut(key).and_then(UnityValue::as_object_mut) {
            clear_packed_vector(value);
        }
    }
}

pub(super) fn mesh_bounds(vertices: &[(f64, f64, f64)]) -> Option<((f64, f64, f64), (f64, f64, f64))> {
    let first = vertices.first().copied()?;
    let mut min = first;
    let mut max = first;
    for &(x, y, z) in vertices.iter().skip(1) {
        min.0 = min.0.min(x);
        min.1 = min.1.min(y);
        min.2 = min.2.min(z);
        max.0 = max.0.max(x);
        max.1 = max.1.max(y);
        max.2 = max.2.max(z);
    }
    let center = (
        (min.0 + max.0) * 0.5,
        (min.1 + max.1) * 0.5,
        (min.2 + max.2) * 0.5,
    );
    let extent = (
        (max.0 - min.0) * 0.5,
        (max.1 - min.1) * 0.5,
        (max.2 - min.2) * 0.5,
    );
    Some((center, extent))
}

pub(super) fn gltf_node_paths(document: &gltf::Document) -> BTreeMap<usize, String> {
    fn visit(node: gltf::Node<'_>, parent: &str, output: &mut BTreeMap<usize, String>) {
        let name = node
            .name()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("node_{}", node.index()));
        let path = if parent.is_empty() {
            name
        } else {
            format!("{parent}/{name}")
        };
        output.entry(node.index()).or_insert_with(|| path.clone());
        for child in node.children() {
            visit(child, &path, output);
        }
    }

    let mut paths = BTreeMap::new();
    for scene in document.scenes() {
        for node in scene.nodes() {
            visit(node, "", &mut paths);
        }
    }
    for node in document.nodes() {
        paths.entry(node.index()).or_insert_with(|| {
            node.name()
                .map(str::to_string)
                .unwrap_or_else(|| format!("node_{}", node.index()))
        });
    }
    let mut used = BTreeSet::<String>::new();
    for (node_index, path) in &mut paths {
        let key = path.to_ascii_lowercase();
        if !used.insert(key) {
            *path = format!("{path}#node_{node_index}");
            used.insert(path.to_ascii_lowercase());
        }
    }
    paths
}

pub(super) fn parse_obj_vertex(value: &str) -> Result<(usize, usize, usize), String> {
    let parts = value.split('/').collect::<Vec<_>>();
    if parts.len() < 3 {
        return Err(format!("OBJ face vertex '{value}' must be v/vt/vn"));
    }
    let vertex = parse_index(parts[0], "vertex")?;
    let uv = parse_index(parts[1], "uv")?;
    let normal = parse_index(parts[2], "normal")?;
    Ok((vertex, uv, normal))
}
