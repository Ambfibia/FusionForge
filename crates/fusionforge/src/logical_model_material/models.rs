use super::*;

pub(crate) fn export_exact_mesh_materials(
    env: &UnityEnvironment,
    selected_meshes: &BTreeSet<(usize, i64)>,
) -> Result<ExactMaterialExport, String> {
    export_exact_mesh_materials_from_selection(env, selected_meshes, None)
}

pub(crate) fn export_exact_mesh_materials_from_selection(
    env: &UnityEnvironment,
    selected_meshes: &BTreeSet<(usize, i64)>,
    selected_objects: Option<&BTreeSet<(usize, i64)>>,
) -> Result<ExactMaterialExport, String> {
    let mut mesh_filter_by_game_object = BTreeMap::<(usize, i64), (usize, i64)>::new();

    for_each_selected_object!(env, selected_objects, asset_index, asset, info, {
        if asset.object_type_name(info) != "MeshFilter" {
            continue;
        }
        let body = asset.read_object(asset_index, info)?;
        let Some(game_object) = resolved_pointer_key(
            env,
            body.get("m_GameObject"),
            "MeshFilter.m_GameObject",
            false,
        )
        .ok()
        .flatten() else {
            continue;
        };
        let Some(mesh) = resolved_pointer_key(env, body.get("m_Mesh"), "MeshFilter.m_Mesh", false)
            .ok()
            .flatten()
        else {
            continue;
        };
        mesh_filter_by_game_object.insert(game_object, mesh);
    });

    let mut result = ExactMaterialExport::default();
    let mut non_uniform_meshes = BTreeSet::<(usize, i64)>::new();
    for_each_selected_object!(env, selected_objects, asset_index, asset, info, {
        let object_type = asset.object_type_name(info);
        if !matches!(object_type.as_str(), "MeshRenderer" | "SkinnedMeshRenderer") {
            continue;
        }
        let renderer = asset.read_object(asset_index, info)?;
        let game_object = resolved_pointer_key(
            env,
            renderer.get("m_GameObject"),
            &format!("{}#{}.m_GameObject", asset.name, info.path_id),
            false,
        )?;
        let mesh = if object_type == "SkinnedMeshRenderer" {
            resolved_pointer_key(
                env,
                renderer.get("m_Mesh"),
                &format!("{}#{}.m_Mesh", asset.name, info.path_id),
                false,
            )?
        } else {
            game_object.and_then(|key| mesh_filter_by_game_object.get(&key).copied())
        };
        let Some(mesh) = mesh else {
            continue;
        };
        if !selected_meshes.contains(&mesh) {
            continue;
        }

        let material_values = value_array(renderer.get("m_Materials"));
        let mut slot_values = Vec::with_capacity(material_values.len());
        let mut slot_provenance = Vec::with_capacity(material_values.len());
        for (slot_index, value) in material_values.iter().enumerate() {
            let pointer = match value {
                UnityValue::Pointer(pointer) => pointer,
                _ => {
                    return Err(format!(
                        "{}#{} renderer material slot {} is not a PPtr",
                        asset.name, info.path_id, slot_index
                    ));
                }
            };
            if pointer.is_null() {
                slot_values.push(JsonValue::Null);
                slot_provenance.push(json!({
                    "slot": slot_index,
                    "pointer": pointer_json(pointer),
                    "materialId": JsonValue::Null,
                }));
                continue;
            }

            let material_key = strict_non_null_pointer(env, pointer).map_err(|error| {
                format!(
                    "{}#{} renderer material slot {} ({}/{}) could not be resolved: {}",
                    asset.name, info.path_id, slot_index, pointer.file_id, pointer.path_id, error
                )
            })?;
            ensure_object_type(env, material_key, "Material", "renderer material")?;
            let material_id = object_id(env, material_key);
            if !result.materials.contains_key(&material_id) {
                let exact = exact_material(env, material_key, &mut result.textures)?;
                result.materials.insert(material_id.clone(), exact);
            }
            let material =
                result.materials.get(&material_id).cloned().ok_or_else(|| {
                    format!("internal exact material cache miss for {material_id}")
                })?;
            slot_values.push(material);
            slot_provenance.push(json!({
                "slot": slot_index,
                "pointer": pointer_json(pointer),
                "materialId": material_id,
            }));
        }

        let mesh_key = ObjectKey {
            asset: mesh.0,
            path_id: mesh.1,
        };
        if non_uniform_meshes.contains(&mesh) {
            // This renderer is still retained verbatim below.
        } else if let Some(previous) = result.mesh_materials.get(&mesh) {
            let previous_ids = previous.iter().map(material_value_id).collect::<Vec<_>>();
            let current_ids = slot_values
                .iter()
                .map(material_value_id)
                .collect::<Vec<_>>();
            if previous_ids != current_ids {
                result.mesh_materials.remove(&mesh);
                non_uniform_meshes.insert(mesh);
            }
        } else {
            result.mesh_materials.insert(mesh, slot_values);
        }

        result.renderer_bindings.push(json!({
                "renderer": source_object_json_from_info(env, asset_index, info),
                "rendererType": object_type,
                "gameObject": game_object.map(|key| source_object_json(env, ObjectKey { asset: key.0, path_id: key.1 })),
                "mesh": source_object_json(env, mesh_key),
                "materialSlots": slot_provenance,
            }));
    });
    // This deterministic identity order is not Unity's transparent draw order.
    // The native publisher combines exact ShaderLab queues with a separately
    // recovered renderer-compositing ordinal.
    result.renderer_bindings.sort_by(|left, right| {
        let left = left
            .pointer("/renderer/id")
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        let right = right
            .pointer("/renderer/id")
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        left.cmp(right)
    });
    result.non_uniform_mesh_materials = non_uniform_meshes
        .into_iter()
        .map(|(asset, path_id)| object_id(env, ObjectKey { asset, path_id }))
        .collect();
    Ok(result)
}
