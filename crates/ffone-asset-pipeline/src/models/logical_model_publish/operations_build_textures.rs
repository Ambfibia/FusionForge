use super::*;

/// Coalesce only fully identical immutable native values. Keep every renderer
/// primitive/slot and ordering. Animated material ownership remains separate.
pub(super) fn reuse_identical_static_materials(converted: &mut ConvertedModel) -> Result<()> {
    if converted.model.animations.iter().any(|a| {
        !a.metadata.float_curves.is_empty()
            || !a.metadata.object_curves.is_empty()
            || !a.metadata.events.is_empty()
    }) {
        return Ok(());
    }
    if converted.model.materials.len() != converted.material_reports.len() {
        return invalid("native material/report count mismatch before sharing");
    }
    let old = std::mem::take(&mut converted.model.materials);
    let reports = std::mem::take(&mut converted.material_reports);
    let mut exact_values = Vec::<Vec<u8>>::new();
    let mut remap = Vec::new();
    for (material, report) in old.into_iter().zip(reports) {
        let bytes = serde_json::to_vec(&material).map_err(|e| invalid_error(e.to_string()))?;
        let mutable = material
            .texture_bindings
            .iter()
            .any(|b| b.dynamic_texture.is_some());
        let existing = if mutable {
            None
        } else {
            exact_values.iter().position(|previous| previous == &bytes)
        };
        let index = existing.unwrap_or(converted.model.materials.len());
        if existing.is_none() {
            exact_values.push(bytes);
            converted.model.materials.push(material);
            converted.material_reports.push(report);
        }
        remap.push(u32_index(index, "shared material index")?);
    }
    for mesh in &mut converted.model.meshes {
        for primitive in &mut mesh.primitives {
            if let Some(index) = &mut primitive.material {
                *index = remap[*index as usize];
            }
        }
    }
    Ok(())
}

/// Unity 2.x occasionally serializes a one-bone `SkinnedMeshRenderer` on the
/// animated bone itself while retaining the importer's empty, mesh-named
/// renderer transform beside the skeleton. A glTF skin evaluates relative to
/// its mesh node; placing that mesh on its own joint makes the joint motion
/// self-relative in Bevy and leaves the rendered surface visually static.
/// Reuse the unique non-joint renderer transform when the exact hierarchy
/// provides one, keeping the source skeleton and animation bindings intact.
pub(super) fn skinned_renderer_output_node(
    source: &SourceDocument,
    source_mesh: &SourceMesh,
    binding_node_index: u32,
    skin_joints: &[u32],
    nodes: &[ModelNode],
) -> Result<u32> {
    if !skin_joints.contains(&binding_node_index) {
        return Ok(binding_node_index);
    }

    let candidates = source
        .model_hierarchy
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, source_node)| {
            let index = u32::try_from(index).ok()?;
            (!skin_joints.contains(&index)
                && source_node.name == source_mesh.name
                && nodes[index as usize].mesh.is_none()
                && nodes[index as usize].skin.is_none())
            .then_some(index)
        })
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [index] => Ok(*index),
        [] => Ok(binding_node_index),
        _ => invalid(format!(
            "self-skinned mesh {:?} has more than one non-joint renderer transform",
            source_mesh.name
        )),
    }
}

pub(super) fn legacy_renderer_sort_key(
    queue: i32,
    compositor: u8,
    source_renderer_index: usize,
    mesh_index: usize,
) -> (i32, u8, usize, usize) {
    (queue, compositor, source_renderer_index, mesh_index)
}

/// Recovers the standalone legacy compositing order independently from Unity
/// Mesh object enumeration. ShaderLab render queue is the primary order at
/// runtime; this renderer ordinal supplies the stable order inside one queue.
/// Ordinary surfaces precede the Fusion overlay, while additive surfaces (the
/// red Fusion eyes) are the final same-queue compositor.
pub(super) fn assign_legacy_renderer_orders(
    meshes: &mut [ModelMesh],
    materials: &[NativeMaterial],
    source_renderer_indices: &[usize],
) -> Result<()> {
    if meshes.len() != source_renderer_indices.len() {
        return invalid("legacy renderer identity count differs from native mesh count");
    }
    let mut indices = (0..meshes.len()).collect::<Vec<_>>();
    indices.sort_by_key(|&mesh_index| {
        let mesh = &meshes[mesh_index];
        let mut queue = i32::MAX;
        let mut compositor = 0_u8;
        for primitive in &mesh.primitives {
            let Some(material_index) = primitive.material else {
                queue = queue.min(2_000);
                continue;
            };
            let Some(material) = materials.get(material_index as usize) else {
                continue;
            };
            queue = queue.min(material.render_queue);
            let additive = material.passes.iter().any(|pass| {
                pass.blend.enabled && pass.blend.destination_color == MaterialBlendFactor::One
            });
            compositor = compositor.max(legacy_material_compositor_rank(
                &material.legacy_shader_name,
                additive,
            ));
        }
        if queue == i32::MAX {
            queue = 2_000;
        }
        legacy_renderer_sort_key(
            queue,
            compositor,
            source_renderer_indices[mesh_index],
            mesh_index,
        )
    });
    for (renderer_order, mesh_index) in indices.into_iter().enumerate() {
        meshes[mesh_index].renderer_order = u16::try_from(renderer_order)
            .map_err(|_| invalid_error("legacy renderer count exceeds u16"))?;
    }
    Ok(())
}

pub(super) fn select_source_visuals(source: &SourceDocument) -> SourceVisualSelection {
    let mut selection = SourceVisualSelection::default();
    for (mesh_index, mesh) in source.meshes.iter().enumerate() {
        for (binding_index, binding) in mesh.source_bindings.iter().enumerate() {
            if binding.component_type == "SkinnedMeshRenderer" {
                selection.preserved_skinned_mesh_bindings += 1;
                continue;
            }
            if is_biped_display_helper(mesh, binding) {
                selection
                    .excluded_biped_helper_bindings
                    .insert((mesh_index, binding_index));
                selection.excluded_mesh_parts += mesh.groups.len();
                selection.excluded_source_mesh_ids.insert(mesh.id.clone());
            }
        }
    }
    selection
}

pub(super) fn is_biped_display_helper(mesh: &SourceMesh, binding: &SourceBinding) -> bool {
    binding.component_type == "MeshFilter" && is_biped_display_name(&mesh.name)
}

pub(super) fn is_biped_display_name(name: &str) -> bool {
    if name == "Biped Object" {
        return true;
    }
    name.strip_prefix("Biped Object@#").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

pub(super) fn build_textures(source: &SourceDocument, ordered_ids: &[String]) -> Result<BuiltTextures> {
    let safe_filename = minimal_windows_glb_filename(&source.logical_name)
        .map_err(|error| invalid_error(error.to_string()))?;
    let safe_stem = safe_filename
        .strip_suffix(".glb")
        .ok_or_else(|| invalid_error("native model filename has no .glb suffix"))?;
    let texture_directory = format!("{safe_stem}.textures");
    let mut uri_collisions = BTreeMap::<String, (String, String)>::new();
    let mut textures = Vec::with_capacity(ordered_ids.len());
    let mut samplers = Vec::with_capacity(ordered_ids.len());
    let mut files = Vec::with_capacity(ordered_ids.len());
    let mut reports = Vec::with_capacity(ordered_ids.len());
    let mut indices = BTreeMap::new();
    let mut preferred_name_counts = BTreeMap::<String, usize>::new();
    for texture_id in ordered_ids {
        let preferred = minimal_windows_png_filename(&source.textures[texture_id].name)
            .map_err(|error| invalid_error(error.to_string()))?;
        *preferred_name_counts
            .entry(preferred.to_ascii_lowercase())
            .or_default() += 1;
    }

    for texture_id in ordered_ids {
        let source_texture = &source.textures[texture_id];
        validate_texture_source(texture_id, source_texture)?;
        let preferred_png_name = minimal_windows_png_filename(&source_texture.name)
            .map_err(|error| invalid_error(error.to_string()))?;
        // Distinct Unity Texture2D objects can have true names that collapse
        // to the same Windows-safe PNG name (`name` versus `name.dds` is a
        // real clean-primary example). Preserve both exact objects and their
        // bytes by retaining the legacy extension only for the colliding
        // group. Non-colliding publication paths remain byte-for-byte stable.
        let png_name = if preferred_name_counts
            .get(&preferred_png_name.to_ascii_lowercase())
            .copied()
            .unwrap_or_default()
            > 1
        {
            windows_png_filename_preserving_legacy_extension(&source_texture.name)
                .map_err(|error| invalid_error(error.to_string()))?
        } else {
            preferred_png_name
        };
        let base_uri = format!("{texture_directory}/{png_name}");
        let mip_stem = png_name
            .strip_suffix(".png")
            .ok_or_else(|| invalid_error("minimal texture filename has no .png suffix"))?;
        let mut level_pngs = Vec::with_capacity(source_texture.mip_levels.len());
        let mut level_uris = Vec::with_capacity(source_texture.mip_levels.len());
        for (level_index, level) in source_texture.mip_levels.iter().enumerate() {
            let bytes = decode_exact_png_payload(
                &source_texture.name,
                level.width,
                level.height,
                &level.payload,
            )?;
            let uri = if level_index == 0 {
                base_uri.clone()
            } else {
                format!("{texture_directory}/{mip_stem}.mips/mip-{level_index:02}.png")
            };
            let png_hash = sha256_hex(&bytes);
            let identity = format!("{} mip {}", source_texture.name, level_index);
            if let Some((previous_name, previous_hash)) =
                uri_collisions.get(&uri.to_ascii_lowercase())
            {
                let detail = if previous_hash != &png_hash {
                    " with different PNG bytes"
                } else {
                    " even though the PNG bytes match; a distinct exact level would be lost"
                };
                return invalid(format!(
                    "texture true-name URI collision between {previous_name:?} and {identity:?}{detail}"
                ));
            }
            uri_collisions.insert(uri.to_ascii_lowercase(), (identity, png_hash));
            level_pngs.push(bytes);
            level_uris.push(uri);
        }
        let base_bytes = level_pngs
            .first()
            .ok_or_else(|| invalid_error("validated texture has no base mip PNG"))?;
        let explicit_base = decode_exact_png(source_texture)?;
        if explicit_base != *base_bytes {
            return invalid(format!(
                "texture {:?} base payload differs from mipLevels[0]",
                source_texture.name
            ));
        }

        let sampler_index = u32_index(samplers.len(), "sampler count")?;
        let source_has_mips = source_texture.mip_count > 1;
        let filter_mode = source_texture.sampler.filter_mode.value;
        let wrap_mode = source_texture.sampler.wrap_mode.value;
        let sampler = NativeSampler {
            name: source_texture.name.clone(),
            mag_filter: if filter_mode == 0 {
                SamplerMagFilter::Nearest
            } else {
                SamplerMagFilter::Linear
            },
            min_filter: match (filter_mode, source_has_mips) {
                (0, false) => SamplerMinFilter::Nearest,
                (0, true) => SamplerMinFilter::NearestMipmapNearest,
                (1, false) | (2, false) => SamplerMinFilter::Linear,
                (1, true) => SamplerMinFilter::LinearMipmapNearest,
                (2, true) => SamplerMinFilter::LinearMipmapLinear,
                _ => {
                    return invalid(format!(
                        "texture {:?} has unsupported legacy filter mode {filter_mode}",
                        source_texture.name
                    ));
                }
            },
            wrap_s: match wrap_mode {
                0 => SamplerWrapMode::Repeat,
                1 => SamplerWrapMode::ClampToEdge,
                _ => {
                    return invalid(format!(
                        "texture {:?} has unsupported legacy wrap mode {wrap_mode}",
                        source_texture.name
                    ));
                }
            },
            wrap_t: match wrap_mode {
                0 => SamplerWrapMode::Repeat,
                1 => SamplerWrapMode::ClampToEdge,
                _ => unreachable!("wrap mode validated above"),
            },
            legacy_filter_mode: filter_mode,
            legacy_wrap_mode: wrap_mode,
            anisotropy_level: source_texture.sampler.aniso.value,
            mip_map_bias: source_texture.sampler.mip_bias.value,
        };
        let published_policy = if source_has_mips {
            PublishedMipPolicy::ExactSourceLevels
        } else {
            PublishedMipPolicy::BaseLevelOnly
        };
        let mut native_levels = Vec::with_capacity(source_texture.mip_levels.len());
        let mut report_levels = Vec::with_capacity(source_texture.mip_levels.len());
        for ((level, uri), bytes) in source_texture
            .mip_levels
            .iter()
            .zip(&level_uris)
            .zip(&level_pngs)
        {
            let png_byte_length = u64_count(bytes.len(), "mip PNG byte length")?;
            let png_sha256 = sha256_hex(bytes);
            let native = NativeTextureMipLevel {
                level: level.level,
                width: level.width,
                height: level.height,
                uri: uri.clone(),
                source_byte_offset: u64_count(level.source_byte_offset, "mip source offset")?,
                source_byte_length: u64_count(level.source_byte_length, "mip source length")?,
                source_byte_sha256: level.source_byte_sha256.clone(),
                decoded_rgba8_byte_length: u64_count(
                    level.decoded_rgba_byte_length,
                    "decoded mip RGBA byte length",
                )?,
                decoded_rgba8_sha256: level.decoded_rgba_sha256.clone(),
                png_byte_length,
                png_sha256: png_sha256.clone(),
            };
            report_levels.push(PublishedTextureMipLevelReport {
                level: native.level,
                uri: native.uri.clone(),
                width: native.width,
                height: native.height,
                source_byte_offset: native.source_byte_offset,
                source_byte_length: native.source_byte_length,
                source_byte_sha256: native.source_byte_sha256.clone(),
                decoded_rgba8_byte_length: native.decoded_rgba8_byte_length,
                decoded_rgba8_sha256: native.decoded_rgba8_sha256.clone(),
                png_byte_length: native.png_byte_length,
                png_sha256,
            });
            native_levels.push(native);
            files.push(TexturePublication {
                uri: uri.clone(),
                bytes: bytes.clone(),
            });
        }

        let texture_index = u32_index(textures.len(), "texture count")?;
        textures.push(NativeTexture {
            source_name: source_texture.name.clone(),
            uri: base_uri.clone(),
            width: source_texture.width,
            height: source_texture.height,
            sampler: sampler_index,
            mip_provenance: TextureMipProvenance {
                source_texture_format: source_texture.texture_format,
                source_texture_format_name: source_texture.texture_format_name.clone(),
                source_mip_count: source_texture.mip_count,
                source_chain_byte_length: u64_count(
                    source_texture.source_payload.byte_length,
                    "source texture chain byte length",
                )?,
                source_chain_sha256: source_texture.source_payload.sha256.clone(),
                source_chain_complete: true,
                source_layout: TextureSourceMipLayout::LargestToSmallestContiguous,
                published_pixel_transform: source_texture.payload.pixel_transform,
                published_policy,
            },
            mip_levels: native_levels,
        });
        samplers.push(sampler);
        reports.push(PublishedTextureReport {
            source_name: source_texture.name.clone(),
            uri: base_uri,
            width: source_texture.width,
            height: source_texture.height,
            source_mip_count: source_texture.mip_count,
            published_policy: match published_policy {
                PublishedMipPolicy::BaseLevelOnly => "baseLevelOnly",
                PublishedMipPolicy::ExactSourceLevels => "exactSourceLevels",
            }
            .to_string(),
            byte_length: u64_count(base_bytes.len(), "base PNG byte length")?,
            sha256: sha256_hex(base_bytes),
            mip_levels: report_levels,
        });
        indices.insert(texture_id.clone(), texture_index);
    }
    Ok((textures, samplers, files, reports, indices))
}

pub(super) fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

pub(super) fn build_materials(
    source: &SourceDocument,
    ordered_ids: &[String],
    texture_indices: &BTreeMap<String, u32>,
    textures: &[NativeTexture],
    samplers: &[NativeSampler],
) -> Result<(
    Vec<NativeMaterial>,
    Vec<PublishedMaterialReport>,
    BTreeMap<String, u32>,
)> {
    let mut materials = Vec::with_capacity(ordered_ids.len());
    let mut reports = Vec::with_capacity(ordered_ids.len());
    let mut indices = BTreeMap::new();
    for material_id in ordered_ids {
        let source_material = &source.materials[material_id];
        validate_material_source(material_id, source_material)?;
        let script = source_material.shader.script.text.as_deref().ok_or_else(|| {
            invalid_error(format!(
                "material {:?} shader is not UTF-8 and cannot be translated to typed render state",
                source_material.name
            ))
        })?;
        let shader_texture_defaults =
            crate::legacy_shader_state::exact_shader_texture_defaults(script)
                .map_err(invalid_error)?;
        let declared_texture_slots = shader_texture_defaults
            .iter()
            .map(|property| property.slot.as_str())
            .collect::<BTreeSet<_>>();
        let colors = source_material
            .saved_properties
            .colors
            .iter()
            .enumerate()
            .map(|(index, property)| {
                if property.slot != index {
                    return invalid(format!(
                        "material {:?} color properties are not exact source order",
                        source_material.name
                    ));
                }
                Ok(MaterialColorProperty {
                    name: property.name.clone(),
                    value: [
                        property.value.r,
                        property.value.g,
                        property.value.b,
                        property.value.a,
                    ],
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let floats = source_material
            .saved_properties
            .floats
            .iter()
            .enumerate()
            .map(|(index, property)| {
                if property.slot != index {
                    return invalid(format!(
                        "material {:?} float properties are not exact source order",
                        source_material.name
                    ));
                }
                Ok(MaterialFloatProperty {
                    name: property.name.clone(),
                    value: property.value,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let texture_bindings = source_material
            .saved_properties
            .texture_envs
            .iter()
            .enumerate()
            .map(|(index, environment)| {
                if environment.slot != index {
                    return invalid(format!(
                        "material {:?} texture properties are not exact source order",
                        source_material.name
                    ));
                }
                let texture = environment
                    .texture_id
                    .as_ref()
                    .map(|texture_id| {
                        texture_indices.get(texture_id).copied().ok_or_else(|| {
                            invalid_error(format!(
                                "material {:?} texture index is missing for {texture_id:?}",
                                source_material.name
                            ))
                        })
                    })
                    .transpose()?;
                let dynamic_texture = environment
                    .dynamic_texture
                    .as_ref()
                    .map(native_dynamic_texture_binding)
                    .transpose()?;
                let (source_name, uri, sampler, mip_provenance, mip_levels) = match texture {
                    Some(texture_index) => {
                        let published_texture =
                            textures.get(texture_index as usize).ok_or_else(|| {
                                invalid_error(format!(
                                    "material {:?} slot {:?} resolved texture index is out of bounds",
                                    source_material.name, environment.name
                                ))
                            })?;
                        let descriptor = samplers
                            .get(published_texture.sampler as usize)
                            .ok_or_else(|| {
                                invalid_error(format!(
                                    "material {:?} slot {:?} resolved sampler index is out of bounds",
                                    source_material.name, environment.name
                                ))
                            })?;
                        (
                            Some(published_texture.source_name.clone()),
                            Some(published_texture.uri.clone()),
                            Some(MaterialTextureSamplerBinding {
                                index: published_texture.sampler,
                                descriptor: descriptor.clone(),
                            }),
                            Some(published_texture.mip_provenance.clone()),
                            Some(published_texture.mip_levels.clone()),
                        )
                    }
                    None => (None, None, None, None, None),
                };
                Ok(MaterialTextureBinding {
                    slot: environment.name.clone(),
                    unassigned_stale_null: environment.unassigned_slot,
                    ignored_stale_shader_binding: texture.is_some()
                        && !environment.name.is_empty()
                        && !declared_texture_slots.contains(environment.name.as_str()),
                    dynamic_texture,
                    texture,
                    source_name,
                    uri,
                    sampler,
                    mip_provenance,
                    mip_levels,
                    scale: [environment.scale.x, environment.scale.y],
                    offset: [environment.offset.x, environment.offset.y],
                    pivot: environment.pivot.map(|pivot| [pivot.x, pivot.y]),
                    rotation: environment.rotation,
                    color_space: texture_color_space(&environment.name),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let (render_queue, passes) = crate::legacy_shader_state::resolve_legacy_material(
            &source_material.shader.declared_name,
            script,
            source_material.render_queue.filter(|queue| *queue != -1),
            &floats,
            &colors,
        )
        .map_err(invalid_error)?;
        let material_index = u32_index(materials.len(), "material count")?;
        materials.push(NativeMaterial {
            name: source_material.name.clone(),
            serialized_shader_name: source_material.shader_name.clone(),
            declared_shader_name: source_material.shader.declared_name.clone(),
            legacy_shader_name: source_material.shader.declared_name.clone(),
            render_queue,
            colors,
            floats,
            shader_texture_defaults,
            texture_bindings,
            passes,
            standard_texture_refs_are_loader_hints: true,
        });
        reports.push(PublishedMaterialReport {
            name: source_material.name.clone(),
            serialized_shader_name: source_material.shader_name.clone(),
            declared_shader_name: source_material.shader.declared_name.clone(),
            legacy_shader_name: source_material.shader.declared_name.clone(),
            shader_sha256: source_material.shader.script_sha256.clone(),
            effective_render_queue: render_queue,
            render_pass_count: u64_count(
                materials.last().expect("material just pushed").passes.len(),
                "material render pass count",
            )?,
        });
        indices.insert(material_id.clone(), material_index);
    }
    Ok((materials, reports, indices))
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut value = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

pub(super) fn build_hierarchy(
    source: &SourceDocument,
) -> Result<(Vec<ModelNode>, BTreeMap<String, u32>, u32)> {
    let hierarchy = &source.model_hierarchy;
    if hierarchy.roots.len() != 1 || hierarchy.nodes.is_empty() {
        return invalid("modelHierarchy must contain exactly one root and at least one node");
    }
    let mut path_counts = BTreeMap::<String, usize>::new();
    for node in &hierarchy.nodes {
        *path_counts.entry(node.path.clone()).or_default() += 1;
    }
    let duplicate_paths = path_counts
        .iter()
        .filter_map(|(path, count)| (*count > 1).then_some(path.as_str()))
        .collect::<BTreeSet<_>>();
    for duplicate_path in &duplicate_paths {
        if hierarchy
            .nodes
            .iter()
            .any(|node| node.parent.as_deref() == Some(*duplicate_path))
            || source.skeleton.joints.iter().any(|joint| {
                joint.path == *duplicate_path
                    || duplicate_path.ends_with(&format!("/{}", joint.path))
            })
        {
            return invalid(format!(
                "duplicate hierarchy path {duplicate_path:?} has children or skeleton ownership"
            ));
        }
    }
    let mut paths = BTreeMap::new();
    let mut identities = BTreeSet::new();
    for (index, node) in hierarchy.nodes.iter().enumerate() {
        validate_hierarchy_path(node)?;
        let index = u32_index(index, "hierarchy node count")?;
        paths.entry(node.path.clone()).or_insert(index);
        if !identities.insert((node.source_asset_index, node.transform_path_id)) {
            return invalid(format!(
                "duplicate Transform identity {}:{}",
                node.source_asset_index, node.transform_path_id
            ));
        }
    }
    let mut nodes = Vec::with_capacity(hierarchy.nodes.len());
    let mut path_ordinals = BTreeMap::<String, u32>::new();
    let mut published_paths = BTreeSet::new();
    for node in &hierarchy.nodes {
        let parent = node
            .parent
            .as_ref()
            .map(|path| exact_node_path(&paths, path, "hierarchy parent"))
            .transpose()?;
        if let Some(parent_index) = parent {
            let expected = format!(
                "{}/{}",
                node.parent.as_deref().expect("parent path exists"),
                normalized_hierarchy_path_component(&node.name)
            );
            if node.path != expected {
                return invalid(format!(
                    "hierarchy node path {:?} is not parent/name {:?}",
                    node.path, expected
                ));
            }
            let _ = parent_index;
        }
        let duplicate_count = path_counts.get(&node.path).copied().unwrap_or(1);
        let (published_name, legacy_name, legacy_sibling_ordinal) = if duplicate_count > 1 {
            let ordinal = path_ordinals.entry(node.path.clone()).or_default();
            *ordinal = ordinal
                .checked_add(1)
                .ok_or_else(|| invalid_error("duplicate hierarchy ordinal overflow"))?;
            (
                format!("{}@#{}", node.name, *ordinal),
                Some(node.name.clone()),
                Some(*ordinal),
            )
        } else {
            (node.name.clone(), None, None)
        };
        let published_path = node.parent.as_ref().map_or_else(
            || published_name.clone(),
            |parent| format!("{parent}/{published_name}"),
        );
        if !published_paths.insert(published_path.clone()) {
            return invalid(format!(
                "native hierarchy disambiguation still collides at {published_path:?}"
            ));
        }
        nodes.push(ModelNode {
            name: published_name,
            legacy_name,
            legacy_sibling_ordinal,
            parent,
            translation: node.translation,
            rotation: node.rotation,
            scale: node.scale,
            mesh: None,
            skin: None,
        });
    }
    let parentless = hierarchy
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.parent.is_none().then_some(index))
        .collect::<Vec<_>>();
    if parentless.len() != 1 {
        return invalid("modelHierarchy nodes do not have exactly one parentless root");
    }
    let root_index = u32_index(parentless[0], "root node index")?;
    let root = &hierarchy.roots[0];
    let root_node = &hierarchy.nodes[root_index as usize];
    if root.name != source.logical_name
        || root_node.name != source.logical_name
        || root.path != root_node.path
        || root.path != source.logical_name
        || root.source_asset_index != root_node.source_asset_index
        || root.transform_path_id != root_node.transform_path_id
        || root.game_object_path_id != root_node.game_object_path_id
    {
        return invalid(
            "logicalName, root summary and sole hierarchy root are not exactly identical",
        );
    }
    Ok((nodes, paths, root_index))
}

pub(super) fn binding_component_key(binding: &SourceBinding) -> Result<(String, usize, i64)> {
    let pair = match binding.component_type.as_str() {
        "MeshFilter" => (
            binding
                .component_asset_index
                .or(binding.mesh_filter_asset_index),
            binding.component_path_id.or(binding.mesh_filter_path_id),
        ),
        "SkinnedMeshRenderer" => (
            binding
                .renderer_asset_index
                .or(binding.component_asset_index),
            binding.renderer_path_id.or(binding.component_path_id),
        ),
        other => return invalid(format!("unsupported component type {other:?}")),
    };
    let (Some(asset), Some(path_id)) = pair else {
        return invalid(format!(
            "{} binding is missing its component identity",
            binding.component_type
        ));
    };
    Ok((binding.component_type.clone(), asset, path_id))
}
