use super::*;

pub(super) fn parse_clips(preview: &JsonValue) -> Result<Vec<AnimationClip>, NpcAnimationError> {
    let Some(values) = preview.get("animations").and_then(JsonValue::as_array) else {
        return Ok(Vec::new());
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| parse_clip(value, index))
        .collect()
}

pub(super) fn parse_split_vec3_curves(
    data: &JsonValue,
    key: &str,
    tracks: &mut BTreeMap<String, TransformTrack>,
    kind: CurveKind,
) -> Result<(), NpcAnimationError> {
    let Some(curves) = data.get(key).and_then(JsonValue::as_array) else {
        return Ok(());
    };
    for curve in curves {
        let Some(path) = curve.get("path").and_then(JsonValue::as_str) else {
            continue;
        };
        let path = normalized_path(path);
        if path.is_empty() {
            continue;
        }
        let Some(keys) = curve.get("keys").and_then(JsonValue::as_array) else {
            continue;
        };
        let parsed = parse_vec3_keys(keys)?;
        match kind {
            CurveKind::Translation => tracks.entry(path).or_default().translations = parsed,
            CurveKind::Scale => tracks.entry(path).or_default().scales = parsed,
        }
    }
    Ok(())
}

pub(super) fn parse_split_quat_curves(
    data: &JsonValue,
    key: &str,
    tracks: &mut BTreeMap<String, TransformTrack>,
) -> Result<(), NpcAnimationError> {
    let Some(curves) = data.get(key).and_then(JsonValue::as_array) else {
        return Ok(());
    };
    for curve in curves {
        let Some(path) = curve.get("path").and_then(JsonValue::as_str) else {
            continue;
        };
        let path = normalized_path(path);
        if path.is_empty() {
            continue;
        }
        let Some(keys) = curve.get("keys").and_then(JsonValue::as_array) else {
            continue;
        };
        tracks.entry(path).or_default().rotations = parse_quat_keys(keys)?;
    }
    Ok(())
}

pub(super) fn parse_vec3_keys(values: &[JsonValue]) -> Result<Vec<Vec3Key>, NpcAnimationError> {
    let mut keys = Vec::new();
    for value in values {
        let Some(time) = value
            .get("time")
            .or_else(|| value.get("t"))
            .and_then(JsonValue::as_f64)
            .filter(|value| value.is_finite())
        else {
            continue;
        };
        let Some(vector) = value
            .get("value")
            .or_else(|| value.get("v"))
            .and_then(parse_vec3)
        else {
            return Err(NpcAnimationError::InvalidPreview(
                "a Vec3 animation key has no finite value".to_string(),
            ));
        };
        keys.push(Vec3Key {
            time: time.max(0.0),
            value: vector,
        });
    }
    keys.sort_by(|left, right| left.time.total_cmp(&right.time));
    Ok(deduplicate_vec3_keys(keys))
}

pub(super) fn parse_quat_keys(values: &[JsonValue]) -> Result<Vec<QuatKey>, NpcAnimationError> {
    let mut keys = Vec::new();
    for value in values {
        let Some(time) = value
            .get("time")
            .or_else(|| value.get("t"))
            .and_then(JsonValue::as_f64)
            .filter(|value| value.is_finite())
        else {
            continue;
        };
        let Some(quaternion) = value
            .get("value")
            .or_else(|| value.get("v"))
            .and_then(parse_quat)
        else {
            return Err(NpcAnimationError::InvalidPreview(
                "a quaternion animation key has no finite value".to_string(),
            ));
        };
        keys.push(QuatKey {
            time: time.max(0.0),
            value: normalize_quat(quaternion),
        });
    }
    keys.sort_by(|left, right| left.time.total_cmp(&right.time));
    Ok(deduplicate_quat_keys(keys))
}

pub(super) fn parse_parent_ref(value: Option<&JsonValue>) -> Option<ParentRef> {
    let value = value?;
    value
        .as_u64()
        .and_then(|index| usize::try_from(index).ok())
        .map(ParentRef::Index)
        .or_else(|| {
            value
                .as_str()
                .map(normalized_path)
                .filter(|path| !path.is_empty())
                .map(ParentRef::Path)
        })
}

pub(super) fn resolve_joint_parents(joints: &mut [Joint]) -> Result<(), NpcAnimationError> {
    let lookup = joints
        .iter()
        .enumerate()
        .map(|(index, joint)| (joint.path.to_ascii_lowercase(), index))
        .collect::<HashMap<_, _>>();
    for (index, joint) in joints.iter_mut().enumerate() {
        if joint.parent.is_none() {
            joint.parent = parent_path(&joint.path)
                .and_then(|path| lookup.get(&path.to_ascii_lowercase()).copied());
        }
        if joint.parent == Some(index) {
            return Err(NpcAnimationError::InvalidPreview(format!(
                "joint '{}' is its own parent",
                joint.path
            )));
        }
    }
    // This also validates cycles before a sampler reaches the render loop.
    let locals = joints
        .iter()
        .map(|joint| {
            compose_matrix(
                joint.rest_translation,
                joint.rest_rotation,
                joint.rest_scale,
            )
        })
        .collect::<Vec<_>>();
    let _ = global_matrices(joints, &locals)?;
    Ok(())
}

pub(super) fn parse_meshes(
    preview: &JsonValue,
    joints: &[Joint],
    lookup: &JointLookup,
    rest_globals: &[Matrix4],
) -> Vec<PreviewMesh> {
    preview
        .get("meshes")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, value)| {
            let positions = parse_positions(value.get("positions")?)?;
            let id = value
                .get("id")
                .or_else(|| value.get("name"))
                .and_then(JsonValue::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("mesh_{index}"));
            let (skin, skin_warning) = match value.get("skin").or_else(|| value.get("skinning")) {
                Some(skin) => {
                    match parse_mesh_skin(skin, positions.len(), joints, lookup, rest_globals) {
                        Ok(skin) => (Some(skin), None),
                        Err(error) => (None, Some(error)),
                    }
                }
                None => (None, None),
            };
            Some(PreviewMesh {
                id,
                positions,
                skin,
                skin_warning,
            })
        })
        .collect()
}

pub(super) fn parse_positions(value: &JsonValue) -> Option<Vec<Vec3>> {
    let values = value.as_array()?;
    if values.first().is_some_and(JsonValue::is_array) {
        let positions = values.iter().filter_map(parse_vec3).collect::<Vec<_>>();
        return (positions.len() == values.len()).then_some(positions);
    }
    if values.len() % 3 != 0 {
        return None;
    }
    values
        .chunks_exact(3)
        .map(|chunk| Some([chunk[0].as_f64()?, chunk[1].as_f64()?, chunk[2].as_f64()?]))
        .collect()
}

pub(super) fn parse_usize4_stream(value: &JsonValue, expected_rows: usize) -> Option<Vec<[usize; 4]>> {
    let values = value.as_array()?;
    let result = if values.first().is_some_and(JsonValue::is_array) {
        values
            .iter()
            .map(|row| {
                let row = row.as_array()?;
                Some([
                    usize::try_from(row.first()?.as_u64()?).ok()?,
                    usize::try_from(row.get(1)?.as_u64()?).ok()?,
                    usize::try_from(row.get(2)?.as_u64()?).ok()?,
                    usize::try_from(row.get(3)?.as_u64()?).ok()?,
                ])
            })
            .collect::<Option<Vec<_>>>()?
    } else {
        if values.len() % 4 != 0 {
            return None;
        }
        values
            .chunks_exact(4)
            .map(|row| {
                Some([
                    usize::try_from(row[0].as_u64()?).ok()?,
                    usize::try_from(row[1].as_u64()?).ok()?,
                    usize::try_from(row[2].as_u64()?).ok()?,
                    usize::try_from(row[3].as_u64()?).ok()?,
                ])
            })
            .collect::<Option<Vec<_>>>()?
    };
    (result.len() == expected_rows).then_some(result)
}

pub(super) fn parse_f64x4_stream(value: &JsonValue, expected_rows: usize) -> Option<Vec<[f64; 4]>> {
    let values = value.as_array()?;
    let result = if values.first().is_some_and(JsonValue::is_array) {
        values
            .iter()
            .map(|row| {
                let row = row.as_array()?;
                Some([
                    finite(row.first()?.as_f64()?)?,
                    finite(row.get(1)?.as_f64()?)?,
                    finite(row.get(2)?.as_f64()?)?,
                    finite(row.get(3)?.as_f64()?)?,
                ])
            })
            .collect::<Option<Vec<_>>>()?
    } else {
        if values.len() % 4 != 0 {
            return None;
        }
        values
            .chunks_exact(4)
            .map(|row| {
                Some([
                    finite(row[0].as_f64()?)?,
                    finite(row[1].as_f64()?)?,
                    finite(row[2].as_f64()?)?,
                    finite(row[3].as_f64()?)?,
                ])
            })
            .collect::<Option<Vec<_>>>()?
    };
    (result.len() == expected_rows).then_some(result)
}

pub(super) fn parse_matrix_stream(value: &JsonValue, expected: usize) -> Option<Vec<Matrix4>> {
    let values = value.as_array()?;
    let matrices = if values.len() == expected
        && values.first().is_some_and(|value| {
            value.as_array().is_some_and(|matrix| {
                matrix.len() == 16
                    || (matrix.len() == 4 && matrix.first().is_some_and(JsonValue::is_array))
            })
        }) {
        values
            .iter()
            .map(parse_matrix4)
            .collect::<Option<Vec<_>>>()?
    } else if values.len() == expected.saturating_mul(16) {
        values
            .chunks_exact(16)
            .map(|chunk| matrix_from_flat(chunk))
            .collect::<Option<Vec<_>>>()?
    } else {
        return None;
    };
    (matrices.len() == expected).then_some(matrices)
}

pub(super) fn find_track<'a>(
    tracks: &'a BTreeMap<String, TransformTrack>,
    path: &str,
) -> Option<&'a TransformTrack> {
    tracks.get(path).or_else(|| {
        tracks
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(path))
            .map(|(_, track)| track)
    })
}

pub(super) fn parse_vec3(value: &JsonValue) -> Option<Vec3> {
    let result = if let Some(values) = value.as_array() {
        [
            values.first()?.as_f64()?,
            values.get(1)?.as_f64()?,
            values.get(2)?.as_f64()?,
        ]
    } else {
        [
            value.get("x")?.as_f64()?,
            value.get("y")?.as_f64()?,
            value.get("z")?.as_f64()?,
        ]
    };
    result
        .iter()
        .all(|value| value.is_finite())
        .then_some(result)
}

pub(super) fn parse_quat(value: &JsonValue) -> Option<Quat> {
    let result = if let Some(values) = value.as_array() {
        [
            values.first()?.as_f64()?,
            values.get(1)?.as_f64()?,
            values.get(2)?.as_f64()?,
            values.get(3)?.as_f64()?,
        ]
    } else {
        [
            value.get("x")?.as_f64()?,
            value.get("y")?.as_f64()?,
            value.get("z")?.as_f64()?,
            value.get("w")?.as_f64()?,
        ]
    };
    result
        .iter()
        .all(|value| value.is_finite())
        .then_some(result)
}

pub(super) fn parse_matrix4(value: &JsonValue) -> Option<Matrix4> {
    let values = value.as_array()?;
    if values.len() == 16 {
        return matrix_from_flat(values);
    }
    if values.len() != 4 {
        return None;
    }
    let mut matrix = [[0.0; 4]; 4];
    for (row_index, row) in values.iter().enumerate() {
        let row = row.as_array()?;
        if row.len() != 4 {
            return None;
        }
        for (column_index, value) in row.iter().enumerate() {
            matrix[row_index][column_index] = finite(value.as_f64()?)?;
        }
    }
    Some(matrix)
}
