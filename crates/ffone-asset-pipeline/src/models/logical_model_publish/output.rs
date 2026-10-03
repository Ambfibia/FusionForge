use super::*;

pub(super) fn convert_source(source: &SourceDocument) -> Result<ConvertedModel> {
    validate_source_header(source)?;
    let (mut nodes, paths, root_index) = build_hierarchy(source)?;
    let skeleton_root = validate_skeleton(source, &paths)?;
    let visual_selection = select_source_visuals(source);
    let material_conversion = build_material_conversion(source, &paths, &visual_selection)?;
    let mut meshes = Vec::new();
    let mut source_renderer_indices = Vec::new();
    let mut skins = Vec::new();
    let mut occupied_nodes = BTreeSet::new();
    let mut components = BTreeSet::new();
    let mut material_slots = BTreeSet::new();
    let mut explicit_null_slots = 0u64;

    for (source_mesh_index, source_mesh) in source.meshes.iter().enumerate() {
        for (binding_index, binding) in source_mesh.source_bindings.iter().enumerate() {
            if !visual_selection.includes(source_mesh_index, binding_index) {
                continue;
            }
            let renderer_slots =
                &material_conversion.binding_slots[source_mesh_index][binding_index];
            validate_binding_root(source, binding, root_index)?;
            let node_index = exact_binding_node_index(source, binding, "mesh binding")?;
            let source_node = &source.model_hierarchy.nodes[node_index as usize];
            if source_node.source_asset_index != binding.transform_asset_index
                || source_node.transform_path_id != binding.transform_path_id
            {
                return invalid(format!(
                    "mesh {:?} binding transform identity differs from hierarchy node {:?}",
                    source_mesh.name, binding.transform_path
                ));
            }
            let component_key = binding_component_key(binding)?;
            if !components.insert(component_key) {
                return invalid(format!(
                    "duplicate source renderer/component binding for mesh {:?}",
                    source_mesh.name
                ));
            }

            let (render_node_index, skin_index, primitives) = match binding.component_type.as_str()
            {
                "MeshFilter" => {
                    validate_rigid_binding(source_mesh, binding, &paths)?;
                    (
                        node_index,
                        None,
                        build_primitives(source_mesh, renderer_slots, None)?,
                    )
                }
                "SkinnedMeshRenderer" => {
                    let skeleton_root = skeleton_root.ok_or_else(|| {
                        invalid_error(format!(
                            "skinned mesh {:?} has no source skeleton root",
                            source_mesh.name
                        ))
                    })?;
                    let (skin, joints, weights) =
                        build_skin(source_mesh, binding, &paths, skeleton_root)?;
                    let render_node_index = skinned_renderer_output_node(
                        source,
                        source_mesh,
                        node_index,
                        &skin.joints,
                        &nodes,
                    )?;
                    let skin_index = u32_index(skins.len(), "skin count")?;
                    skins.push(skin);
                    let primitives =
                        build_primitives(source_mesh, renderer_slots, Some((&joints, &weights)))?;
                    (render_node_index, Some(skin_index), primitives)
                }
                other => {
                    return invalid(format!(
                        "mesh {:?} has unsupported binding component type {other:?}",
                        source_mesh.name
                    ));
                }
            };
            if !occupied_nodes.insert(render_node_index) {
                return invalid(format!(
                    "hierarchy node {:?} owns more than one render binding; NativeModel cannot preserve that without scattering or a synthetic node",
                    source.model_hierarchy.nodes[render_node_index as usize].path
                ));
            }
            for primitive in &primitives {
                if let Some(slot) = &primitive.material_slot {
                    material_slots.insert(slot.clone());
                } else {
                    explicit_null_slots = explicit_null_slots.checked_add(1).ok_or_else(|| {
                        invalid_error("explicit null material slot count overflow")
                    })?;
                }
            }
            let mesh_index = u32_index(meshes.len(), "mesh count")?;
            let source_renderer_index =
                exact_renderer_material_binding(source, source_mesh, binding, source_node)?;
            meshes.push(ModelMesh {
                name: source_mesh.name.clone(),
                renderer_order: 0,
                primitives,
            });
            source_renderer_indices.push(source_renderer_index);
            nodes[render_node_index as usize].mesh = Some(mesh_index);
            nodes[render_node_index as usize].skin = skin_index;
        }
        if source_mesh.source_bindings.is_empty() {
            return invalid(format!(
                "source mesh {source_mesh_index} {:?} has no Transform binding",
                source_mesh.name
            ));
        }
    }

    assign_legacy_renderer_orders(
        &mut meshes,
        &material_conversion.materials,
        &source_renderer_indices,
    )?;

    let animation_root_path = &source.model_hierarchy.nodes[root_index as usize].path;
    let animations = build_animations(source, &paths, animation_root_path)?;
    let model = NativeModel {
        schema: ffone_skinned_model::MODEL_SCHEMA.to_string(),
        name: source.logical_name.clone(),
        native_coordinate_contract: exact_native_coordinate_contract(),
        roots: vec![root_index],
        nodes,
        meshes,
        skins,
        materials: material_conversion.materials,
        textures: material_conversion.textures,
        samplers: material_conversion.samplers,
        animations,
    };
    let source_counts = source_feature_counts(source, &visual_selection, &paths)?;
    Ok(ConvertedModel {
        model,
        source_counts,
        source_geometry_filter: visual_selection.report(source)?,
        material_slots: material_slots.into_iter().collect(),
        explicit_null_slots,
        texture_files: material_conversion.texture_files,
        material_reports: material_conversion.material_reports,
        texture_reports: material_conversion.texture_reports,
    })
}

pub(super) fn convert_float_curve(
    track: &SourceFloatTrack,
    paths: &BTreeMap<String, u32>,
    root_path: &str,
) -> Result<FloatCurve> {
    if !track.duplicate_keys.is_empty() {
        return invalid(format!(
            "float curve {:?}/{:?} has duplicate-time keys without typed native provenance",
            track.path, track.property
        ));
    }
    // A serialized Unity EditorCurveBinding may intentionally own an empty
    // AnimationCurve. Preserve that zero-key binding in GLB metadata; it has
    // no runtime samples but remains part of the exact source identity.
    if track.source_key_count != track.keys.len() {
        return invalid(format!(
            "float curve {:?}/{:?} does not preserve every serialized key",
            track.path, track.property
        ));
    }
    let interpolation = parse_interpolation(&track.interpolation)?;
    let tangent_modes = tangent_modes(track.keys.iter().map(|key| key.tangent_mode), &track.path)?;
    let (in_tangents, out_tangents) = match interpolation {
        Interpolation::CubicSpline => (
            Some(
                track
                    .keys
                    .iter()
                    .enumerate()
                    .map(|(index, key)| {
                        key.in_tangent.ok_or_else(|| {
                            invalid_error(format!(
                                "float curve {:?} key[{index}] has no in tangent",
                                track.path
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            ),
            Some(
                track
                    .keys
                    .iter()
                    .enumerate()
                    .map(|(index, key)| {
                        key.out_tangent.ok_or_else(|| {
                            invalid_error(format!(
                                "float curve {:?} key[{index}] has no out tangent",
                                track.path
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            ),
        ),
        Interpolation::Linear | Interpolation::Step => {
            if track
                .keys
                .iter()
                .any(|key| key.in_tangent.is_some() || key.out_tangent.is_some())
            {
                return invalid(format!(
                    "non-cubic float curve {:?} contains source tangents",
                    track.path
                ));
            }
            (None, None)
        }
    };
    Ok(FloatCurve {
        target_node: animation_node_path(paths, root_path, &track.path, "float animation track")?,
        target_path: track.path.clone(),
        source_index: u32_index(track.source_index, "float curve source index")?,
        source_key_count: u32_index(track.source_key_count, "float curve source key count")?,
        source_key_indices: track
            .keys
            .iter()
            .map(|key| u32_index(key.source_key_index, "float curve source key index"))
            .collect::<Result<Vec<_>>>()?,
        property: track.property.clone(),
        class_id: track.class_id,
        script: track.script.clone(),
        pre_infinity: track.pre_infinity,
        post_infinity: track.post_infinity,
        interpolation,
        times: track.keys.iter().map(|key| key.time).collect(),
        values: track.keys.iter().map(|key| key.value).collect(),
        in_tangents,
        out_tangents,
        tangent_modes,
    })
}

pub(super) fn convert_duplicate_keys(keys: &SourceDuplicateTrsKeys) -> Result<DuplicateTrsKeys> {
    match keys {
        SourceDuplicateTrsKeys::Vec3(keys) => Ok(DuplicateTrsKeys::Vec3(
            keys.iter()
                .map(|key| {
                    Ok(ExactVec3Key {
                        source_key_index: u32_index(
                            key.source_key_index,
                            "duplicate TRS canonical source key index",
                        )?,
                        time: key.time,
                        value: key.value,
                        in_tangent: key.in_tangent,
                        out_tangent: key.out_tangent,
                        tangent_mode: key.tangent_mode,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        )),
        SourceDuplicateTrsKeys::Quaternion(keys) => Ok(DuplicateTrsKeys::Quaternion(
            keys.iter()
                .map(|key| {
                    Ok(ExactQuaternionKey {
                        source_key_index: u32_index(
                            key.source_key_index,
                            "duplicate TRS canonical source key index",
                        )?,
                        time: key.time,
                        value: key.value,
                        in_tangent: key.in_tangent,
                        out_tangent: key.out_tangent,
                        tangent_mode: key.tangent_mode,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        )),
    }
}

pub(super) fn convert_curve_recovery_canonical(
    track: &SourceAnimationCurveRecoveryCanonicalTrack,
) -> Result<AnimationCurveRecoveryCanonicalTrack> {
    Ok(AnimationCurveRecoveryCanonicalTrack {
        path: track.path.clone(),
        interpolation: parse_interpolation(&track.interpolation)?,
        keys: convert_duplicate_keys(&track.keys)?,
        duplicate_keys: convert_duplicate_animation_keys(&track.duplicate_keys)?,
        source_key_count: u32_index(
            track.source_key_count,
            "curve recovery canonical source key count",
        )?,
        source_index: u32_index(track.source_index, "curve recovery canonical source index")?,
        source_encoding: track.source_encoding,
        track_index: u32_index(track.track_index, "curve recovery canonical track index")?,
    })
}

pub(super) fn convert_curve_recovery_track(
    track: &SourceAnimationCurveRecoveryTrack,
) -> Result<AnimationCurveRecoveryTrack> {
    Ok(AnimationCurveRecoveryTrack {
        path: track.path.clone(),
        interpolation: parse_interpolation(&track.interpolation)?,
        keys: convert_duplicate_keys(&track.keys)?,
        duplicate_keys: convert_duplicate_animation_keys(&track.duplicate_keys)?,
        source_key_count: u32_index(
            track.source_key_count,
            "curve recovery rejected source key count",
        )?,
        source_index: u32_index(track.source_index, "curve recovery rejected source index")?,
        source_encoding: track.source_encoding,
    })
}

pub(super) fn write_publication(output_root: &Path, files: &[PreparedPublicationFile]) -> Result<()> {
    let mut final_keys = BTreeSet::new();
    let mut staged = Vec::with_capacity(files.len());
    for file in files {
        let final_path = output_root.join(&file.relative_path);
        let key = final_path.to_string_lossy().to_ascii_lowercase();
        if !final_keys.insert(key) {
            return invalid(format!(
                "logical-model publication has a case-insensitive output collision at {final_path:?}"
            ));
        }
        if final_path.exists() {
            return invalid(format!(
                "publisher never overwrites an existing GLB/PNG/report: {final_path:?}"
            ));
        }
        let file_name = final_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid_error("logical-model output filename is not Unicode"))?;
        let temp_path = final_path.with_file_name(format!(".{file_name}.ffone-publish.tmp"));
        if temp_path.exists() {
            return invalid(format!(
                "stale logical-model publisher temporary file exists: {temp_path:?}"
            ));
        }
        staged.push((final_path, temp_path, file.bytes.as_slice()));
    }

    for (final_path, _, _) in &staged {
        let parent = final_path
            .parent()
            .ok_or_else(|| invalid_error("logical-model output has no parent directory"))?;
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    for (index, (_, temp_path, bytes)) in staged.iter().enumerate() {
        if let Err(error) = write_new_file(temp_path, bytes) {
            for (_, cleanup, _) in staged.iter().take(index) {
                let _ = fs::remove_file(cleanup);
            }
            return Err(error);
        }
    }
    let mut committed = Vec::with_capacity(staged.len());
    for (index, (final_path, temp_path, _)) in staged.iter().enumerate() {
        if let Err(error) = fs::rename(temp_path, final_path) {
            for (_, cleanup, _) in staged.iter().skip(index) {
                let _ = fs::remove_file(cleanup);
            }
            for cleanup in committed {
                let _ = fs::remove_file(cleanup);
            }
            return Err(io_at(final_path, error));
        }
        committed.push(final_path);
    }
    Ok(())
}

pub(super) fn write_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_at(path, error))?;
    file.write_all(bytes).map_err(|error| io_at(path, error))?;
    file.sync_all().map_err(|error| io_at(path, error))
}
