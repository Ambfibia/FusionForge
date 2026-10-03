use super::*;

pub(super) fn decode_event_object_parameter(
    value: Option<&fusionforge::UnityValue>,
) -> Result<(Option<JsonValue>, JsonValue), String> {
    let Some(value) = value else {
        return Ok((
            None,
            json!({
                "presence": "missing",
                "interpretation": "missing",
            }),
        ));
    };
    let fusionforge::UnityValue::Pointer(pointer) = value else {
        return Err("has invalid objectReferenceParameter".to_string());
    };
    let pointer_json = json!({
        "sourceAssetIndex": pointer.source_asset,
        "fileId": pointer.file_id,
        "pathId": pointer.path_id,
    });
    if pointer.is_null() {
        Ok((
            None,
            json!({
                "presence": "serialized-pointer",
                "sourceAssetIndex": pointer.source_asset,
                "fileId": pointer.file_id,
                "pathId": pointer.path_id,
                "interpretation": "null-path-id",
            }),
        ))
    } else {
        Ok((
            Some(pointer_json),
            json!({
                "presence": "serialized-pointer",
                "sourceAssetIndex": pointer.source_asset,
                "fileId": pointer.file_id,
                "pathId": pointer.path_id,
                "interpretation": "non-null-unresolved",
            }),
        ))
    }
}

pub(super) fn decode_compressed_quaternion_curves(
    body: &fusionforge::UnityValue,
    output: &mut DecodedCurveTracks,
    empty_trs_bindings: &mut Vec<JsonValue>,
    duplicate_trs_bindings: &mut Vec<JsonValue>,
    curve_recovery_plans: &[CurveRecoveryPlan],
    errors: &mut Vec<String>,
) {
    for (source_index, curve) in fusionforge::value_array(body.get("m_CompressedRotationCurves"))
        .iter()
        .enumerate()
    {
        match decode_compressed_quaternion_curve(curve) {
            Ok((path, keys)) => keep_decoded_curve(
                output,
                empty_trs_bindings,
                duplicate_trs_bindings,
                curve_recovery_plans,
                errors,
                "rotation",
                "compressed",
                source_index,
                path,
                keys,
            ),
            Err(error) => errors.push(format!(
                "m_CompressedRotationCurves[{source_index}]: {error}"
            )),
        }
    }
}

pub(super) fn decode_compressed_quaternion_curve(
    curve: &fusionforge::UnityValue,
) -> Result<(String, DecodedCurveSamples), String> {
    let path = curve
        .get("m_Path")
        .or_else(|| curve.get("path"))
        .and_then(fusionforge::UnityValue::as_str)
        .map(normalized_path)
        .ok_or_else(|| "compressed curve path is absent or invalid".to_string())?;
    let time_deltas = curve
        .get("m_Times")
        .map(fusionforge::read_packed_bits)
        .unwrap_or_default();
    let values = curve
        .get("m_Values")
        .ok_or_else(|| format!("{path}: m_Values is absent"))?;
    let count = values
        .get("m_NumItems")
        .and_then(fusionforge::UnityValue::as_i64)
        .unwrap_or(time_deltas.len() as i64)
        .max(0) as usize;
    if count == 0 {
        return Ok((
            path,
            DecodedCurveSamples {
                keys: Vec::new(),
                duplicate_keys: Vec::new(),
                source_key_count: 0,
            },
        ));
    }
    let data = values
        .get("m_Data")
        .and_then(fusionforge::UnityValue::as_bytes)
        .ok_or_else(|| format!("{path}: packed quaternion bytes are absent"))?;
    if time_deltas.len() != count {
        return Err(format!(
            "{path}: {} packed times for {count} quaternion values",
            time_deltas.len()
        ));
    }
    if data.len() < count.saturating_mul(4) {
        return Err(format!(
            "{path}: {} packed quaternion bytes for {count} values",
            data.len()
        ));
    }

    let mut elapsed_centiseconds = 0_u64;
    let mut keys = Vec::with_capacity(count);
    for (index, delta) in time_deltas.into_iter().enumerate() {
        elapsed_centiseconds = elapsed_centiseconds.saturating_add(u64::from(delta));
        let offset = index * 4;
        let packed = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        let unity = unpack_legacy_compressed_quaternion(packed);
        let preview = normalize_quat([unity[0], -unity[1], -unity[2], unity[3]]);
        keys.push(json!({
            "time": elapsed_centiseconds as f64 * 0.01,
            "value": preview,
        }));
    }
    Ok((path.clone(), canonicalize_curve_keys(&path, keys)?))
}

pub(super) fn decode_mesh_skin(
    mesh: &fusionforge::UnityValue,
    renderer: &SkinnedRenderer,
    paths: &BTreeMap<ObjectKey, String>,
) -> Result<JsonValue, String> {
    let vertex_count = fusionforge::mesh_vertex_count(mesh);
    if vertex_count == 0 {
        return Err("mesh has no vertices".to_string());
    }
    let joint_paths = renderer
        .bones
        .iter()
        .map(|bone| {
            paths
                .get(bone)
                .cloned()
                .ok_or_else(|| format!("bone {}#{} is absent from the skeleton", bone.0, bone.1))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if joint_paths.is_empty() {
        return Err("renderer has no resolvable bone palette".to_string());
    }

    let direct_skin = fusionforge::value_array(mesh.get("m_Skin"));
    let compressed = mesh.get("m_CompressedMesh");
    let has_compressed_weights = compressed
        .and_then(|value| value.get("m_Weights"))
        .map(fusionforge::read_packed_bits)
        .is_some_and(|values| !values.is_empty());
    let has_compressed_indices = compressed
        .and_then(|value| value.get("m_BoneIndices"))
        .map(fusionforge::read_packed_bits)
        .is_some_and(|values| !values.is_empty());
    let (indices, weights, stream_warning) = if !direct_skin.is_empty() {
        decode_direct_skin(mesh, vertex_count, joint_paths.len())?
    } else if has_compressed_weights && has_compressed_indices {
        decode_compressed_skin(mesh, vertex_count, joint_paths.len())?
    } else if joint_paths.len() == 1 {
        // Several legacy FusionFall attachments are authored as a
        // SkinnedMeshRenderer with one bone and no explicit per-vertex skin
        // stream. Unity treats every vertex as fully weighted to that only
        // palette entry. Preserve that semantic instead of dropping JOINTS_0
        // and WEIGHTS_0 from the native model.
        (
            vec![[0, 0, 0, 0]; vertex_count],
            vec![[1.0, 0.0, 0.0, 0.0]; vertex_count],
            Some("implicit rigid one-bone skin reconstructed from renderer palette".to_string()),
        )
    } else {
        return Err(format!(
            "skin streams are incomplete for a renderer with {} bones (weights={}, indices={})",
            joint_paths.len(),
            has_compressed_weights,
            has_compressed_indices
        ));
    };
    let bind_source = if !fusionforge::value_array(mesh.get("m_BindPose")).is_empty() {
        mesh.get("m_BindPose")
    } else {
        renderer.body.get("m_BindPose")
    };
    let bind_poses = fusionforge::value_array(bind_source)
        .iter()
        .filter_map(parse_unity_matrix)
        .map(reflect_matrix_x)
        .collect::<Vec<_>>();
    if bind_poses.len() != joint_paths.len() {
        return Err(format!(
            "{} bind poses for {} renderer bones",
            bind_poses.len(),
            joint_paths.len()
        ));
    }

    let mut value = json!({
        "source": "unity-skinned-mesh-renderer",
        "rendererAssetIndex": renderer.key.0,
        "rendererPathId": renderer.key.1,
        "rendererTransformPathId": renderer.transform.map(|key| key.1),
        "jointPaths": joint_paths,
        "boneIndices": indices.into_iter().flatten().collect::<Vec<_>>(),
        "weights": weights.into_iter().flatten().collect::<Vec<_>>(),
        "inverseBindMatrices": bind_poses,
    });
    if let Some(warning) = stream_warning {
        value
            .as_object_mut()
            .expect("skin object")
            .insert("decodeWarning".to_string(), json!(warning));
    }
    Ok(value)
}

pub(super) fn decode_direct_skin(
    mesh: &fusionforge::UnityValue,
    vertex_count: usize,
    palette_len: usize,
) -> Result<(Vec<[usize; 4]>, Vec<[f64; 4]>, Option<String>), String> {
    let entries = fusionforge::value_array(mesh.get("m_Skin"));
    if entries.len() != vertex_count {
        return Err(format!(
            "direct skin has {} entries for {vertex_count} vertices",
            entries.len()
        ));
    }
    let mut indices = Vec::with_capacity(vertex_count);
    let mut weights = Vec::with_capacity(vertex_count);
    for (vertex, entry) in entries.iter().enumerate() {
        let mut vertex_indices = [0_usize; 4];
        let mut vertex_weights = [0.0_f64; 4];
        for slot in 0..4 {
            let bone = entry
                .get(&format!("boneIndex[{slot}]"))
                .or_else(|| entry.get(&format!("boneIndex{slot}")))
                .and_then(fusionforge::UnityValue::as_i64)
                .unwrap_or(0);
            let Ok(bone) = usize::try_from(bone) else {
                return Err(format!("vertex {vertex} has a negative bone index"));
            };
            if bone >= palette_len {
                return Err(format!(
                    "vertex {vertex} bone index {bone} exceeds palette size {palette_len}"
                ));
            }
            vertex_indices[slot] = bone;
            vertex_weights[slot] = entry
                .get(&format!("weight[{slot}]"))
                .or_else(|| entry.get(&format!("weight{slot}")))
                .and_then(fusionforge::UnityValue::as_f64)
                .filter(|value| value.is_finite())
                .unwrap_or(0.0)
                .max(0.0);
        }
        normalize_weights(&mut vertex_weights);
        indices.push(vertex_indices);
        weights.push(vertex_weights);
    }
    Ok((indices, weights, None))
}

pub(super) fn decode_compressed_skin(
    mesh: &fusionforge::UnityValue,
    vertex_count: usize,
    palette_len: usize,
) -> Result<(Vec<[usize; 4]>, Vec<[f64; 4]>, Option<String>), String> {
    let compressed = mesh
        .get("m_CompressedMesh")
        .ok_or_else(|| "m_CompressedMesh is absent".to_string())?;
    let packed_weights = compressed
        .get("m_Weights")
        .map(fusionforge::read_packed_bits)
        .unwrap_or_default();
    let packed_indices = compressed
        .get("m_BoneIndices")
        .map(fusionforge::read_packed_bits)
        .unwrap_or_default();
    if packed_weights.is_empty() || packed_indices.is_empty() {
        return Err("compressed weight or bone-index stream is empty".to_string());
    }

    decode_compressed_skin_streams(&packed_weights, &packed_indices, vertex_count, palette_len)
}

pub(super) fn decode_compressed_skin_streams(
    packed_weights: &[u32],
    packed_indices: &[u32],
    vertex_count: usize,
    palette_len: usize,
) -> Result<(Vec<[usize; 4]>, Vec<[f64; 4]>, Option<String>), String> {
    let mut weight_cursor = 0_usize;
    let mut index_cursor = 0_usize;
    let mut indices = Vec::with_capacity(vertex_count);
    let mut weights = Vec::with_capacity(vertex_count);
    for vertex in 0..vertex_count {
        let mut vertex_indices = [0_usize; 4];
        let mut vertex_weights = [0.0_f64; 4];
        let mut accumulated = 0_u32;
        for slot in 0..4 {
            if accumulated >= 31 {
                break;
            }
            let quantized_weight = if slot == 3 {
                31 - accumulated
            } else {
                let value = *packed_weights
                    .get(weight_cursor)
                    .ok_or_else(|| format!("compressed weight stream ended at vertex {vertex}"))?;
                weight_cursor += 1;
                value.min(31 - accumulated)
            };
            let bone = *packed_indices
                .get(index_cursor)
                .ok_or_else(|| format!("compressed bone-index stream ended at vertex {vertex}"))?
                as usize;
            index_cursor += 1;
            if bone >= palette_len {
                return Err(format!(
                    "vertex {vertex} bone index {bone} exceeds palette size {palette_len}"
                ));
            }
            vertex_indices[slot] = bone;
            vertex_weights[slot] = quantized_weight as f64 / 31.0;
            accumulated += quantized_weight;
        }
        normalize_weights(&mut vertex_weights);
        indices.push(vertex_indices);
        weights.push(vertex_weights);
    }

    let warning = (weight_cursor != packed_weights.len() || index_cursor != packed_indices.len())
        .then(|| {
            format!(
                "decoded {weight_cursor}/{} weights and {index_cursor}/{} bone indices",
                packed_weights.len(),
                packed_indices.len()
            )
        });
    Ok((indices, weights, warning))
}
