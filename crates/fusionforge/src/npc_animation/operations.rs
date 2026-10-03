use super::*;

pub(super) fn deduplicate_vec3_keys(keys: Vec<Vec3Key>) -> Vec<Vec3Key> {
    let mut result: Vec<Vec3Key> = Vec::with_capacity(keys.len());
    for key in keys {
        if result
            .last()
            .is_some_and(|previous| (previous.time - key.time).abs() <= EPSILON)
        {
            *result.last_mut().expect("key exists") = key;
        } else {
            result.push(key);
        }
    }
    result
}

pub(super) fn deduplicate_quat_keys(keys: Vec<QuatKey>) -> Vec<QuatKey> {
    let mut result: Vec<QuatKey> = Vec::with_capacity(keys.len());
    for key in keys {
        if result
            .last()
            .is_some_and(|previous| (previous.time - key.time).abs() <= EPSILON)
        {
            *result.last_mut().expect("key exists") = key;
        } else {
            result.push(key);
        }
    }
    result
}

pub(super) fn add_missing_track_joints(joints: &mut Vec<Joint>, clips: &[AnimationClip]) {
    let mut paths = joints
        .iter()
        .map(|joint| joint.path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let required = clips
        .iter()
        .flat_map(|clip| clip.tracks.keys())
        .flat_map(path_and_ancestors)
        .collect::<BTreeSet<_>>();
    let mut required = required.into_iter().collect::<Vec<_>>();
    required.sort_by_key(|path| path.matches('/').count());
    for path in required {
        if paths.insert(path.to_ascii_lowercase()) {
            joints.push(Joint {
                path,
                parent: None,
                rest_translation: [0.0, 0.0, 0.0],
                rest_rotation: [0.0, 0.0, 0.0, 1.0],
                rest_scale: [1.0, 1.0, 1.0],
                inverse_bind: None,
            });
        }
    }
}

pub(super) fn skin_positions(mesh: &PreviewMesh, skin: &MeshSkin, globals: &[Matrix4]) -> Vec<[f32; 3]> {
    let skin_matrices = skin
        .palette
        .iter()
        .zip(&skin.inverse_bind_matrices)
        .map(|(joint_index, inverse_bind)| {
            mat_mul(
                skin.skeleton_to_mesh,
                mat_mul(globals[*joint_index], *inverse_bind),
            )
        })
        .collect::<Vec<_>>();
    mesh.positions
        .iter()
        .zip(&skin.bone_indices)
        .zip(&skin.weights)
        .map(|((position, indices), weights)| {
            let mut output = [0.0_f64; 3];
            let mut total_weight = 0.0_f64;
            for influence in 0..4 {
                let weight = weights[influence].max(0.0);
                if weight <= EPSILON {
                    continue;
                }
                let transformed = transform_point(skin_matrices[indices[influence]], *position);
                output[0] += transformed[0] * weight;
                output[1] += transformed[1] * weight;
                output[2] += transformed[2] * weight;
                total_weight += weight;
            }
            if total_weight > EPSILON {
                [
                    (output[0] / total_weight) as f32,
                    (output[1] / total_weight) as f32,
                    (output[2] / total_weight) as f32,
                ]
            } else {
                [position[0] as f32, position[1] as f32, position[2] as f32]
            }
        })
        .collect()
}

pub(super) fn global_matrices(
    joints: &[Joint],
    local_matrices: &[Matrix4],
) -> Result<Vec<Matrix4>, NpcAnimationError> {
    if joints.len() != local_matrices.len() {
        return Err(NpcAnimationError::InvalidPreview(
            "joint/local transform count mismatch".to_string(),
        ));
    }
    fn resolve(
        index: usize,
        joints: &[Joint],
        locals: &[Matrix4],
        states: &mut [u8],
        globals: &mut [Matrix4],
    ) -> Result<Matrix4, NpcAnimationError> {
        match states[index] {
            2 => return Ok(globals[index]),
            1 => {
                return Err(NpcAnimationError::InvalidPreview(format!(
                    "skeleton cycle contains joint '{}'",
                    joints[index].path
                )))
            }
            _ => {}
        }
        states[index] = 1;
        globals[index] = if let Some(parent) = joints[index].parent {
            if parent >= joints.len() {
                return Err(NpcAnimationError::InvalidPreview(format!(
                    "joint '{}' has an invalid parent index",
                    joints[index].path
                )));
            }
            mat_mul(
                resolve(parent, joints, locals, states, globals)?,
                locals[index],
            )
        } else {
            locals[index]
        };
        states[index] = 2;
        Ok(globals[index])
    }

    let mut states = vec![0_u8; joints.len()];
    let mut globals = vec![identity_matrix(); joints.len()];
    for index in 0..joints.len() {
        resolve(index, joints, local_matrices, &mut states, &mut globals)?;
    }
    Ok(globals)
}

pub(super) fn normalized_sample_time(time: f64, duration: f64, looped: bool) -> f64 {
    let time = if time.is_finite() { time } else { 0.0 };
    if duration <= EPSILON {
        return 0.0;
    }
    if looped {
        time.rem_euclid(duration)
    } else {
        time.clamp(0.0, duration)
    }
}

pub(super) fn sample_vec3(keys: &[Vec3Key], time: f64) -> Option<Vec3> {
    let (left, right, blend) = key_interval(keys, time, |key| key.time)?;
    let a = keys[left].value;
    let b = keys[right].value;
    Some([
        lerp(a[0], b[0], blend),
        lerp(a[1], b[1], blend),
        lerp(a[2], b[2], blend),
    ])
}

pub(super) fn sample_quat(keys: &[QuatKey], time: f64) -> Option<Quat> {
    let (left, right, blend) = key_interval(keys, time, |key| key.time)?;
    Some(slerp(keys[left].value, keys[right].value, blend))
}

pub(super) fn key_interval<T, F>(keys: &[T], time: f64, get_time: F) -> Option<(usize, usize, f64)>
where
    F: Fn(&T) -> f64,
{
    if keys.is_empty() {
        return None;
    }
    if keys.len() == 1 || time <= get_time(&keys[0]) {
        return Some((0, 0, 0.0));
    }
    let last = keys.len() - 1;
    if time >= get_time(&keys[last]) {
        return Some((last, last, 0.0));
    }
    let right = keys.partition_point(|key| get_time(key) <= time);
    let left = right.saturating_sub(1);
    let start = get_time(&keys[left]);
    let end = get_time(&keys[right]);
    let blend = if end - start <= EPSILON {
        0.0
    } else {
        ((time - start) / (end - start)).clamp(0.0, 1.0)
    };
    Some((left, right, blend))
}

pub(super) fn first_array<'a>(value: &'a JsonValue, keys: &[&str]) -> Option<&'a [JsonValue]> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(JsonValue::as_array))
        .map(Vec::as_slice)
}

pub(super) fn matrix_from_flat(values: &[JsonValue]) -> Option<Matrix4> {
    if values.len() != 16 {
        return None;
    }
    let mut matrix = [[0.0; 4]; 4];
    for (index, value) in values.iter().enumerate() {
        matrix[index / 4][index % 4] = finite(value.as_f64()?)?;
    }
    Some(matrix)
}

pub(super) fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

pub(super) fn identity_matrix() -> Matrix4 {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub(super) fn compose_matrix(translation: Vec3, quaternion: Quat, scale: Vec3) -> Matrix4 {
    let [x, y, z, w] = normalize_quat(quaternion);
    let [sx, sy, sz] = scale;
    let (xx, yy, zz) = (x * x, y * y, z * z);
    let (xy, xz, yz) = (x * y, x * z, y * z);
    let (wx, wy, wz) = (w * x, w * y, w * z);
    [
        [
            (1.0 - 2.0 * (yy + zz)) * sx,
            (2.0 * (xy - wz)) * sy,
            (2.0 * (xz + wy)) * sz,
            translation[0],
        ],
        [
            (2.0 * (xy + wz)) * sx,
            (1.0 - 2.0 * (xx + zz)) * sy,
            (2.0 * (yz - wx)) * sz,
            translation[1],
        ],
        [
            (2.0 * (xz - wy)) * sx,
            (2.0 * (yz + wx)) * sy,
            (1.0 - 2.0 * (xx + yy)) * sz,
            translation[2],
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub(super) fn mat_mul(left: Matrix4, right: Matrix4) -> Matrix4 {
    let mut result = [[0.0; 4]; 4];
    for row in 0..4 {
        for column in 0..4 {
            result[row][column] = (0..4)
                .map(|index| left[row][index] * right[index][column])
                .sum();
        }
    }
    result
}

pub(super) fn transform_point(matrix: Matrix4, point: Vec3) -> Vec3 {
    [
        matrix[0][0] * point[0] + matrix[0][1] * point[1] + matrix[0][2] * point[2] + matrix[0][3],
        matrix[1][0] * point[0] + matrix[1][1] * point[1] + matrix[1][2] * point[2] + matrix[1][3],
        matrix[2][0] * point[0] + matrix[2][1] * point[1] + matrix[2][2] * point[2] + matrix[2][3],
    ]
}

pub(super) fn invert_matrix(matrix: Matrix4) -> Option<Matrix4> {
    let mut augmented = [[0.0_f64; 8]; 4];
    for row in 0..4 {
        for column in 0..4 {
            augmented[row][column] = matrix[row][column];
        }
        augmented[row][row + 4] = 1.0;
    }
    for column in 0..4 {
        let pivot = (column..4).max_by(|left, right| {
            augmented[*left][column]
                .abs()
                .total_cmp(&augmented[*right][column].abs())
        })?;
        if augmented[pivot][column].abs() <= EPSILON {
            return None;
        }
        if pivot != column {
            augmented.swap(pivot, column);
        }
        let divisor = augmented[column][column];
        for value in &mut augmented[column] {
            *value /= divisor;
        }
        for row in 0..4 {
            if row == column {
                continue;
            }
            let factor = augmented[row][column];
            for index in 0..8 {
                augmented[row][index] -= factor * augmented[column][index];
            }
        }
    }
    let mut inverse = [[0.0; 4]; 4];
    for row in 0..4 {
        inverse[row].copy_from_slice(&augmented[row][4..]);
    }
    Some(inverse)
}

pub(super) fn normalize_quat(value: Quat) -> Quat {
    let length = value
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    if length <= EPSILON || !length.is_finite() {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [
            value[0] / length,
            value[1] / length,
            value[2] / length,
            value[3] / length,
        ]
    }
}

pub(super) fn slerp(left: Quat, mut right: Quat, blend: f64) -> Quat {
    let left = normalize_quat(left);
    right = normalize_quat(right);
    let mut dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    if dot < 0.0 {
        right = [-right[0], -right[1], -right[2], -right[3]];
        dot = -dot;
    }
    if dot > 0.9995 {
        return normalize_quat([
            lerp(left[0], right[0], blend),
            lerp(left[1], right[1], blend),
            lerp(left[2], right[2], blend),
            lerp(left[3], right[3], blend),
        ]);
    }
    let theta = dot.clamp(-1.0, 1.0).acos();
    let sine = theta.sin();
    if sine.abs() <= EPSILON {
        return left;
    }
    let left_weight = ((1.0 - blend) * theta).sin() / sine;
    let right_weight = (blend * theta).sin() / sine;
    normalize_quat([
        left[0] * left_weight + right[0] * right_weight,
        left[1] * left_weight + right[1] * right_weight,
        left[2] * left_weight + right[2] * right_weight,
        left[3] * left_weight + right[3] * right_weight,
    ])
}

pub(super) fn lerp(left: f64, right: f64, blend: f64) -> f64 {
    left + (right - left) * blend
}
