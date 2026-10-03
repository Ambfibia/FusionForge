use super::*;

pub(super) fn push_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend(value.to_le_bytes());
}

pub(super) fn quat_matrix3(rotation: [f32; 4]) -> [[f32; 3]; 3] {
    let [x, y, z, w] = rotation;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;
    [
        [1.0 - 2.0 * (yy + zz), 2.0 * (xy - wz), 2.0 * (xz + wy)],
        [2.0 * (xy + wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz - wx)],
        [2.0 * (xz - wy), 2.0 * (yz + wx), 1.0 - 2.0 * (xx + yy)],
    ]
}

pub(super) fn trs_matrix4_f32(
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
) -> [[f32; 4]; 4] {
    let rotation = quat_matrix3(rotation);
    [
        [
            rotation[0][0] * scale[0],
            rotation[0][1] * scale[1],
            rotation[0][2] * scale[2],
            translation[0],
        ],
        [
            rotation[1][0] * scale[0],
            rotation[1][1] * scale[1],
            rotation[1][2] * scale[2],
            translation[1],
        ],
        [
            rotation[2][0] * scale[0],
            rotation[2][1] * scale[1],
            rotation[2][2] * scale[2],
            translation[2],
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub(super) fn invert_affine_matrix4_f32(matrix: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let a = matrix[0][0];
    let b = matrix[0][1];
    let c = matrix[0][2];
    let d = matrix[1][0];
    let e = matrix[1][1];
    let f = matrix[1][2];
    let g = matrix[2][0];
    let h = matrix[2][1];
    let i = matrix[2][2];
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    assert!(det.abs() > f32::EPSILON, "non-invertible affine matrix");
    let inv_det = 1.0 / det;
    let inv3 = [
        [
            (e * i - f * h) * inv_det,
            (c * h - b * i) * inv_det,
            (b * f - c * e) * inv_det,
        ],
        [
            (f * g - d * i) * inv_det,
            (a * i - c * g) * inv_det,
            (c * d - a * f) * inv_det,
        ],
        [
            (d * h - e * g) * inv_det,
            (b * g - a * h) * inv_det,
            (a * e - b * d) * inv_det,
        ],
    ];
    let tx = matrix[0][3];
    let ty = matrix[1][3];
    let tz = matrix[2][3];
    [
        [
            inv3[0][0],
            inv3[0][1],
            inv3[0][2],
            -(inv3[0][0] * tx + inv3[0][1] * ty + inv3[0][2] * tz),
        ],
        [
            inv3[1][0],
            inv3[1][1],
            inv3[1][2],
            -(inv3[1][0] * tx + inv3[1][1] * ty + inv3[1][2] * tz),
        ],
        [
            inv3[2][0],
            inv3[2][1],
            inv3[2][2],
            -(inv3[2][0] * tx + inv3[2][1] * ty + inv3[2][2] * tz),
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub(super) fn trs_matrix4(
    translation: (f64, f64, f64),
    rotation: (f64, f64, f64, f64),
    scale: (f64, f64, f64),
) -> [[f64; 4]; 4] {
    let (x, y, z, w) = rotation;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;
    let rotation = [
        [1.0 - 2.0 * (yy + zz), 2.0 * (xy - wz), 2.0 * (xz + wy), 0.0],
        [2.0 * (xy + wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz - wx), 0.0],
        [2.0 * (xz - wy), 2.0 * (yz + wx), 1.0 - 2.0 * (xx + yy), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let scale = [
        [scale.0, 0.0, 0.0, 0.0],
        [0.0, scale.1, 0.0, 0.0],
        [0.0, 0.0, scale.2, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut matrix = multiply_matrix4(rotation, scale);
    matrix[0][3] = translation.0;
    matrix[1][3] = translation.1;
    matrix[2][3] = translation.2;
    matrix
}

pub(super) fn imported_matrix4(matrix: &ImportedMatrix4x4) -> [[f64; 4]; 4] {
    [
        [
            matrix.values[0],
            matrix.values[1],
            matrix.values[2],
            matrix.values[3],
        ],
        [
            matrix.values[4],
            matrix.values[5],
            matrix.values[6],
            matrix.values[7],
        ],
        [
            matrix.values[8],
            matrix.values[9],
            matrix.values[10],
            matrix.values[11],
        ],
        [
            matrix.values[12],
            matrix.values[13],
            matrix.values[14],
            matrix.values[15],
        ],
    ]
}

pub(super) fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.000001,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn compressed_skin_streams_store_weights_for_first_three_influences() {
    let mesh = ImportedMesh {
        name: None,
        vertices: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
        submeshes: Vec::new(),
        skin: vec![
            ImportedBoneWeight {
                bone_indices: [7, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            ImportedBoneWeight {
                bone_indices: [3, 9, 0, 0],
                weights: [0.5, 0.5, 0.0, 0.0],
            },
            ImportedBoneWeight {
                bone_indices: [1, 2, 4, 0],
                weights: [0.5, 0.25, 0.25, 0.0],
            },
            ImportedBoneWeight {
                bone_indices: [6, 5, 4, 3],
                weights: [0.4, 0.3, 0.2, 0.1],
            },
        ],
        bind_poses: Vec::new(),
        joint_names: Vec::new(),
    };

    let (weights, bone_indices) = compressed_skin_streams(&mesh);

    assert_eq!(bone_indices, vec![7, 3, 9, 1, 2, 4, 6, 5, 4, 3]);
    // The legacy decoder reads a stored weight for every influence except
    // a 4th one (implicit 31 - sum), so 1+2+3+3 weights are stored here.
    assert_eq!(weights, vec![31, 16, 15, 15, 8, 8, 13, 9, 6]);

    // Simulate the legacy web player decoder: both streams must be
    // consumed exactly.
    let mut wi = 0usize;
    let mut ii = 0usize;
    for _ in 0..mesh.skin.len() {
        let mut sum = 0u32;
        let mut j = 0usize;
        loop {
            if j == 3 {
                ii += 1;
                break;
            }
            sum += weights[wi];
            wi += 1;
            ii += 1;
            j += 1;
            if sum >= 31 {
                break;
            }
        }
    }
    assert_eq!(wi, weights.len());
    assert_eq!(ii, bone_indices.len());
}

#[test]
fn remap_skin_to_bones_matches_legacy_safe_joint_names() {
    let mut mesh = ImportedMesh {
        name: None,
        vertices: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
        submeshes: Vec::new(),
        skin: vec![
            ImportedBoneWeight {
                bone_indices: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            ImportedBoneWeight {
                bone_indices: [1, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
        ],
        bind_poses: vec![
            ImportedMatrix4x4 {
                values: [
                    2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0,
                    1.0,
                ],
            },
            ImportedMatrix4x4 {
                values: [
                    3.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0,
                    1.0,
                ],
            },
        ],
        joint_names: vec![
            "Bip01 Head.002".to_string(),
            "Bip01 L Calf-IKTarget.001".to_string(),
        ],
    };

    let warnings = mesh.remap_skin_to_bones(&[
        "Bip01 Head_002".to_string(),
        "Bip01 L Calf_IKTarget_001".to_string(),
    ]);

    assert!(warnings.is_empty());
    assert_eq!(mesh.skin[0].bone_indices[0], 0);
    assert_eq!(mesh.skin[1].bone_indices[0], 1);
    assert_eq!(mesh.bind_poses[0].values[0], 2.0);
    assert_eq!(mesh.bind_poses[1].values[0], 3.0);
}

#[test]
fn used_joint_indices_ignores_zero_weight_slots() {
    let mesh = ImportedMesh {
        name: None,
        vertices: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
        submeshes: Vec::new(),
        skin: vec![
            ImportedBoneWeight {
                bone_indices: [4, 7, 9, 11],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            ImportedBoneWeight {
                bone_indices: [3, 8, 12, 15],
                weights: [0.5, 0.5, 0.0, 0.0],
            },
        ],
        bind_poses: Vec::new(),
        joint_names: Vec::new(),
    };

    assert_eq!(
        mesh.used_joint_indices().into_iter().collect::<Vec<_>>(),
        vec![3, 4, 8]
    );
}

#[test]
fn best_strip_join_sequence_reduces_simple_join_overhead() {
    let mut out = vec![0, 1, 2];
    let strip = vec![2, 1, 3];
    let joined = best_strip_join_sequence(&out, &strip).expect("join");
    let naive_len = {
        let mut naive = out.clone();
        let last = *naive.last().unwrap();
        if (naive.len() + 2) % 2 == 0 {
            naive.extend_from_slice(&[last, strip[0], strip[0], strip[1], strip[2]]);
        } else {
            naive.extend_from_slice(&[last, last, strip[0], strip[0], strip[1], strip[2]]);
        }
        naive.len() - out.len()
    };

    out.extend_from_slice(&joined);
    assert_eq!(strip_to_triangles_u16(&out), vec![0, 1, 2, 2, 1, 3]);
    assert!(joined.len() <= naive_len);
}
