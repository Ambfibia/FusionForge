use super::*;

pub(super) fn multiply_matrix4(left: [[f64; 4]; 4], right: [[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut out = [[0.0_f64; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            out[row][col] = (0..4)
                .map(|index| left[row][index] * right[index][col])
                .sum();
        }
    }
    out
}

pub(super) fn identity_matrix4x4() -> ImportedMatrix4x4 {
    ImportedMatrix4x4 {
        values: [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ],
    }
}

pub(super) fn append_strip(strip: &[u16], out: &mut Vec<u16>) {
    if strip.len() < 3 {
        return;
    }
    if out.is_empty() {
        out.extend_from_slice(strip);
        return;
    }
    if let Some(joined) = best_strip_join_sequence(out, strip) {
        out.extend_from_slice(&joined);
        return;
    }
    let last = *out.last().unwrap_or(&strip[0]);
    if (out.len() + 2) % 2 == 0 {
        out.extend_from_slice(&[last, strip[0], strip[0], strip[1], strip[2]]);
    } else {
        out.extend_from_slice(&[last, last, strip[0], strip[0], strip[1], strip[2]]);
    }
    out.extend_from_slice(&strip[3..]);
}

pub(super) fn best_strip_join_sequence(out: &[u16], strip: &[u16]) -> Option<Vec<u16>> {
    if out.len() < 2 || strip.len() < 3 {
        return None;
    }
    let target = strip_to_triangles_u16(strip);
    let tail = [out[out.len() - 2], out[out.len() - 1]];
    let mut alphabet = Vec::from(tail);
    for &value in strip.iter().take(3) {
        if !alphabet.contains(&value) {
            alphabet.push(value);
        }
    }

    let mut best = None::<Vec<u16>>;
    for skip in 0..=2.min(strip.len()) {
        for bridge_len in 0..=4 {
            let mut current = Vec::with_capacity(bridge_len);
            search_strip_join_bridge(
                &mut best,
                &mut current,
                bridge_len,
                &alphabet,
                &tail,
                out.len(),
                strip,
                skip,
                &target,
            );
        }
    }
    best
}

pub(super) fn search_strip_join_bridge(
    best: &mut Option<Vec<u16>>,
    current: &mut Vec<u16>,
    remaining: usize,
    alphabet: &[u16],
    tail: &[u16; 2],
    out_len: usize,
    strip: &[u16],
    skip: usize,
    target: &[u16],
) {
    if remaining == 0 {
        let mut candidate = current.clone();
        candidate.extend_from_slice(&strip[skip..]);
        let produced = strip_to_triangles_from_tail(tail, out_len, &candidate);
        if produced == target
            && best
                .as_ref()
                .is_none_or(|existing| candidate.len() < existing.len())
        {
            *best = Some(candidate);
        }
        return;
    }
    if best
        .as_ref()
        .is_some_and(|existing| current.len() + strip.len().saturating_sub(skip) >= existing.len())
    {
        return;
    }
    for &value in alphabet {
        current.push(value);
        search_strip_join_bridge(
            best,
            current,
            remaining - 1,
            alphabet,
            tail,
            out_len,
            strip,
            skip,
            target,
        );
        current.pop();
    }
}

pub(super) fn strip_to_triangles_from_tail(tail: &[u16; 2], out_len: usize, appended: &[u16]) -> Vec<u16> {
    let mut tris = Vec::new();
    let mut window = vec![tail[0], tail[1]];
    for (offset, value) in appended.iter().enumerate() {
        window.push(*value);
        if window.len() < 3 {
            continue;
        }
        let (t1, t2, t3) = (window[0], window[1], window[2]);
        window.remove(0);
        if t1 == t2 || t1 == t3 || t2 == t3 {
            continue;
        }
        let global_index = out_len + offset;
        if global_index % 2 == 0 {
            tris.extend([t1, t2, t3]);
        } else {
            tris.extend([t2, t1, t3]);
        }
    }
    tris
}

pub(super) fn estimate_bidirectional_strip_extension(
    strip: &[u16],
    triangles: &[[u16; 3]],
    adjacency: &BTreeMap<(u16, u16), Vec<usize>>,
    used: &[bool],
    local_used: &BTreeSet<usize>,
    depth: usize,
) -> usize {
    if depth == 0 {
        return 0;
    }

    let tail = next_strip_candidates(strip, triangles, adjacency, used, local_used);
    let mut reversed = strip.to_vec();
    reversed.reverse();
    let head = next_strip_candidates(&reversed, triangles, adjacency, used, local_used);

    tail.into_iter()
        .chain(head)
        .map(|(triangle_index, next_strip)| {
            let mut next_used = local_used.clone();
            next_used.insert(triangle_index);
            1 + estimate_bidirectional_strip_extension(
                &next_strip,
                triangles,
                adjacency,
                used,
                &next_used,
                depth - 1,
            )
        })
        .max()
        .unwrap_or(0)
}

pub(super) fn next_strip_candidates(
    strip: &[u16],
    triangles: &[[u16; 3]],
    adjacency: &BTreeMap<(u16, u16), Vec<usize>>,
    used: &[bool],
    local_used: &BTreeSet<usize>,
) -> Vec<(usize, Vec<u16>)> {
    if strip.len() < 2 {
        return Vec::new();
    }
    let edge_key = undirected_edge_key(strip[strip.len() - 2], strip[strip.len() - 1]);
    let Some(candidate_indices) = adjacency.get(&edge_key) else {
        return Vec::new();
    };
    let expected_edge = if strip.len() % 2 == 0 {
        (strip[strip.len() - 2], strip[strip.len() - 1])
    } else {
        (strip[strip.len() - 1], strip[strip.len() - 2])
    };

    candidate_indices
        .iter()
        .filter_map(|&triangle_index| {
            if used.get(triangle_index).copied().unwrap_or(true)
                || local_used.contains(&triangle_index)
            {
                return None;
            }
            let triangle = triangles[triangle_index];
            let vertex = orientation_preserving_third_vertex(triangle, expected_edge)?;
            let mut next_strip = strip.to_vec();
            next_strip.push(vertex);
            Some((triangle_index, next_strip))
        })
        .collect()
}

pub(super) fn mark_strip_triangles_used(strip: &[u16], triangles: &[[u16; 3]], used: &mut [bool]) {
    for triangle in strip_to_triangles_u16(strip).chunks_exact(3) {
        let current = [triangle[0], triangle[1], triangle[2]];
        if let Some((index, _)) = triangles.iter().enumerate().find(|(index, candidate)| {
            !used[*index] && same_triangle_vertices(**candidate, current)
        }) {
            used[index] = true;
        }
    }
}

pub(super) fn undirected_edge_key(a: u16, b: u16) -> (u16, u16) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

pub(super) fn strip_to_triangles_u16(indices: &[u16]) -> Vec<u16> {
    let mut tris = Vec::new();
    let mut window = Vec::new();
    for (i, value) in indices.iter().enumerate() {
        window.push(*value);
        if window.len() < 3 {
            continue;
        }
        let (t1, t2, t3) = (window[0], window[1], window[2]);
        window.remove(0);
        if t1 == t2 || t1 == t3 || t2 == t3 {
            continue;
        }
        if i % 2 == 0 {
            tris.extend([t1, t2, t3]);
        } else {
            tris.extend([t2, t1, t3]);
        }
    }
    tris
}

pub(super) fn compressed_skin_streams(mesh: &ImportedMesh) -> (Vec<u32>, Vec<u32>) {
    let mut weights = Vec::new();
    let mut bone_indices = Vec::new();
    for skin in &mesh.skin {
        let influences = quantized_skin_influences(skin);
        for (index, (bone_index, weight)) in influences.iter().enumerate() {
            bone_indices.push(*bone_index);
            // The legacy runtime decoder reads one stored weight per influence
            // and stops as soon as the cumulative weight reaches 31; ONLY a
            // 4th influence is allowed to omit its weight (reconstructed as
            // 31 - sum). Omitting the weight of a 2nd or 3rd influence
            // desynchronizes the weight/index streams for every following
            // vertex, which garbles the skin or crashes the old web player.
            if index < 3 {
                weights.push(*weight);
            }
        }
    }
    (weights, bone_indices)
}

pub(super) fn quantized_skin_influences(skin: &ImportedBoneWeight) -> Vec<(u32, u32)> {
    let mut pairs = (0..4)
        .filter_map(|index| {
            let weight = skin.weights[index].clamp(0.0, 1.0);
            (weight > 0.0).then_some((skin.bone_indices[index].max(0) as u32, weight))
        })
        .collect::<Vec<_>>();
    if pairs.is_empty() {
        return vec![(0, 31)];
    }
    if pairs.len() > 4 {
        pairs.truncate(4);
    }
    let total = pairs.iter().map(|(_, weight)| *weight).sum::<f64>();
    if total > f64::EPSILON {
        for (_, weight) in &mut pairs {
            *weight /= total;
        }
    }

    let mut quantized = pairs
        .iter()
        .map(|(bone_index, weight)| (*bone_index, (*weight * 31.0).round() as i32))
        .collect::<Vec<_>>();
    for (_, weight) in &mut quantized {
        *weight = (*weight).clamp(1, 31);
    }
    let mut sum = quantized.iter().map(|(_, weight)| *weight).sum::<i32>();
    while sum > 31 {
        if let Some((_, weight)) = quantized.iter_mut().max_by_key(|(_, weight)| *weight) {
            if *weight <= 1 {
                break;
            }
            *weight -= 1;
            sum -= 1;
        } else {
            break;
        }
    }
    while sum < 31 {
        if let Some((_, weight)) = quantized.iter_mut().max_by_key(|(_, weight)| *weight) {
            *weight += 1;
            sum += 1;
        } else {
            break;
        }
    }
    quantized
        .into_iter()
        .filter(|(_, weight)| *weight > 0)
        .map(|(bone_index, weight)| (bone_index, weight as u32))
        .collect()
}

pub(super) fn packed_float_vector<I>(values: I, bit_size: u32) -> BTreeMap<String, UnityValue>
where
    I: IntoIterator<Item = f64>,
{
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        return packed_float_object(0, bit_size, 0.0, 0.0, Vec::new());
    }
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for value in &values {
        min = min.min(*value);
        max = max.max(*value);
    }
    let range = (max - min).max(0.0);
    let quantized_max = ((1_u64 << bit_size.min(31)) - 1).max(1) as f64;
    let packed = values
        .iter()
        .map(|value| {
            if range <= f64::EPSILON {
                0
            } else {
                (((*value - min) / range) * quantized_max)
                    .round()
                    .clamp(0.0, quantized_max) as u32
            }
        })
        .collect::<Vec<_>>();
    packed_float_object(
        values.len() as i64,
        bit_size,
        min,
        range,
        pack_bits(&packed, bit_size),
    )
}

pub(super) fn packed_int_vector<I>(values: I, bit_size: u32) -> BTreeMap<String, UnityValue>
where
    I: IntoIterator<Item = u32>,
{
    let values = values.into_iter().collect::<Vec<_>>();
    BTreeMap::from([
        (
            "m_NumItems".to_string(),
            UnityValue::Int(values.len() as i64),
        ),
        ("m_BitSize".to_string(), UnityValue::Int(bit_size as i64)),
        (
            "m_Data".to_string(),
            UnityValue::Bytes(pack_bits(&values, bit_size)),
        ),
    ])
}

pub(super) fn clear_packed_vector(value: &mut BTreeMap<String, UnityValue>) {
    value.insert("m_NumItems".to_string(), UnityValue::Int(0));
    value.insert("m_Data".to_string(), UnityValue::Bytes(Vec::new()));
}

pub(super) fn bit_size_for(max_value: u32) -> u32 {
    (u32::BITS - max_value.leading_zeros()).max(2)
}

pub(super) fn pack_bits(values: &[u32], bit_size: u32) -> Vec<u8> {
    if values.is_empty() || bit_size == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut buffer = 0_u64;
    let mut bit_count = 0_u32;
    let mask = if bit_size >= 32 {
        u64::from(u32::MAX)
    } else {
        (1_u64 << bit_size) - 1
    };
    for value in values {
        buffer |= (u64::from(*value) & mask) << bit_count;
        bit_count += bit_size;
        while bit_count >= 8 {
            out.push((buffer & 0xff) as u8);
            buffer >>= 8;
            bit_count -= 8;
        }
    }
    if bit_count > 0 {
        out.push((buffer & 0xff) as u8);
    }
    out
}

pub(super) fn matrix4x4_value(value: &ImportedMatrix4x4) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        ("e00".to_string(), UnityValue::Float(value.values[0])),
        ("e01".to_string(), UnityValue::Float(value.values[1])),
        ("e02".to_string(), UnityValue::Float(value.values[2])),
        ("e03".to_string(), UnityValue::Float(value.values[3])),
        ("e10".to_string(), UnityValue::Float(value.values[4])),
        ("e11".to_string(), UnityValue::Float(value.values[5])),
        ("e12".to_string(), UnityValue::Float(value.values[6])),
        ("e13".to_string(), UnityValue::Float(value.values[7])),
        ("e20".to_string(), UnityValue::Float(value.values[8])),
        ("e21".to_string(), UnityValue::Float(value.values[9])),
        ("e22".to_string(), UnityValue::Float(value.values[10])),
        ("e23".to_string(), UnityValue::Float(value.values[11])),
        ("e30".to_string(), UnityValue::Float(value.values[12])),
        ("e31".to_string(), UnityValue::Float(value.values[13])),
        ("e32".to_string(), UnityValue::Float(value.values[14])),
        ("e33".to_string(), UnityValue::Float(value.values[15])),
    ]))
}

pub(super) fn vector3_array(values: &[(f64, f64, f64)]) -> UnityValue {
    UnityValue::Array(
        values
            .iter()
            .map(|(x, y, z)| {
                UnityValue::Object(BTreeMap::from([
                    ("x".to_string(), UnityValue::Float(*x)),
                    ("y".to_string(), UnityValue::Float(*y)),
                    ("z".to_string(), UnityValue::Float(*z)),
                ]))
            })
            .collect(),
    )
}

pub(super) fn vector2_array(values: &[(f64, f64)]) -> UnityValue {
    UnityValue::Array(
        values
            .iter()
            .map(|(x, y)| {
                UnityValue::Object(BTreeMap::from([
                    ("x".to_string(), UnityValue::Float(*x)),
                    ("y".to_string(), UnityValue::Float(*y)),
                ]))
            })
            .collect(),
    )
}

pub(super) fn vec3_curve_value(curve: &ImportedVec3Curve) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        (
            "curve".to_string(),
            UnityValue::Object(BTreeMap::from([
                (
                    "m_Curve".to_string(),
                    UnityValue::Array(curve.keys.iter().map(vec3_key_value).collect()),
                ),
                ("m_PreInfinity".to_string(), UnityValue::Int(2)),
                ("m_PostInfinity".to_string(), UnityValue::Int(2)),
                ("m_RotationOrder".to_string(), UnityValue::Int(4)),
            ])),
        ),
        ("path".to_string(), UnityValue::String(curve.path.clone())),
    ]))
}

pub(super) fn should_keep_scale_curve(curve: &ImportedVec3Curve) -> bool {
    if curve.path == "Bip01" || curve.path == "Bip01/Bip01 NonAccum" {
        return true;
    }
    curve
        .keys
        .iter()
        .any(|key| !vec3_near(key.value, (1.0, 1.0, 1.0), 0.0001))
}

pub(super) fn vec3_near(left: (f64, f64, f64), right: (f64, f64, f64), epsilon: f64) -> bool {
    (left.0 - right.0).abs() <= epsilon
        && (left.1 - right.1).abs() <= epsilon
        && (left.2 - right.2).abs() <= epsilon
}

pub(super) fn quaternion_value(value: (f64, f64, f64, f64)) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        ("x".to_string(), UnityValue::Float(value.0)),
        ("y".to_string(), UnityValue::Float(value.1)),
        ("z".to_string(), UnityValue::Float(value.2)),
        ("w".to_string(), UnityValue::Float(value.3)),
    ]))
}

pub(super) fn quat_key_value(key: &ImportedQuatKey) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        ("time".to_string(), UnityValue::Float(key.time)),
        ("value".to_string(), quaternion_value(key.value)),
        (
            "inSlope".to_string(),
            quaternion_value((0.0, 0.0, 0.0, 0.0)),
        ),
        (
            "outSlope".to_string(),
            quaternion_value((0.0, 0.0, 0.0, 0.0)),
        ),
    ]))
}

pub(super) fn quat_curve_value(curve: &ImportedQuatCurve) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        (
            "curve".to_string(),
            UnityValue::Object(BTreeMap::from([
                (
                    "m_Curve".to_string(),
                    UnityValue::Array(curve.keys.iter().map(quat_key_value).collect()),
                ),
                ("m_PreInfinity".to_string(), UnityValue::Int(2)),
                ("m_PostInfinity".to_string(), UnityValue::Int(2)),
                ("m_RotationOrder".to_string(), UnityValue::Int(4)),
            ])),
        ),
        ("path".to_string(), UnityValue::String(curve.path.clone())),
    ]))
}

#[allow(dead_code)]
pub(super) fn compressed_quat_curve_value(curve: &ImportedQuatCurve, sample_rate: f64) -> UnityValue {
    let sample_rate = if sample_rate.is_finite() && sample_rate > 0.0 {
        sample_rate
    } else {
        30.0
    };
    let frame_times = curve
        .keys
        .iter()
        .map(|key| (key.time.max(0.0) * sample_rate).round().max(0.0) as u32)
        .collect::<Vec<_>>();
    let time_bit_size = frame_times
        .iter()
        .copied()
        .max()
        .map(bit_size_for)
        .unwrap_or(1);
    let packed_values = curve
        .keys
        .iter()
        .flat_map(|key| pack_legacy_compressed_quaternion(key.value).to_le_bytes())
        .collect::<Vec<_>>();
    UnityValue::Object(BTreeMap::from([
        ("m_Path".to_string(), UnityValue::String(curve.path.clone())),
        (
            "m_Times".to_string(),
            UnityValue::Object(packed_int_vector(frame_times, time_bit_size)),
        ),
        (
            "m_Values".to_string(),
            UnityValue::Object(BTreeMap::from([
                (
                    "m_NumItems".to_string(),
                    UnityValue::Int(curve.keys.len() as i64),
                ),
                ("m_Data".to_string(), UnityValue::Bytes(packed_values)),
            ])),
        ),
        (
            "m_Slopes".to_string(),
            UnityValue::Object(packed_float_vector(
                std::iter::repeat(0.0).take(curve.keys.len().saturating_mul(4)),
                6,
            )),
        ),
        ("m_PreInfinity".to_string(), UnityValue::Int(2)),
        ("m_PostInfinity".to_string(), UnityValue::Int(2)),
    ]))
}

pub(super) fn pack_legacy_compressed_quaternion(value: (f64, f64, f64, f64)) -> u32 {
    let mut q = [value.0, value.1, value.2, value.3];
    let length = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    if length > f64::EPSILON {
        for component in &mut q {
            *component /= length;
        }
    } else {
        q = [0.0, 0.0, 0.0, 1.0];
    }
    let largest_index = (0..4)
        .max_by(|left, right| {
            q[*left]
                .abs()
                .partial_cmp(&q[*right].abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(3);
    if q[largest_index] < 0.0 {
        for component in &mut q {
            *component = -*component;
        }
    }

    let scale = std::f64::consts::FRAC_1_SQRT_2;
    let mut packed = (largest_index as u32) << 30;
    let mut shift = 0;
    for (index, component) in q.iter().enumerate() {
        if index == largest_index {
            continue;
        }
        let quantized = (((component.clamp(-scale, scale) + scale) / (2.0 * scale)) * 1023.0)
            .round()
            .clamp(0.0, 1023.0) as u32;
        packed |= quantized << shift;
        shift += 10;
    }
    packed
}

pub(super) fn vec3_key_value(key: &ImportedVec3Key) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        ("time".to_string(), UnityValue::Float(key.time)),
        ("value".to_string(), vector3_value(key.value)),
        ("inSlope".to_string(), vector3_value((0.0, 0.0, 0.0))),
        ("outSlope".to_string(), vector3_value((0.0, 0.0, 0.0))),
    ]))
}

pub(super) fn vector3_value(value: (f64, f64, f64)) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        ("x".to_string(), UnityValue::Float(value.0)),
        ("y".to_string(), UnityValue::Float(value.1)),
        ("z".to_string(), UnityValue::Float(value.2)),
    ]))
}

pub(super) fn generated_normals(vertices: &[(f64, f64, f64)], indices: &[u16]) -> Vec<(f64, f64, f64)> {
    let mut normals = vec![(0.0_f64, 0.0_f64, 0.0_f64); vertices.len()];
    for triangle in indices.chunks_exact(3) {
        let a = usize::from(triangle[0]);
        let b = usize::from(triangle[1]);
        let c = usize::from(triangle[2]);
        let Some((&va, &vb, &vc)) = vertices
            .get(a)
            .zip(vertices.get(b))
            .zip(vertices.get(c))
            .map(|((a, b), c)| (a, b, c))
        else {
            continue;
        };
        let ab = (vb.0 - va.0, vb.1 - va.1, vb.2 - va.2);
        let ac = (vc.0 - va.0, vc.1 - va.1, vc.2 - va.2);
        let normal = (
            ab.1 * ac.2 - ab.2 * ac.1,
            ab.2 * ac.0 - ab.0 * ac.2,
            ab.0 * ac.1 - ab.1 * ac.0,
        );
        for index in [a, b, c] {
            let target = &mut normals[index];
            target.0 += normal.0;
            target.1 += normal.1;
            target.2 += normal.2;
        }
    }
    normals
        .into_iter()
        .map(|normal| {
            let length = (normal.0 * normal.0 + normal.1 * normal.1 + normal.2 * normal.2).sqrt();
            if length > f64::EPSILON {
                (normal.0 / length, normal.1 / length, normal.2 / length)
            } else {
                (0.0, 1.0, 0.0)
            }
        })
        .collect()
}
