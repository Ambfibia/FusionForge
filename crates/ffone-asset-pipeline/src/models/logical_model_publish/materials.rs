use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialPublishReport {
    pub status: String,
    pub reason: String,
    pub preserved_slot_names: Vec<String>,
    pub explicit_null_slots: u64,
    pub material_count: u64,
    pub texture_count: u64,
    pub sampler_count: u64,
    pub materials: Vec<PublishedMaterialReport>,
    pub textures: Vec<PublishedTextureReport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishedMaterialReport {
    pub name: String,
    pub serialized_shader_name: String,
    pub declared_shader_name: String,
    pub legacy_shader_name: String,
    pub shader_sha256: String,
    pub effective_render_queue: i32,
    pub render_pass_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceMaterial {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) source: SourceObjectReference,
    pub(super) shader_name: String,
    pub(super) shader: SourceShader,
    pub(super) render_queue: Option<i32>,
    pub(super) saved_properties: SourceSavedProperties,
    #[serde(default)]
    pub(super) render_state: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceShader {
    pub(super) name: String,
    pub(super) declared_name: String,
    pub(super) script: SourceShaderScript,
    pub(super) script_byte_length: usize,
    pub(super) script_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceShaderScript {
    pub(super) encoding: String,
    pub(super) text: Option<String>,
    pub(super) data: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceRendererMaterialBinding {
    pub(super) renderer: SourceObjectReference,
    pub(super) renderer_type: String,
    pub(super) game_object: Option<SourceObjectReference>,
    pub(super) mesh: SourceObjectReference,
    pub(super) material_slots: Vec<SourceRendererMaterialSlot>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceRendererMaterialSlot {
    pub(super) slot: usize,
    pub(super) material_id: Option<String>,
    pub(super) pointer: SourcePointer,
}

pub(super) struct MaterialConversion {
    pub(super) materials: Vec<NativeMaterial>,
    pub(super) textures: Vec<NativeTexture>,
    pub(super) samplers: Vec<NativeSampler>,
    pub(super) binding_slots: Vec<Vec<Vec<ResolvedMaterialSlot>>>,
    pub(super) texture_files: Vec<TexturePublication>,
    pub(super) material_reports: Vec<PublishedMaterialReport>,
    pub(super) texture_reports: Vec<PublishedTextureReport>,
}

#[derive(Clone)]
pub(super) struct ResolvedMaterialSlot {
    pub(super) material: Option<u32>,
    pub(super) name: Option<String>,
}

pub(super) const FUSION_EFFECT_SHADER: &str = "Skin_FusionEffect_blendSrcalphaInvsrcalpha";

pub(super) const FUSION_MATTER_LIGHT_DIR_SHADER: &str = "SkinnedFusionMatterLightDir_blendSrcalphaInvsrcalpha";

pub(super) fn legacy_material_compositor_rank(legacy_shader_name: &str, additive: bool) -> u8 {
    if additive {
        2
    } else if matches!(
        legacy_shader_name,
        FUSION_EFFECT_SHADER | FUSION_MATTER_LIGHT_DIR_SHADER
    ) {
        1
    } else {
        0
    }
}

pub(super) fn build_material_conversion(
    source: &SourceDocument,
    _paths: &BTreeMap<String, u32>,
    visual_selection: &SourceVisualSelection,
) -> Result<MaterialConversion> {
    if source.materials.is_empty() || source.renderer_material_bindings.is_empty() {
        return invalid(
            "exact logical model source has no complete materials/rendererMaterialBindings contract",
        );
    }

    let mut used_renderer_bindings = BTreeSet::new();
    let mut ordered_material_ids = Vec::new();
    let mut used_material_ids = BTreeSet::new();
    let mut selected_material_ids = BTreeSet::new();
    let mut binding_material_ids = Vec::with_capacity(source.meshes.len());
    for (mesh_index, mesh) in source.meshes.iter().enumerate() {
        if mesh.id.trim().is_empty() {
            return invalid(format!(
                "mesh {:?} has no exact source object id",
                mesh.name
            ));
        }
        let mut mesh_bindings = Vec::with_capacity(mesh.source_bindings.len());
        for (binding_index, binding) in mesh.source_bindings.iter().enumerate() {
            let binding_is_selected = visual_selection.includes(mesh_index, binding_index);
            let node_index = exact_binding_node_index(source, binding, "material binding")?;
            let source_node = &source.model_hierarchy.nodes[node_index as usize];
            let renderer_index =
                exact_renderer_material_binding(source, mesh, binding, source_node)?;
            if !used_renderer_bindings.insert(renderer_index) {
                return invalid(format!(
                    "rendererMaterialBindings entry {renderer_index} is joined to more than one mesh binding"
                ));
            }
            let renderer = &source.renderer_material_bindings[renderer_index];
            validate_renderer_slots(mesh, renderer)?;
            let mut slots = Vec::with_capacity(renderer.material_slots.len());
            for (slot_index, slot) in renderer.material_slots.iter().enumerate() {
                if slot.slot != slot_index {
                    return invalid(format!(
                        "renderer {:?} material slots are not exact contiguous source order",
                        renderer.renderer.id
                    ));
                }
                let material_id = match &slot.material_id {
                    Some(material_id) => {
                        if slot.pointer.is_null || slot.pointer.path_id == 0 {
                            return invalid(format!(
                                "renderer {:?} slot {slot_index} has a material id but a null pointer",
                                renderer.renderer.id
                            ));
                        }
                        let material = source.materials.get(material_id).ok_or_else(|| {
                            invalid_error(format!(
                                "renderer {:?} slot {slot_index} references missing material id {material_id:?}",
                                renderer.renderer.id
                            ))
                        })?;
                        validate_source_object(
                            material_id,
                            &material.id,
                            &material.source,
                            "Material",
                        )?;
                        validate_pointer_identity(
                            &slot.pointer,
                            &material.source,
                            "material slot",
                        )?;
                        used_material_ids.insert(material_id.clone());
                        if binding_is_selected && selected_material_ids.insert(material_id.clone())
                        {
                            ordered_material_ids.push(material_id.clone());
                        }
                        binding_is_selected.then(|| material_id.clone())
                    }
                    None => {
                        if !slot.pointer.is_null || slot.pointer.path_id != 0 {
                            return invalid(format!(
                                "renderer {:?} slot {slot_index} omits materialId without an explicit null pointer",
                                renderer.renderer.id
                            ));
                        }
                        None
                    }
                };
                slots.push(material_id);
            }
            mesh_bindings.push(slots);
        }
        binding_material_ids.push(mesh_bindings);
    }
    if used_renderer_bindings.len() != source.renderer_material_bindings.len() {
        return invalid(format!(
            "rendererMaterialBindings contains {} unrelated/unjoined entries",
            source.renderer_material_bindings.len() - used_renderer_bindings.len()
        ));
    }
    if used_material_ids.len() != source.materials.len() {
        return invalid(format!(
            "exact source contains {} material resources not used by any renderer submesh",
            source.materials.len() - used_material_ids.len()
        ));
    }

    let mut source_texture_ids = BTreeSet::new();
    for material_id in &used_material_ids {
        let material = &source.materials[material_id];
        validate_material_source(material_id, material)?;
        for environment in &material.saved_properties.texture_envs {
            if environment.unassigned_slot {
                // The exporter marks a slot unassigned in two shapes and both mean
                // the same thing: the property binds no texture. A *stale-null*
                // slot carries no serialized name and an explicit null pointer. A
                // *dangling* slot keeps its shader property name but its PPtr
                // names no object in the source environment -- authoring left
                // uninitialised bytes in the field, which is how the `_ShaderMap`
                // property of the added NPC characters records file ids near
                // 163774465 while the sibling `_MainTex` of the same material uses
                // file id 0. Neither shape may carry a resolved texture, and only
                // the nameless shape may carry a null pointer: a named slot with a
                // null pointer is an ordinary empty property that the exporter
                // never marks unassigned.
                let pointer_is_null =
                    environment.texture_pointer.is_null || environment.texture_pointer.path_id == 0;
                if environment.texture_id.is_some()
                    || environment.dynamic_texture.is_some()
                    || (pointer_is_null && !environment.name.trim().is_empty())
                {
                    return invalid(format!(
                        "material {:?} has a contradictory unassigned texture property",
                        material.name
                    ));
                }
                continue;
            }
            if environment.texture_id.is_some() && environment.dynamic_texture.is_some() {
                return invalid(format!(
                    "material {:?} slot {:?} references both Texture2D and a dynamic texture",
                    material.name, environment.name
                ));
            }
            if let Some(texture_id) = &environment.texture_id {
                let texture = source.textures.get(texture_id).ok_or_else(|| {
                    invalid_error(format!(
                        "material {:?} slot {:?} references missing texture id {texture_id:?}",
                        material.name, environment.name
                    ))
                })?;
                validate_source_object(texture_id, &texture.id, &texture.source, "Texture2D")?;
                validate_pointer_identity(
                    &environment.texture_pointer,
                    &texture.source,
                    "material texture slot",
                )?;
                source_texture_ids.insert(texture_id.clone());
            } else if let Some(dynamic_texture) = &environment.dynamic_texture {
                validate_dynamic_texture_source(
                    dynamic_texture,
                    &environment.texture_pointer,
                    &material.name,
                    &environment.name,
                )?;
            } else if !environment.texture_pointer.is_null
                || environment.texture_pointer.path_id != 0
            {
                return invalid(format!(
                    "material {:?} slot {:?} omits textureId without an explicit null pointer",
                    material.name, environment.name
                ));
            }
        }
    }
    if source_texture_ids.len() != source.textures.len() {
        return invalid(format!(
            "exact source contains {} texture resources not used by any renderer material",
            source.textures.len() - source_texture_ids.len()
        ));
    }

    let mut ordered_texture_ids = Vec::new();
    let mut used_texture_ids = BTreeSet::new();
    for material_id in &ordered_material_ids {
        let material = &source.materials[material_id];
        for environment in &material.saved_properties.texture_envs {
            if let Some(texture_id) = &environment.texture_id {
                if used_texture_ids.insert(texture_id.clone()) {
                    ordered_texture_ids.push(texture_id.clone());
                }
            }
        }
    }

    let (textures, samplers, texture_files, texture_reports, texture_indices) =
        build_textures(source, &ordered_texture_ids)?;
    let (materials, material_reports, material_indices) = build_materials(
        source,
        &ordered_material_ids,
        &texture_indices,
        &textures,
        &samplers,
    )?;

    let mut binding_slots = Vec::with_capacity(binding_material_ids.len());
    for mesh_bindings in binding_material_ids {
        let mut resolved_mesh_bindings = Vec::with_capacity(mesh_bindings.len());
        for slots in mesh_bindings {
            let mut resolved_slots = Vec::with_capacity(slots.len());
            for material_id in slots {
                let (material, name) = match material_id {
                    Some(material_id) => {
                        let index =
                            material_indices.get(&material_id).copied().ok_or_else(|| {
                                invalid_error(format!(
                                    "internal material index missing for {material_id:?}"
                                ))
                            })?;
                        (
                            Some(index),
                            Some(source.materials[&material_id].name.clone()),
                        )
                    }
                    None => (None, None),
                };
                resolved_slots.push(ResolvedMaterialSlot { material, name });
            }
            resolved_mesh_bindings.push(resolved_slots);
        }
        binding_slots.push(resolved_mesh_bindings);
    }

    Ok(MaterialConversion {
        materials,
        textures,
        samplers,
        binding_slots,
        texture_files,
        material_reports,
        texture_reports,
    })
}

pub(super) fn exact_renderer_material_binding(
    source: &SourceDocument,
    mesh: &SourceMesh,
    binding: &SourceBinding,
    source_node: &SourceNode,
) -> Result<usize> {
    let game_object_path_id = source_node.game_object_path_id.ok_or_else(|| {
        invalid_error(format!(
            "mesh binding {:?} has no hierarchy GameObject identity",
            binding.transform_path
        ))
    })?;
    let matches = source
        .renderer_material_bindings
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            candidate.mesh.id == mesh.id
                && candidate.mesh.object_type == "Mesh"
                && candidate.game_object.as_ref().is_some_and(|game_object| {
                    game_object.object_type == "GameObject"
                        && game_object.asset_index == source_node.source_asset_index
                        && game_object.path_id == game_object_path_id
                })
                && match binding.component_type.as_str() {
                    "SkinnedMeshRenderer" => {
                        candidate.renderer_type == "SkinnedMeshRenderer"
                            && candidate.renderer.object_type == "SkinnedMeshRenderer"
                            && binding.renderer_asset_index == Some(candidate.renderer.asset_index)
                            && binding.renderer_path_id == Some(candidate.renderer.path_id)
                    }
                    "MeshFilter" => {
                        candidate.renderer_type == "MeshRenderer"
                            && candidate.renderer.object_type == "MeshRenderer"
                    }
                    _ => false,
                }
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => invalid(format!(
            "mesh {:?} binding {:?} has no exact rendererMaterialBindings identity match",
            mesh.name, binding.transform_path
        )),
        _ => invalid(format!(
            "mesh {:?} binding {:?} ambiguously matches {} rendererMaterialBindings entries",
            mesh.name,
            binding.transform_path,
            matches.len()
        )),
    }
}

pub(super) fn validate_material_source(material_id: &str, material: &SourceMaterial) -> Result<()> {
    validate_source_object(material_id, &material.id, &material.source, "Material")?;
    let script = &material.shader.script;
    if material.name.trim().is_empty()
        || material.shader_name.trim().is_empty()
        || material.shader.name != material.shader_name
        || material.shader.declared_name.trim().is_empty()
        || script.encoding != "utf-8"
        || script.text.is_none()
        || script.data.is_some()
        || material.render_state.is_null()
    {
        return invalid(format!(
            "material {material_id:?} has contradictory name/shader/render-state evidence"
        ));
    }
    let bytes = script.text.as_deref().expect("checked above").as_bytes();
    if bytes.len() != material.shader.script_byte_length
        || sha256_hex(bytes) != material.shader.script_sha256
    {
        return invalid(format!(
            "material {:?} ShaderLab byteLength/SHA-256 evidence does not match script",
            material.name
        ));
    }
    let declared = crate::legacy_shader_state::exact_declared_shader_name(
        script.text.as_deref().expect("checked above"),
    )
    .map_err(invalid_error)?;
    if declared != material.shader.declared_name {
        return invalid(format!(
            "material {:?} ShaderLab declaredName {:?} contradicts exact script declaration {:?}",
            material.name, material.shader.declared_name, declared
        ));
    }
    Ok(())
}
