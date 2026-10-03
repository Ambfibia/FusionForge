use super::*;

#[derive(Debug, Clone)]
pub struct ImportedMesh {
    pub name: Option<String>,
    pub vertices: Vec<(f64, f64, f64)>,
    pub normals: Vec<(f64, f64, f64)>,
    pub uvs: Vec<(f64, f64)>,
    pub indices: Vec<u16>,
    pub submeshes: Vec<ImportedSubMesh>,
    pub skin: Vec<ImportedBoneWeight>,
    pub bind_poses: Vec<ImportedMatrix4x4>,
    pub joint_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ImportedSubMesh {
    pub first_index: usize,
    pub index_count: usize,
}

/// Authoring-model data prepared with a single GLTF import pass. Keeping mesh,
/// skeleton and animation data together avoids loading and decoding the same
/// GLB three times when the native preview opens.
#[derive(Debug, Clone)]
pub struct ImportedModelPreview {
    pub mesh: ImportedMesh,
    pub skeleton: ImportedSkeleton,
    pub animations: Vec<ImportedAnimationClip>,
    pub materials: Vec<ImportedMaterial>,
    /// Material index for every imported mesh primitive/submesh. Values refer
    /// to [`ImportedMaterial::index`] and preserve the GLTF primitive binding.
    pub submesh_material_indices: Vec<Option<usize>>,
    pub warnings: Vec<String>,
}

pub(super) fn imported_gltf_materials(
    document: &gltf::Document,
    images: &[gltf::image::Data],
) -> Result<Vec<ImportedMaterial>, String> {
    document
        .materials()
        .enumerate()
        .map(|(fallback_index, material)| {
            let index = material.index().unwrap_or(fallback_index);
            let pbr = material.pbr_metallic_roughness();
            let base_color = pbr.base_color_factor().map(f64::from);
            let emissive = material.emissive_factor().map(f64::from);
            let base_color_texture = pbr
                .base_color_texture()
                .map(|texture_info| {
                    let texture = texture_info.texture();
                    let sampler = texture.sampler();
                    let clamp =
                        matches!(sampler.wrap_s(), gltf::texture::WrappingMode::ClampToEdge)
                            || matches!(sampler.wrap_t(), gltf::texture::WrappingMode::ClampToEdge);
                    let image_index = texture.source().index();
                    let image = images.get(image_index).ok_or_else(|| {
                        format!(
                            "GLTF material {} references missing image {}",
                            index, image_index
                        )
                    })?;
                    ImportedTexture::from_gltf_image(
                        image,
                        Some(format!(
                            "{}_base_color",
                            material.name().unwrap_or("material")
                        )),
                        clamp,
                    )
                })
                .transpose()?;
            let alpha_mode = match material.alpha_mode() {
                gltf::material::AlphaMode::Opaque => ImportedMaterialAlphaMode::Opaque,
                gltf::material::AlphaMode::Mask => ImportedMaterialAlphaMode::Mask,
                gltf::material::AlphaMode::Blend => ImportedMaterialAlphaMode::Blend,
            };
            Ok(ImportedMaterial {
                index,
                name: material
                    .name()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("material_{index}")),
                base_color,
                emissive,
                metallic: f64::from(pbr.metallic_factor()),
                roughness: f64::from(pbr.roughness_factor()),
                alpha_mode,
                alpha_cutoff: material.alpha_cutoff().map(f64::from),
                double_sided: material.double_sided(),
                base_color_texture,
            })
        })
        .collect()
}

pub(super) fn gltf_vec3_to_fusionfall(x: f32, y: f32, z: f32) -> (f64, f64, f64) {
    (-f64::from(x), f64::from(z), f64::from(y))
}

pub(super) fn gltf_direction_to_fusionfall(x: f32, y: f32, z: f32) -> (f64, f64, f64) {
    (-f64::from(x), f64::from(z), f64::from(y))
}

pub(super) fn gltf_scale_to_fusionfall(x: f32, y: f32, z: f32) -> (f64, f64, f64) {
    (f64::from(x), f64::from(z), f64::from(y))
}

pub(super) fn gltf_quat_to_fusionfall(x: f32, y: f32, z: f32, w: f32) -> (f64, f64, f64, f64) {
    (-f64::from(x), f64::from(z), f64::from(y), f64::from(w))
}

impl ImportedMesh {
    pub fn from_model_path(path: &Path, name: Option<String>) -> Result<Self, String> {
        match path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "obj" => Self::from_obj_path(path, name),
            "glb" | "gltf" => Self::from_gltf_path(path, name),
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

    pub fn from_obj_path(path: &Path, name: Option<String>) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
        Self::from_obj_str(&text, name)
    }

    pub fn from_gltf_path(path: &Path, name: Option<String>) -> Result<Self, String> {
        let (document, buffers, _) =
            gltf::import(path).map_err(|err| format!("{}: {err}", path.display()))?;
        Self::from_gltf_document(path, name, &document, &buffers)
    }

    pub(super) fn from_gltf_document(
        path: &Path,
        name: Option<String>,
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> Result<Self, String> {
        let mut vertices = Vec::<(f64, f64, f64)>::new();
        let mut normals = Vec::<(f64, f64, f64)>::new();
        let mut uvs = Vec::<(f64, f64)>::new();
        let mut indices = Vec::<u16>::new();
        let mut submeshes = Vec::<ImportedSubMesh>::new();
        let mut skin = Vec::<ImportedBoneWeight>::new();
        let mut resolved_name = name;
        let mut saw_missing_normals = false;
        let first_skin = document.skins().next();
        let joint_names = first_skin
            .as_ref()
            .map(|skin| {
                skin.joints()
                    .map(|joint| joint.name().unwrap_or_default().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for gltf_mesh in document.meshes() {
            if resolved_name.is_none() {
                resolved_name = gltf_mesh.name().map(str::to_string);
            }
            for primitive in gltf_mesh.primitives() {
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    return Err(format!(
                        "{}: GLTF primitive mode {:?} is not supported; triangulate the model first",
                        path.display(),
                        primitive.mode()
                    ));
                }
                let reader = primitive
                    .reader(|buffer| buffers.get(buffer.index()).map(|data| data.0.as_slice()));
                let positions = reader
                    .read_positions()
                    .ok_or_else(|| {
                        format!(
                            "{}: GLTF mesh primitive has no POSITION attribute",
                            path.display()
                        )
                    })?
                    .collect::<Vec<_>>();
                if positions.is_empty() {
                    continue;
                }
                let base = u32::try_from(vertices.len())
                    .map_err(|_| "GLTF has too many vertices".to_string())?;
                for [x, y, z] in &positions {
                    vertices.push(gltf_vec3_to_fusionfall(*x, *y, *z));
                }

                let primitive_normals = reader.read_normals().map(|values| {
                    values
                        .map(|[x, y, z]| gltf_direction_to_fusionfall(x, y, z))
                        .collect::<Vec<_>>()
                });
                if let Some(primitive_normals) =
                    primitive_normals.filter(|values| values.len() == positions.len())
                {
                    normals.extend(primitive_normals);
                } else {
                    saw_missing_normals = true;
                    normals.extend(std::iter::repeat((0.0, 0.0, 0.0)).take(positions.len()));
                }

                let primitive_uvs = reader
                    .read_tex_coords(0)
                    .map(|coords| {
                        coords
                            .into_f32()
                            .map(|[u, v]| (f64::from(u), 1.0 - f64::from(v)))
                            .collect::<Vec<_>>()
                    })
                    .filter(|values| values.len() == positions.len());
                if let Some(primitive_uvs) = primitive_uvs {
                    uvs.extend(primitive_uvs);
                } else {
                    uvs.extend(std::iter::repeat((0.0, 0.0)).take(positions.len()));
                }

                let primitive_joints = reader
                    .read_joints(0)
                    .map(|joints| joints.into_u16().collect::<Vec<_>>())
                    .filter(|values| values.len() == positions.len());
                let primitive_weights = reader
                    .read_weights(0)
                    .map(|weights| weights.into_f32().collect::<Vec<_>>())
                    .filter(|values| values.len() == positions.len());
                if let (Some(primitive_joints), Some(primitive_weights)) =
                    (primitive_joints, primitive_weights)
                {
                    skin.extend(primitive_joints.into_iter().zip(primitive_weights).map(
                        |([j0, j1, j2, j3], [w0, w1, w2, w3])| ImportedBoneWeight {
                            weights: [w0 as f64, w1 as f64, w2 as f64, w3 as f64],
                            bone_indices: [j0 as i64, j1 as i64, j2 as i64, j3 as i64],
                        },
                    ));
                } else {
                    skin.extend((0..positions.len()).map(|_| ImportedBoneWeight {
                        weights: [1.0, 0.0, 0.0, 0.0],
                        bone_indices: [0, 0, 0, 0],
                    }));
                }

                let primitive_indices = reader
                    .read_indices()
                    .map(|values| values.into_u32().collect::<Vec<_>>())
                    .unwrap_or_else(|| (0..positions.len() as u32).collect::<Vec<_>>());
                if primitive_indices.len() % 3 != 0 {
                    return Err(format!(
                        "{}: GLTF triangle index count is not divisible by 3",
                        path.display()
                    ));
                }
                for triangle in primitive_indices.chunks_exact(3) {
                    for source_index in [triangle[0], triangle[1], triangle[2]] {
                        let index = base
                            .checked_add(source_index)
                            .ok_or_else(|| "GLTF index overflow".to_string())?;
                        if usize::try_from(index)
                            .ok()
                            .is_none_or(|index| index >= vertices.len())
                        {
                            return Err(format!(
                                "{}: GLTF index {} is outside the vertex buffer",
                                path.display(),
                                source_index
                            ));
                        }
                        indices.push(
                            u16::try_from(index)
                                .map_err(|_| "GLTF has more than 65535 vertices".to_string())?,
                        );
                    }
                }
                let first_index = indices.len().saturating_sub(primitive_indices.len());
                if !primitive_indices.is_empty() {
                    submeshes.push(ImportedSubMesh {
                        first_index,
                        index_count: primitive_indices.len(),
                    });
                }
            }
        }

        if vertices.is_empty() || indices.is_empty() {
            return Err(format!(
                "{}: GLTF contained no mesh triangles",
                path.display()
            ));
        }
        if saw_missing_normals {
            normals = generated_normals(&vertices, &indices);
        }
        dedupe_imported_vertex_streams(
            &mut vertices,
            &mut normals,
            &mut uvs,
            &mut skin,
            &mut indices,
        );
        align_triangle_winding_with_normals(&vertices, &normals, &mut indices);
        let bind_poses = first_skin
            .and_then(|skin| {
                skin.reader(|buffer| buffers.get(buffer.index()).map(|data| data.0.as_slice()))
                    .read_inverse_bind_matrices()
                    .map(|matrices| {
                        matrices
                            .map(gltf_bind_pose_to_fusionfall)
                            .collect::<Vec<_>>()
                    })
            })
            .unwrap_or_default();

        Ok(Self {
            name: resolved_name,
            vertices,
            normals,
            uvs,
            indices,
            submeshes,
            skin,
            bind_poses,
            joint_names,
        })
    }

    pub fn from_obj_str(text: &str, name: Option<String>) -> Result<Self, String> {
        let mut source_vertices = Vec::<(f64, f64, f64)>::new();
        let mut source_normals = Vec::<(f64, f64, f64)>::new();
        let mut source_uvs = Vec::<(f64, f64)>::new();
        let mut vertices = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();
        let mut dedupe = BTreeMap::<(usize, usize, usize), u16>::new();

        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts = line.split_whitespace().collect::<Vec<_>>();
            match parts.first().copied() {
                Some("v") if parts.len() >= 4 => {
                    let x = parse_f64(parts[1], "vertex x")?;
                    let y = parse_f64(parts[2], "vertex y")?;
                    let z = parse_f64(parts[3], "vertex z")?;
                    source_vertices.push((-x, z, y));
                }
                Some("vn") if parts.len() >= 4 => {
                    let x = parse_f64(parts[1], "normal x")?;
                    let y = parse_f64(parts[2], "normal y")?;
                    let z = parse_f64(parts[3], "normal z")?;
                    source_normals.push((-x, z, y));
                }
                Some("vt") if parts.len() >= 3 => {
                    source_uvs.push((
                        parse_f64(parts[1], "uv x")?,
                        1.0 - parse_f64(parts[2], "uv y")?,
                    ));
                }
                Some("f") => {
                    if parts.len() != 4 {
                        return Err("OBJ import expects triangulated faces".to_string());
                    }
                    let mut face = Vec::with_capacity(3);
                    for vertex in &parts[1..] {
                        let key = parse_obj_vertex(vertex)?;
                        if key.0 >= source_vertices.len() {
                            return Err(format!("OBJ vertex index {} out of range", key.0 + 1));
                        }
                        if key.1 >= source_uvs.len() {
                            return Err(format!("OBJ uv index {} out of range", key.1 + 1));
                        }
                        if key.2 >= source_normals.len() {
                            return Err(format!("OBJ normal index {} out of range", key.2 + 1));
                        }
                        let index = if let Some(index) = dedupe.get(&key) {
                            *index
                        } else {
                            let index = u16::try_from(vertices.len()).map_err(|_| {
                                "OBJ has more than 65535 unique vertices".to_string()
                            })?;
                            dedupe.insert(key, index);
                            vertices.push(source_vertices[key.0]);
                            uvs.push(source_uvs[key.1]);
                            normals.push(source_normals[key.2]);
                            index
                        };
                        face.push(index);
                    }
                    indices.extend(face);
                }
                _ => {}
            }
        }

        Ok(Self {
            name,
            vertices,
            normals,
            uvs,
            indices,
            submeshes: Vec::new(),
            skin: Vec::new(),
            bind_poses: Vec::new(),
            joint_names: Vec::new(),
        })
    }

    pub fn index_buffer(&self) -> Vec<u8> {
        self.indices
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    pub fn remap_skin_to_bones(&mut self, target_bones: &[String]) -> Vec<String> {
        if self.skin.is_empty() || self.joint_names.is_empty() || target_bones.is_empty() {
            return Vec::new();
        }
        let mut target_by_name = BTreeMap::<String, i64>::new();
        for (index, name) in target_bones.iter().enumerate() {
            target_by_name.insert(normalized_bone_name(name), index as i64);
        }

        let mut source_to_target = Vec::with_capacity(self.joint_names.len());
        let mut source_to_target_exact = Vec::with_capacity(self.joint_names.len());
        let mut warnings = Vec::new();
        for name in &self.joint_names {
            let normalized = normalized_bone_name(name);
            let fallback = fallback_bone_name(&normalized);
            let exact_target = target_by_name.get(&normalized).copied();
            let fallback_target = fallback.and_then(|name| target_by_name.get(&name).copied());
            let target = exact_target.or(fallback_target);
            if target.is_none() {
                warnings.push(format!("missing target bone for GLB joint '{name}'"));
            }
            source_to_target.push(target);
            source_to_target_exact.push(exact_target.is_some());
        }

        let mut used_source = vec![false; self.joint_names.len()];
        for skin in &mut self.skin {
            for i in 0..4 {
                let source = skin.bone_indices[i].max(0) as usize;
                if skin.weights[i] <= 0.0 {
                    skin.bone_indices[i] = 0;
                    continue;
                }
                if let Some(Some(target)) = source_to_target.get(source) {
                    skin.bone_indices[i] = *target;
                    if let Some(used) = used_source.get_mut(source) {
                        *used = true;
                    }
                } else {
                    skin.bone_indices[i] = 0;
                }
            }
        }

        let mut remapped_bind_poses = vec![identity_matrix4x4(); target_bones.len()];
        let mut remapped_bind_pose_exact = vec![false; target_bones.len()];
        for (source_index, maybe_target) in source_to_target.iter().enumerate() {
            let Some(target_index) = maybe_target else {
                continue;
            };
            let Some(bind_pose) = self.bind_poses.get(source_index) else {
                continue;
            };
            let target_index = *target_index as usize;
            let source_is_exact = source_to_target_exact
                .get(source_index)
                .copied()
                .unwrap_or(false);
            if !source_is_exact
                && remapped_bind_pose_exact
                    .get(target_index)
                    .copied()
                    .unwrap_or(false)
            {
                continue;
            }
            if let Some(slot) = remapped_bind_poses.get_mut(target_index) {
                *slot = bind_pose.clone();
            }
            if source_is_exact {
                if let Some(slot) = remapped_bind_pose_exact.get_mut(target_index) {
                    *slot = true;
                }
            }
        }
        self.bind_poses = remapped_bind_poses;

        warnings
            .into_iter()
            .filter(|warning| {
                let Some(name) = warning
                    .strip_prefix("missing target bone for GLB joint '")
                    .and_then(|value| value.strip_suffix('\''))
                else {
                    return true;
                };
                self.joint_names
                    .iter()
                    .position(|joint| joint == name)
                    .and_then(|index| used_source.get(index))
                    .copied()
                    .unwrap_or(false)
            })
            .collect()
    }

    pub fn used_joint_indices(&self) -> BTreeSet<usize> {
        let mut used = BTreeSet::new();
        for skin in &self.skin {
            for index in 0..4 {
                if skin.weights[index] > 0.0 {
                    used.insert(skin.bone_indices[index].max(0) as usize);
                }
            }
        }
        used
    }

    pub fn set_identity_bind_poses(&mut self, count: usize) {
        self.bind_poses = vec![identity_matrix4x4(); count];
    }
}

pub(super) fn dedupe_imported_vertex_streams(
    vertices: &mut Vec<(f64, f64, f64)>,
    normals: &mut Vec<(f64, f64, f64)>,
    uvs: &mut Vec<(f64, f64)>,
    skin: &mut Vec<ImportedBoneWeight>,
    indices: &mut Vec<u16>,
) {
    if vertices.is_empty()
        || vertices.len() != normals.len()
        || vertices.len() != uvs.len()
        || vertices.len() != skin.len()
    {
        return;
    }

    let mut remap = vec![0u16; vertices.len()];
    let mut dedupe = BTreeMap::<[u64; 16], u16>::new();
    let mut compact_vertices = Vec::with_capacity(vertices.len());
    let mut compact_normals = Vec::with_capacity(normals.len());
    let mut compact_uvs = Vec::with_capacity(uvs.len());
    let mut compact_skin = Vec::with_capacity(skin.len());

    for i in 0..vertices.len() {
        let vertex = vertices[i];
        let normal = normals[i];
        let uv = uvs[i];
        let weights = skin[i].weights;
        let bone_indices = skin[i].bone_indices;
        let key = [
            vertex.0.to_bits(),
            vertex.1.to_bits(),
            vertex.2.to_bits(),
            normal.0.to_bits(),
            normal.1.to_bits(),
            normal.2.to_bits(),
            uv.0.to_bits(),
            uv.1.to_bits(),
            bone_indices[0] as u64,
            bone_indices[1] as u64,
            bone_indices[2] as u64,
            bone_indices[3] as u64,
            weights[0].to_bits(),
            weights[1].to_bits(),
            weights[2].to_bits(),
            weights[3].to_bits(),
        ];
        let index = if let Some(index) = dedupe.get(&key).copied() {
            index
        } else {
            let index = compact_vertices.len() as u16;
            dedupe.insert(key, index);
            compact_vertices.push(vertex);
            compact_normals.push(normal);
            compact_uvs.push(uv);
            compact_skin.push(skin[i].clone());
            index
        };
        remap[i] = index;
    }

    for index in indices.iter_mut() {
        *index = remap[*index as usize];
    }

    *vertices = compact_vertices;
    *normals = compact_normals;
    *uvs = compact_uvs;
    *skin = compact_skin;
}

#[cfg(test)]
pub(super) fn quantized_vertex_component(value: f64, min: f64, range: f64, bit_size: u32) -> u32 {
    if range <= f64::EPSILON {
        return 0;
    }
    let quantized_max = ((1_u64 << bit_size.min(31)) - 1).max(1) as f64;
    (((value - min) / range) * quantized_max)
        .round()
        .clamp(0.0, quantized_max) as u32
}

#[cfg(test)]
pub(super) fn quantized_mesh_float_ranges(mesh: &ImportedMesh) -> ((f64, f64), (f64, f64), (f64, f64)) {
    let (mut vertex_min, mut vertex_max) = (f64::INFINITY, f64::NEG_INFINITY);
    for (x, y, z) in &mesh.vertices {
        vertex_min = vertex_min.min(*x).min(*y).min(*z);
        vertex_max = vertex_max.max(*x).max(*y).max(*z);
    }
    let (mut uv_min, mut uv_max) = (f64::INFINITY, f64::NEG_INFINITY);
    for (u, v) in &mesh.uvs {
        uv_min = uv_min.min(*u).min(*v);
        uv_max = uv_max.max(*u).max(*v);
    }
    let (mut normal_min, mut normal_max) = (f64::INFINITY, f64::NEG_INFINITY);
    for (x, y, _) in &mesh.normals {
        normal_min = normal_min.min(*x).min(*y);
        normal_max = normal_max.max(*x).max(*y);
    }
    let vertex_range = (vertex_min, (vertex_max - vertex_min).max(0.0));
    let uv_range = (uv_min, (uv_max - uv_min).max(0.0));
    let normal_range = (normal_min, (normal_max - normal_min).max(0.0));
    (vertex_range, uv_range, normal_range)
}

#[cfg(test)]
pub(super) fn quantized_vertex_key(mesh: &ImportedMesh, index: usize) -> [u32; 16] {
    let ((vertex_start, vertex_range), (uv_start, uv_range), (normal_start, normal_range)) =
        quantized_mesh_float_ranges(mesh);
    let vertex = mesh.vertices[index];
    let normal = mesh.normals[index];
    let uv = mesh.uvs[index];
    let influences = quantized_skin_influences(&mesh.skin[index]);
    let mut influence_indices = [0_u32; 4];
    let mut influence_weights = [0_u32; 4];
    for (slot, (bone, weight)) in influences.into_iter().take(4).enumerate() {
        influence_indices[slot] = bone;
        influence_weights[slot] = weight;
    }
    [
        quantized_vertex_component(vertex.0, vertex_start, vertex_range, 20),
        quantized_vertex_component(vertex.1, vertex_start, vertex_range, 20),
        quantized_vertex_component(vertex.2, vertex_start, vertex_range, 20),
        quantized_vertex_component(uv.0, uv_start, uv_range, 16),
        quantized_vertex_component(uv.1, uv_start, uv_range, 16),
        quantized_vertex_component(normal.0, normal_start, normal_range, 8),
        quantized_vertex_component(normal.1, normal_start, normal_range, 8),
        if normal.2 >= 0.0 { 1 } else { 0 },
        influence_indices[0],
        influence_indices[1],
        influence_indices[2],
        influence_indices[3],
        influence_weights[0],
        influence_weights[1],
        influence_weights[2],
        influence_weights[3],
    ]
}

pub fn gltf_has_skins(path: &Path) -> Result<bool, String> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "glb" | "gltf" => {}
        _ => return Ok(false),
    }
    let (document, _, _) =
        gltf::import(path).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(document.skins().next().is_some())
}
