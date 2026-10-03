use super::*;

pub(super) fn read_normals(normals: &UnityValue, normal_signs: &UnityValue) -> Vec<f64> {
    let items = read_packed_bits(normals);
    let signs = read_packed_bits(normal_signs);
    if items.is_empty() || signs.is_empty() {
        return Vec::new();
    }
    let bit_size = normals
        .get("m_BitSize")
        .and_then(UnityValue::as_i64)
        .unwrap_or(0)
        .max(1);
    let max_value = ((1_u64 << bit_size.min(32)) - 1) as f64;
    let range = normals
        .get("m_Range")
        .and_then(UnityValue::as_f64)
        .unwrap_or(0.0)
        / max_value;
    let start = normals
        .get("m_Start")
        .and_then(UnityValue::as_f64)
        .unwrap_or(0.0);
    let mut floats = Vec::with_capacity(signs.len() * 3);
    for (i, sign) in signs.iter().enumerate() {
        let x = items.get(i * 2).copied().unwrap_or(0) as f64 * range + start;
        let y = items.get(i * 2 + 1).copied().unwrap_or(0) as f64 * range + start;
        // The compressed stream stores X/Y plus the sign of a unit normal's
        // Z. Recover the missing component, not its square. Quantization can
        // push X/Y just outside the unit circle; normalize that boundary case.
        let z_squared = 1.0 - x * x - y * y;
        if z_squared >= 0.0 {
            let z = z_squared.sqrt() * if *sign == 0 { -1.0 } else { 1.0 };
            floats.extend([x, y, z]);
        } else {
            let length = (x * x + y * y).sqrt();
            floats.extend([x / length, y / length, 0.0]);
        }
    }
    floats
}
