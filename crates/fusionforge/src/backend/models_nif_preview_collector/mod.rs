use super::super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ModelSource {
    pub(in super::super) kind: String,
    pub(in super::super) bundle: Option<String>,
    pub(in super::super) path_id: Option<i64>,
    pub(in super::super) uri: Option<String>,
    pub(in super::super) notes: Option<String>,
}

pub(in super::super) fn nif_bytes_to_preview_meshes(
    env: &fusionforge::UnityEnvironment,
    bytes: &[u8],
    name: &str,
    source_asset: &str,
) -> Result<Vec<JsonValue>, String> {
    let mut cursor = std::io::Cursor::new(bytes);
    let nif = nif::Nif::parse(&mut cursor).map_err(|err| format!("NIF parse failed: {err}"))?;
    let mut collector = NifPreviewCollector {
        env,
        source_asset,
        root_name: name,
        meshes: Vec::new(),
    };
    if let Some(root) = nif.blocks.first() {
        match root {
            nif::blocks::Block::NiNode(node) => {
                collector.visit_ni_node(&nif, node, None, name.to_string(), 0.0);
            }
            nif::blocks::Block::NiTriShape(shape) => {
                collector.visit_ni_tri_shape(
                    &nif,
                    shape,
                    nif::glam::Mat4::IDENTITY,
                    name.to_string(),
                );
            }
            _ => {}
        }
    }
    if collector.meshes.is_empty() {
        return Err("NIF contained no previewable triangles".to_string());
    }
    Ok(collector.meshes)
}

pub(in super::super) struct NifPreviewCollector<'a> {
    pub(in super::super) env: &'a fusionforge::UnityEnvironment,
    pub(in super::super) source_asset: &'a str,
    pub(in super::super) root_name: &'a str,
    pub(in super::super) meshes: Vec<JsonValue>,
}

impl NifPreviewCollector<'_> {
    pub(in super::super) fn visit_ni_node(
        &mut self,
        nif: &nif::Nif,
        node: &nif::blocks::NiNode,
        parent_transform: Option<nif::glam::Mat4>,
        label: String,
        lod_distance: f32,
    ) {
        let mut transform = node.transform();
        if let Some(parent) = parent_transform {
            transform = parent * transform;
        }

        for child_ref in node.child_refs.iter().filter(|child| child.0 >= 0) {
            let Some(child) = child_ref.get(&nif.blocks) else {
                continue;
            };
            match child {
                nif::blocks::Block::NiNode(child_node) => {
                    self.visit_ni_node(
                        nif,
                        child_node,
                        Some(transform),
                        format!("{label}_NiNode{}", child_ref.0),
                        lod_distance,
                    );
                }
                nif::blocks::Block::NiLODNode(lod_node) => {
                    self.visit_ni_lod_node(
                        nif,
                        lod_node,
                        transform,
                        format!("{label}_NiLODNode{}", child_ref.0),
                        lod_distance,
                    );
                }
                nif::blocks::Block::NiTriShape(shape) => {
                    self.visit_ni_tri_shape(
                        nif,
                        shape,
                        transform,
                        format!("{label}_NiTriShape{}", child_ref.0),
                    );
                }
                _ => {}
            }
        }
    }

    pub(in super::super) fn visit_ni_lod_node(
        &mut self,
        nif: &nif::Nif,
        lod_node: &nif::blocks::NiLODNode,
        parent_transform: nif::glam::Mat4,
        label: String,
        lod_distance: f32,
    ) {
        let transform = parent_transform * lod_node.transform();
        if let Some(nif::blocks::Block::NiRangeLODData(range_data)) =
            lod_node.lod_level_data_ref.get(&nif.blocks)
        {
            let Some((range_index, _)) = range_data
                .lod_levels
                .iter()
                .enumerate()
                .find(|(_, range)| lod_distance >= range.near && lod_distance < range.far)
                .or_else(|| range_data.lod_levels.first().map(|range| (0, range)))
            else {
                return;
            };
            let Some(child_ref) = lod_node.child_refs.get(range_index) else {
                return;
            };
            let Some(child) = child_ref.get(&nif.blocks) else {
                return;
            };
            match child {
                nif::blocks::Block::NiNode(node) => {
                    self.visit_ni_node(nif, node, Some(transform), label, lod_distance);
                }
                nif::blocks::Block::NiTriShape(shape) => {
                    self.visit_ni_tri_shape(nif, shape, transform, label);
                }
                _ => {}
            }
        } else {
            self.visit_ni_node(
                nif,
                &lod_node.base.base,
                Some(parent_transform),
                label,
                lod_distance,
            );
        }
    }

    pub(in super::super) fn visit_ni_tri_shape(
        &mut self,
        nif: &nif::Nif,
        shape: &nif::blocks::NiTriShape,
        parent_transform: nif::glam::Mat4,
        label: String,
    ) {
        let transform = parent_transform * shape.transform();
        let Some(nif::blocks::Block::NiTriShapeData(data)) = shape.data_ref.get(&nif.blocks) else {
            return;
        };
        let Some(mesh) = self.preview_mesh_from_tri_shape_data(nif, shape, data, transform, &label)
        else {
            return;
        };
        self.meshes.push(mesh);
    }

    pub(in super::super) fn preview_mesh_from_tri_shape_data(
        &self,
        nif: &nif::Nif,
        shape: &nif::blocks::NiTriShape,
        data: &nif::blocks::NiTriShapeData,
        transform: nif::glam::Mat4,
        label: &str,
    ) -> Option<JsonValue> {
        let geometry = &data.base.base;
        if !geometry.has_vertices || !data.has_triangles {
            return None;
        }
        let vertices = geometry.vertices.as_ref()?;
        let triangles = data.triangles.as_ref()?;
        if vertices.is_empty() || triangles.is_empty() {
            return None;
        }

        let positions = vertices
            .iter()
            .map::<nif::glam::Vec3, _>(|vertex| vertex.into())
            .map(|vertex| transform.transform_point3(vertex))
            .flat_map(|vertex| {
                [
                    f64::from(vertex.x),
                    f64::from(vertex.y),
                    f64::from(vertex.z),
                ]
            })
            .collect::<Vec<_>>();
        let normal_transform = transform.inverse().transpose();
        let normals = geometry
            .normals
            .as_ref()
            .map(|normals| {
                normals
                    .iter()
                    .map::<nif::glam::Vec3, _>(|normal| normal.into())
                    .map(|normal| normal_transform.transform_vector3(normal))
                    .flat_map(|normal| {
                        [
                            f64::from(normal.x),
                            f64::from(normal.y),
                            f64::from(normal.z),
                        ]
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let uvs = geometry
            .uv_sets
            .first()
            .map(|uv_set| {
                uv_set
                    .uvs
                    .iter()
                    .flat_map(|uv| [f64::from(uv.u), f64::from(1.0 - uv.v)])
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let indices = triangles
            .iter()
            .flat_map(|triangle| {
                [
                    u32::from(triangle.a),
                    u32::from(triangle.b),
                    u32::from(triangle.c),
                ]
            })
            .collect::<Vec<_>>();
        let material = self.nif_shape_material(nif, shape, label);
        let material_id = material
            .get("id")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string();

        Some(json!({
            "id": format!("nif:{}:{label}", self.source_asset),
            "name": label,
            "kind": "mesh",
            "sourceAsset": self.source_asset,
            "pathId": 0,
            "positions": positions,
            "normals": if normals.len() == positions.len() { json!(normals) } else { json!([]) },
            "uvs": if uvs.len() == positions.len() / 3 * 2 { json!(uvs) } else { json!([]) },
            "indices": indices,
            "position": { "x": 0.0, "y": 0.0, "z": 0.0 },
            "rotation": { "x": 0.0, "y": 0.0, "z": 0.0 },
            "scale": { "x": 1.0, "y": 1.0, "z": 1.0 },
            "materialId": material_id,
            "materialIds": [material_id],
            "groups": [{ "start": 0, "count": triangles.len() * 3, "materialIndex": 0 }],
            "material": material,
        }))
    }

    pub(in super::super) fn nif_shape_material(
        &self,
        nif: &nif::Nif,
        shape: &nif::blocks::NiTriShape,
        label: &str,
    ) -> JsonValue {
        let mut material_name = label.to_string();
        let mut color = "#ffffff".to_string();
        let mut alpha = 1.0_f32;
        let mut texture_path = None::<String>;
        let mut texture_slot = None::<String>;
        let mut texture_candidates = Vec::<JsonValue>::new();
        let mut texture_data_url = JsonValue::Null;
        let mut texture_width = JsonValue::Null;
        let mut texture_height = JsonValue::Null;
        let mut texture_has_alpha = false;
        let mut texture_has_partial_alpha = false;
        let mut has_alpha_property = false;
        let mut texturing_properties = Vec::new();

        for property_ref in &shape.property_refs {
            let Some(property) = property_ref.get(&nif.blocks) else {
                continue;
            };
            match property {
                nif::blocks::Block::NiMaterialProperty(material) => {
                    if !material.name.value.trim().is_empty() {
                        material_name = material.name.value.clone();
                    }
                    color = nif_color3_to_hex(material.color_diffuse);
                    alpha = material.alpha.clamp(0.0, 1.0);
                }
                nif::blocks::Block::NiTexturingProperty(texturing) => {
                    texturing_properties.push(texturing);
                }
                nif::blocks::Block::NiAlphaProperty(_) => {
                    has_alpha_property = true;
                }
                _ => {}
            }
        }

        for texturing in texturing_properties {
            for candidate in nif_texturing_property_texture_paths(nif, texturing) {
                let apply_black_key_alpha = has_alpha_property
                    || matches!(candidate.slot.as_str(), "glow" | "decal0" | "shader");
                let preview = texture_preview_for_nif_texture(
                    self.env,
                    &candidate.path,
                    apply_black_key_alpha,
                );
                texture_candidates.push(json!({
                    "slot": candidate.slot,
                    "path": candidate.path,
                    "found": preview.is_some(),
                }));
                if texture_data_url.is_null() {
                    texture_path = Some(candidate.path.clone());
                    texture_slot = Some(candidate.slot.clone());
                }
                if texture_data_url.is_null() {
                    if let Some(preview) = preview {
                        if let Some(data_url) = preview.get("dataUrl").cloned() {
                            texture_data_url = data_url;
                        }
                        texture_width = preview.get("width").cloned().unwrap_or(JsonValue::Null);
                        texture_height = preview.get("height").cloned().unwrap_or(JsonValue::Null);
                        texture_has_alpha = preview
                            .get("hasAlpha")
                            .and_then(JsonValue::as_bool)
                            .unwrap_or(false);
                        texture_has_partial_alpha = preview
                            .get("hasPartialAlpha")
                            .and_then(JsonValue::as_bool)
                            .unwrap_or(false);
                        texture_path = Some(candidate.path.clone());
                        texture_slot = Some(candidate.slot.clone());
                    }
                }
            }
        }

        let id = format!(
            "nif:{}:{}:{}",
            self.source_asset,
            self.root_name,
            normalized_asset_path(label)
        );
        json!({
            "id": id,
            "name": material_name,
            "color": color,
            "diffuseColor": color,
            "colorAlpha": alpha,
            "shaderName": "NIF/Gamebryo",
            "alphaMode": if has_alpha_property {
                "cutout"
            } else if alpha < 0.995 {
                "blend"
            } else {
                "opaque"
            },
            "texturePath": texture_path,
            "textureSlot": texture_slot,
            "textureCandidates": texture_candidates,
            "textureDataUrl": texture_data_url,
            "textureWidth": texture_width,
            "textureHeight": texture_height,
            "hasAlpha": texture_has_alpha || has_alpha_property || alpha < 0.995,
            "hasPartialAlpha": texture_has_partial_alpha || (alpha > 0.0 && alpha < 0.995),
            "textureTint": false,
        })
    }
}

pub(in super::super) fn nif_color3_to_hex(color: nif::common::Color3) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (color.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.b.clamp(0.0, 1.0) * 255.0).round() as u8
    )
}

pub(in super::super) fn nif_tex_desc_path(nif: &nif::Nif, tex_desc: Option<&nif::blocks::TexDesc>) -> Option<String> {
    let source = tex_desc?.source_ref.get(&nif.blocks)?;
    let nif::blocks::Block::NiSourceTexture(source) = source else {
        return None;
    };
    if source.use_external == 0 {
        return None;
    }
    Some(normalized_nif_texture_name(&source.file_name.value))
}

pub(in super::super) fn nif_image_has_alpha(image: &image::RgbaImage) -> bool {
    image.pixels().any(|pixel| pixel.0[3] < 255)
}

pub(in super::super) fn nif_image_has_partial_alpha(image: &image::RgbaImage) -> bool {
    image.pixels().any(|pixel| {
        let alpha = pixel.0[3];
        alpha > 0 && alpha < 255
    })
}

pub(in super::super) fn nif_has_black_key_coverage(image: &image::RgbaImage) -> bool {
    let total = (image.width() as u64 * image.height() as u64).max(1);
    let mut black = 0_u64;
    let mut lit = 0_u64;
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        if a == 0 {
            continue;
        }
        let max_rgb = r.max(g).max(b);
        if max_rgb <= 8 {
            black += 1;
        } else if max_rgb >= 48 {
            lit += 1;
        }
    }
    black * 100 >= total && lit * 100 >= total
}

pub(in super::super) fn apply_nif_black_key_alpha(image: &mut image::RgbaImage) {
    if !nif_has_black_key_coverage(image) {
        return;
    }
    let use_luminance_alpha = !nif_image_has_alpha(image);
    for pixel in image.pixels_mut() {
        let max_rgb = pixel.0[0].max(pixel.0[1]).max(pixel.0[2]);
        if pixel.0[3] == 0 {
            continue;
        }
        if use_luminance_alpha {
            pixel.0[3] = if max_rgb <= 6 {
                0
            } else if max_rgb >= 220 {
                255
            } else {
                ((max_rgb as u16 * 255 + 90) / 180).min(255) as u8
            };
        } else if max_rgb < 40 {
            pixel.0[3] = if max_rgb <= 6 {
                0
            } else {
                ((pixel.0[3] as u16 * max_rgb as u16 + 20) / 40) as u8
            };
        }
    }
}

pub(in super::super) fn imported_model_to_preview_mesh(
    mesh: &fusionforge::modding::ImportedMesh,
    source_path: &Path,
    joint_paths: &[String],
    materials: &[JsonValue],
    submesh_material_indices: &[Option<usize>],
) -> JsonValue {
    let name = mesh.name.clone().unwrap_or_else(|| {
        source_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("imported_model")
            .to_string()
    });
    let mut positions = Vec::with_capacity(mesh.vertices.len() * 3);
    for (x, y, z) in &mesh.vertices {
        positions.extend([*x, *y, *z]);
    }
    let mut normals = Vec::with_capacity(mesh.normals.len() * 3);
    for (x, y, z) in &mesh.normals {
        normals.extend([*x, *y, *z]);
    }
    let mut uvs = Vec::with_capacity(mesh.uvs.len() * 2);
    for (u, v) in &mesh.uvs {
        uvs.extend([*u, 1.0 - *v]);
    }
    let indices = mesh
        .indices
        .iter()
        .map(|value| json!(u32::from(*value)))
        .collect::<Vec<_>>();
    let bone_indices = mesh
        .skin
        .iter()
        .flat_map(|weight| weight.bone_indices)
        .collect::<Vec<_>>();
    let weights = mesh
        .skin
        .iter()
        .flat_map(|weight| weight.weights)
        .collect::<Vec<_>>();
    let inverse_bind_matrices = mesh
        .bind_poses
        .iter()
        .map(|matrix| matrix.values.to_vec())
        .collect::<Vec<_>>();
    let material_ids = materials
        .iter()
        .filter_map(|material| material.get("id").cloned())
        .collect::<Vec<_>>();
    let material_position = |source_index: usize| {
        materials.iter().position(|material| {
            material
                .get("sourceMaterialIndex")
                .and_then(JsonValue::as_u64)
                == Some(source_index as u64)
        })
    };
    let submeshes = if mesh.submeshes.is_empty() {
        vec![(0_usize, mesh.indices.len())]
    } else {
        mesh.submeshes
            .iter()
            .map(|submesh| (submesh.first_index, submesh.index_count))
            .collect::<Vec<_>>()
    };
    let submesh_material_ids = submeshes
        .iter()
        .enumerate()
        .map(|(index, _)| {
            submesh_material_indices
                .get(index)
                .copied()
                .flatten()
                .and_then(material_position)
                .and_then(|index| material_ids.get(index).cloned())
                .unwrap_or(JsonValue::Null)
        })
        .collect::<Vec<_>>();
    let groups = submeshes
        .iter()
        .enumerate()
        .map(|(index, (start, count))| {
            let material_index = submesh_material_indices
                .get(index)
                .copied()
                .flatten()
                .and_then(material_position)
                .unwrap_or(0);
            json!({ "start": start, "count": count, "materialIndex": material_index })
        })
        .collect::<Vec<_>>();
    let primary_material = submesh_material_ids
        .iter()
        .find_map(JsonValue::as_str)
        .and_then(|id| {
            materials
                .iter()
                .find(|material| material.get("id").and_then(JsonValue::as_str) == Some(id))
        })
        .or_else(|| materials.first());
    let material_summary = primary_material.map(|material| {
        json!({
            "id": material.get("id").cloned().unwrap_or(JsonValue::Null),
            "name": material.get("name").cloned().unwrap_or(JsonValue::Null),
            "color": material.get("color").cloned().unwrap_or(json!("#8b8f76")),
        })
    });
    json!({
        "id": format!("authoring:{}", source_path.to_string_lossy()),
        "name": name,
        "kind": "mesh",
        "sourceAsset": source_path.to_string_lossy(),
        "pathId": 0,
        "positions": positions,
        "normals": if normals.len() == positions.len() { json!(normals) } else { json!([]) },
        "uvs": if uvs.len() == positions.len() / 3 * 2 { json!(uvs) } else { json!([]) },
        "indices": indices,
        "position": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "rotation": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "scale": { "x": 1.0, "y": 1.0, "z": 1.0 },
        "materialId": primary_material.and_then(|material| material.get("id")).cloned().unwrap_or(JsonValue::Null),
        "materialIds": material_ids,
        "submeshMaterialIds": submesh_material_ids,
        "groups": groups,
        "material": material_summary.unwrap_or_else(|| json!({
            "name": "Imported model",
            "color": "#7b9a8b"
        })),
        "skin": if mesh.skin.len() == mesh.vertices.len() && !joint_paths.is_empty() {
            json!({
                "jointPaths": joint_paths,
                "boneIndices": bone_indices,
                "weights": weights,
                "inverseBindMatrices": inverse_bind_matrices,
            })
        } else {
            JsonValue::Null
        },
    })
}

pub(in super::super) fn preview_authoring_model(model_path: String) -> EditorResult<serde_json::Value> {
    run_table_data_task(move || {
        let path = PathBuf::from(&model_path);
        if !path.is_file() {
            return Err(EditorError::MissingPath(model_path).to_string());
        }
        let preview =
            fusionforge::modding::ImportedModelPreview::from_model_path(&path, None, 30.0)?;
        let animations = preview
            .animations
            .iter()
            .into_iter()
            .enumerate()
            .map(|(index, clip)| imported_animation_to_preview(clip, &path, index))
            .collect::<Vec<_>>();
        let skeleton = imported_skeleton_to_preview(&preview.skeleton);
        let material_previews = preview
            .materials
            .iter()
            .map(|material| imported_material_to_preview(material, &path))
            .collect::<Vec<_>>();
        let mesh = imported_model_to_preview_mesh(
            &preview.mesh,
            &path,
            &preview.skeleton.skin_joint_paths,
            &material_previews,
            &preview.submesh_material_indices,
        );
        let materials = material_previews
            .into_iter()
            .filter_map(|material| {
                let id = material.get("id")?.as_str()?.to_string();
                Some((id, material))
            })
            .collect::<serde_json::Map<_, _>>();
        Ok(json!({
            "bundlePath": model_path,
            "cacheDir": null,
            "containerPaths": [],
            "matchedPaths": [],
            "status": "ready",
            "meshes": [mesh],
            "skeleton": skeleton,
            "materials": materials,
            "kfm": [],
            "animations": animations,
            "warnings": preview.warnings,
        }))
    })
}

pub(in super::super) fn apply_exact_preload_materials_to_nif_previews(
    mut previews: Vec<JsonValue>,
    materials: &[JsonValue],
) -> Vec<JsonValue> {
    if materials.is_empty() {
        return previews;
    }

    let material_by_name = materials
        .iter()
        .filter_map(|material| Some((material_exact_name_key(material)?, material.clone())))
        .collect::<BTreeMap<_, _>>();
    if material_by_name.is_empty() {
        return previews;
    }

    for mesh in &mut previews {
        let Some(mesh_object) = mesh.as_object_mut() else {
            continue;
        };
        let Some(existing_key) = mesh_object
            .get("material")
            .and_then(material_exact_name_key)
        else {
            continue;
        };
        let Some(material) = material_by_name.get(&existing_key).cloned() else {
            continue;
        };
        let Some(material_id) = material
            .get("id")
            .and_then(JsonValue::as_str)
            .map(ToOwned::to_owned)
        else {
            continue;
        };
        mesh_object.insert("material".to_string(), material);
        mesh_object.insert("materialId".to_string(), json!(material_id));
        mesh_object.insert("materialIds".to_string(), json!([material_id]));
    }

    previews
}

pub(in super::super) fn kfm_preload_nif_previews(
    env: &fusionforge::UnityEnvironment,
    preload_table: &[fusionforge::UnityValue],
    start: usize,
    end: usize,
    kfm_path: &str,
    source_asset: &str,
    kfm_references: &[String],
    warnings: &mut Vec<String>,
) -> Vec<JsonValue> {
    let reference_stems = kfm_references
        .iter()
        .filter(|path| path.to_ascii_lowercase().ends_with(".nif"))
        .filter_map(|path| path_stem_token(path))
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::<(usize, i64)>::new();
    let mut previews = Vec::new();
    let preload_materials = preload_material_previews(env, preload_table, start, end);

    for preload in &preload_table[start..end] {
        let Some(pointer) = preload.as_pointer() else {
            continue;
        };
        let Ok(key) = env.resolve_pointer(pointer) else {
            continue;
        };
        if !seen.insert((key.asset, key.path_id)) {
            continue;
        }
        let Some(asset) = env.assets.get(key.asset) else {
            continue;
        };
        let Some(info) = asset.objects.get(&key.path_id) else {
            continue;
        };
        let Ok(body) = asset.read_object(key.asset, info) else {
            continue;
        };
        let object_name = fusionforge::object_name(&body);
        let lower_name = object_name.to_ascii_lowercase();
        let likely_nif = lower_name.ends_with(".nif")
            || reference_stems.iter().any(|stem| lower_name.contains(stem));
        let mut visited = BTreeSet::new();
        let Some(bytes) = container_asset_bytes(env, key.asset, key.path_id, &mut visited) else {
            continue;
        };
        let preview_name = if object_name.trim().is_empty() {
            format!("{kfm_path} preload #{}", key.path_id)
        } else {
            object_name
        };
        match nif_bytes_to_preview_meshes(env, &bytes, &preview_name, source_asset) {
            Ok(meshes) => previews.extend(apply_exact_preload_materials_to_nif_previews(
                meshes,
                &preload_materials,
            )),
            Err(err) if likely_nif => warnings.push(format!("{preview_name}: {err}")),
            Err(_) => {}
        }
        if previews.len() >= 8 {
            break;
        }
    }

    previews
}

pub(in super::super) fn selected_contains_mesh(
    env: &fusionforge::UnityEnvironment,
    selected: &BTreeSet<(usize, i64)>,
) -> bool {
    selected.iter().any(|(asset_index, path_id)| {
        env.assets
            .get(*asset_index)
            .and_then(|asset| {
                asset
                    .objects
                    .get(path_id)
                    .map(|info| asset.object_type_name(info) == "Mesh")
            })
            .unwrap_or(false)
    })
}
