use super::*;

pub fn identity_matrix() -> Matrix4 {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn mat_mul(a: Matrix4, b: Matrix4) -> Matrix4 {
    let mut result = [[0.0; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            result[row][col] = (0..4).map(|k| a[row][k] * b[k][col]).sum();
        }
    }
    result
}

pub fn transform_point(matrix: Matrix4, point: Vec3) -> Vec3 {
    (
        matrix[0][0] * point.0 + matrix[0][1] * point.1 + matrix[0][2] * point.2 + matrix[0][3],
        matrix[1][0] * point.0 + matrix[1][1] * point.1 + matrix[1][2] * point.2 + matrix[1][3],
        matrix[2][0] * point.0 + matrix[2][1] * point.1 + matrix[2][2] * point.2 + matrix[2][3],
    )
}

pub fn transform_normal(matrix: Matrix4, normal: Vec3) -> Option<Vec3> {
    // A normal is a covector. Baking a non-uniformly-scaled Transform therefore
    // requires `(linear(matrix)^-1)^T`, not the point/direction matrix. The
    // determinant sign is retained so reflected transforms keep their authored
    // normal orientation. Singular transforms fail closed because no
    // mathematically unique inverse exists.
    let (a, b, c) = (matrix[0][0], matrix[0][1], matrix[0][2]);
    let (d, e, f) = (matrix[1][0], matrix[1][1], matrix[1][2]);
    let (g, h, i) = (matrix[2][0], matrix[2][1], matrix[2][2]);
    let cofactor = [
        [e * i - f * h, f * g - d * i, d * h - e * g],
        [c * h - b * i, a * i - c * g, b * g - a * h],
        [b * f - c * e, c * d - a * f, a * e - b * d],
    ];
    let determinant = a * cofactor[0][0] + b * cofactor[0][1] + c * cofactor[0][2];
    if !determinant.is_finite() || determinant.abs() <= 1.0e-12 {
        return None;
    }
    let inverse_determinant = determinant.recip();
    let (nx, ny, nz) = (
        (cofactor[0][0] * normal.0 + cofactor[0][1] * normal.1 + cofactor[0][2] * normal.2)
            * inverse_determinant,
        (cofactor[1][0] * normal.0 + cofactor[1][1] * normal.1 + cofactor[1][2] * normal.2)
            * inverse_determinant,
        (cofactor[2][0] * normal.0 + cofactor[2][1] * normal.1 + cofactor[2][2] * normal.2)
            * inverse_determinant,
    );
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if !len.is_finite() || len <= 1.0e-12 {
        return None;
    }
    Some((nx / len, ny / len, nz / len))
}

pub fn converted_position(value: Option<&UnityValue>) -> Vec3 {
    let vec = vector(value).unwrap_or((0.0, 0.0, 0.0));
    unity_to_native_vec3(vec)
}

pub fn converted_scale(value: Option<&UnityValue>) -> Vec3 {
    unity_to_native_scale(vector(value).unwrap_or((1.0, 1.0, 1.0)))
}

pub fn converted_quaternion(value: Option<&UnityValue>) -> (f64, f64, f64, f64) {
    let object = value.and_then(UnityValue::as_object);
    unity_to_native_quaternion((
        object
            .and_then(|value| value.get("x"))
            .and_then(UnityValue::as_f64)
            .unwrap_or(0.0),
        object
            .and_then(|value| value.get("y"))
            .and_then(UnityValue::as_f64)
            .unwrap_or(0.0),
        object
            .and_then(|value| value.get("z"))
            .and_then(UnityValue::as_f64)
            .unwrap_or(0.0),
        object
            .and_then(|value| value.get("w"))
            .and_then(UnityValue::as_f64)
            .unwrap_or(1.0),
    ))
}

pub fn quaternion_json(value: (f64, f64, f64, f64)) -> JsonValue {
    json!({ "x": value.0, "y": value.1, "z": value.2, "w": value.3 })
}

pub fn compose_matrix(position: Vec3, quat: (f64, f64, f64, f64), scale: Vec3) -> Matrix4 {
    let (x, y, z, w) = quat;
    let (sx, sy, sz) = scale;
    let (xx, yy, zz) = (x * x, y * y, z * z);
    let (xy, xz, yz) = (x * y, x * z, y * z);
    let (wx, wy, wz) = (w * x, w * y, w * z);

    [
        [
            (1.0 - 2.0 * (yy + zz)) * sx,
            (2.0 * (xy - wz)) * sy,
            (2.0 * (xz + wy)) * sz,
            position.0,
        ],
        [
            (2.0 * (xy + wz)) * sx,
            (1.0 - 2.0 * (xx + zz)) * sy,
            (2.0 * (yz - wx)) * sz,
            position.1,
        ],
        [
            (2.0 * (xz - wy)) * sx,
            (2.0 * (yz + wx)) * sy,
            (1.0 - 2.0 * (xx + yy)) * sz,
            position.2,
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub(super) fn round(value: f64, digits: i32) -> f64 {
    let factor = 10_f64.powi(digits);
    (value * factor).round() / factor
}

pub(super) fn vec_json(value: Vec3, digits: i32) -> JsonValue {
    json!({ "x": round(value.0, digits), "y": round(value.1, digits), "z": round(value.2, digits) })
}

pub(super) fn vec_sub(a: Vec3, b: Vec3) -> Vec3 {
    (a.0 - b.0, a.1 - b.1, a.2 - b.2)
}

pub(super) fn vec_cross(a: Vec3, b: Vec3) -> Vec3 {
    (
        a.1 * b.2 - a.2 * b.1,
        a.2 * b.0 - a.0 * b.2,
        a.0 * b.1 - a.1 * b.0,
    )
}

pub(super) fn vec_dot(a: Vec3, b: Vec3) -> f64 {
    a.0 * b.0 + a.1 * b.1 + a.2 * b.2
}

pub fn color_to_hex(value: Option<&UnityValue>, fallback: &str) -> String {
    let Some(object) = value.and_then(UnityValue::as_object) else {
        return fallback.to_string();
    };
    let r = color_byte(object.get("r").and_then(UnityValue::as_f64).unwrap_or(1.0));
    let g = color_byte(object.get("g").and_then(UnityValue::as_f64).unwrap_or(1.0));
    let b = color_byte(object.get("b").and_then(UnityValue::as_f64).unwrap_or(1.0));
    format!("#{r:02x}{g:02x}{b:02x}")
}

pub(super) fn color_alpha(value: Option<&UnityValue>) -> Option<f64> {
    value
        .and_then(UnityValue::as_object)
        .and_then(|object| object.get("a"))
        .and_then(UnityValue::as_f64)
        .map(|value| value.clamp(0.0, 1.0))
}

pub(super) fn color_byte(value: f64) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

pub fn image_to_data_url(mut image: RgbaImage, max_size: u32, flip_y: bool) -> Option<String> {
    if flip_y {
        image::imageops::flip_vertical_in_place(&mut image);
    }
    repair_transparent_rgb(&mut image, 3);
    if max_size > 0 && (image.width() > max_size || image.height() > max_size) {
        image = image::imageops::thumbnail(&image, max_size, max_size);
        repair_transparent_rgb(&mut image, 2);
    }
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8.into(),
        )
        .ok()?;
    Some(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

pub(super) fn repair_transparent_rgb(image: &mut RgbaImage, passes: usize) {
    if !image.pixels().any(|pixel| pixel.0[3] < 255) {
        return;
    }
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return;
    }

    for _ in 0..passes {
        let source = image.clone();
        let mut changed = false;
        for y in 0..height {
            for x in 0..width {
                let current = source.get_pixel(x, y).0;
                if current[3] >= 240 {
                    continue;
                }
                let mut total = [0_u32; 3];
                let mut count = 0_u32;
                let min_y = y.saturating_sub(1);
                let max_y = (y + 1).min(height - 1);
                let min_x = x.saturating_sub(1);
                let max_x = (x + 1).min(width - 1);
                for ny in min_y..=max_y {
                    for nx in min_x..=max_x {
                        if nx == x && ny == y {
                            continue;
                        }
                        let neighbor = source.get_pixel(nx, ny).0;
                        if neighbor[3] <= current[3] || neighbor[3] < 16 {
                            continue;
                        }
                        total[0] += neighbor[0] as u32;
                        total[1] += neighbor[1] as u32;
                        total[2] += neighbor[2] as u32;
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                let pixel = image.get_pixel_mut(x, y);
                pixel.0[0] = (total[0] / count) as u8;
                pixel.0[1] = (total[1] / count) as u8;
                pixel.0[2] = (total[2] / count) as u8;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

pub fn average_image_color(image: &RgbaImage) -> String {
    let thumb = image::imageops::thumbnail(image, 32, 32);
    let mut total_alpha = 0_u64;
    let mut r = 0_u64;
    let mut g = 0_u64;
    let mut b = 0_u64;
    for pixel in thumb.pixels() {
        let [pr, pg, pb, pa] = pixel.0;
        total_alpha += pa as u64;
        r += pr as u64 * pa as u64;
        g += pg as u64 * pa as u64;
        b += pb as u64 * pa as u64;
    }
    if total_alpha == 0 {
        return "#5a785a".to_string();
    }
    format!(
        "#{:02x}{:02x}{:02x}",
        (r / total_alpha) as u8,
        (g / total_alpha) as u8,
        (b / total_alpha) as u8
    )
}

pub(super) fn put_pixel(rgba: &mut [u8], width: u32, height: u32, x: u32, y: u32, color: [u8; 4]) {
    if x >= width || y >= height {
        return;
    }
    let offset = ((y * width + x) * 4) as usize;
    rgba[offset..offset + 4].copy_from_slice(&color);
}

pub(super) fn lerp_color(a: [u8; 4], b: [u8; 4], wa: u16, wb: u16, div: u16) -> [u8; 4] {
    [
        ((a[0] as u16 * wa + b[0] as u16 * wb) / div) as u8,
        ((a[1] as u16 * wa + b[1] as u16 * wb) / div) as u8,
        ((a[2] as u16 * wa + b[2] as u16 * wb) / div) as u8,
        255,
    ]
}

pub(super) fn saved_float(saved: &UnityValue, wanted: &str) -> Option<f64> {
    for entry in value_array(saved.get("m_Floats")) {
        let Some((name, value)) = pair_name_value(entry) else {
            continue;
        };
        if name == wanted {
            return value.as_f64();
        }
    }
    None
}

pub(super) fn image_has_alpha(image: &RgbaImage) -> bool {
    image.pixels().any(|pixel| pixel.0[3] < 255)
}

pub(super) fn image_has_partial_alpha(image: &RgbaImage) -> bool {
    image.pixels().any(|pixel| {
        let alpha = pixel.0[3];
        alpha > 0 && alpha < 255
    })
}

pub(super) fn sampled_mask_alpha(
    mask: &RgbaImage,
    mask_has_alpha: bool,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> u8 {
    let sample_x = if width <= 1 {
        0
    } else {
        ((x as f64 / (width - 1) as f64) * mask.width().saturating_sub(1) as f64).round() as u32
    };
    let sample_y = if height <= 1 {
        0
    } else {
        ((y as f64 / (height - 1) as f64) * mask.height().saturating_sub(1) as f64).round() as u32
    };
    let pixel = mask.get_pixel(sample_x, sample_y).0;
    if mask_has_alpha {
        pixel[3]
    } else {
        ((pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16) / 3) as u8
    }
}

pub(super) fn has_black_key_coverage(image: &RgbaImage, minimum_lit_per_mille: u64) -> bool {
    let total = (image.width() as u64 * image.height() as u64).max(1);
    let mut black = 0_u64;
    let mut lit = 0_u64;
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        if a == 0 {
            continue;
        }
        let max_rgb = r.max(g).max(b);
        if max_rgb <= 8 {
            black += 1;
        } else if max_rgb >= 48 {
            lit += 1;
        }
    }
    black * 100 >= total && lit * 1000 >= total * minimum_lit_per_mille
}

pub(super) fn pointer_ref_loaded_for_preview(env: &UnityEnvironment, pointer: &Pointer) -> bool {
    if pointer.is_null() || pointer.file_id == 0 {
        return true;
    }
    // Early Unity 2.x serialized files occasionally store local references in
    // the signed file-id field. `resolve_pointer` already has the format-7
    // recovery rules for those values, so do not reject a successfully
    // resolved material/texture merely because its raw file id is negative.
    if env.resolve_pointer(pointer).is_ok() {
        return true;
    }
    let Some(source) = env.assets.get(pointer.source_asset) else {
        return false;
    };
    let Some(index) = usize::try_from(pointer.file_id).ok() else {
        return false;
    };
    let Some(asset_ref) = source.asset_refs.get(index) else {
        return false;
    };
    let names = [&asset_ref.file_path, &asset_ref.asset_path]
        .into_iter()
        .map(|value| normalized_preview_asset_ref_name(value))
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if names.is_empty() {
        return true;
    }
    env.assets
        .iter()
        .any(|asset| names.contains(&normalized_preview_asset_ref_name(&asset.name)))
}

pub fn pointer_id(key: &(String, i64)) -> String {
    format!("{}:{}", key.0, key.1)
}

pub(super) fn sample_alpha(alpha_images: &[RgbaImage], layer_index: usize, u: f64, v: f64) -> f64 {
    let alpha_index = layer_index / 4;
    let channel = layer_index % 4;
    let Some(image) = alpha_images.get(alpha_index) else {
        return if layer_index == 0 { 1.0 } else { 0.0 };
    };
    let pixel = sample_image(image, u, v);
    pixel[channel] as f64 / 255.0
}

pub(super) fn sample_layer(
    layers: &[TerrainSplatLayerPreview],
    layer_index: usize,
    u: f64,
    v: f64,
    world_size: f64,
) -> [f64; 4] {
    let Some(layer) = layers.get(layer_index) else {
        return [0.35, 0.47, 0.35, 1.0];
    };
    let Some(image) = layer.image.as_ref() else {
        return [layer.color[0], layer.color[1], layer.color[2], 1.0];
    };
    let repeat_x = world_size / layer.tile_size.0.max(0.001);
    let repeat_y = world_size / layer.tile_size.1.max(0.001);
    let pixel = sample_image(
        image,
        1.0 - repeat_coord(u * repeat_x),
        repeat_coord(v * repeat_y),
    );
    [
        pixel[0] as f64 / 255.0,
        pixel[1] as f64 / 255.0,
        pixel[2] as f64 / 255.0,
        pixel[3] as f64 / 255.0,
    ]
}

pub(super) fn sample_image(image: &RgbaImage, u: f64, v: f64) -> [u8; 4] {
    let x = (u.clamp(0.0, 1.0) * image.width().saturating_sub(1) as f64).round() as u32;
    let y = (v.clamp(0.0, 1.0) * image.height().saturating_sub(1) as f64).round() as u32;
    image.get_pixel(x, y).0
}

pub(super) fn repeat_coord(value: f64) -> f64 {
    let value = value - value.floor();
    if value < 0.0 {
        value + 1.0
    } else {
        value
    }
}

pub(super) fn hex_color_to_rgb(value: &str) -> [f64; 3] {
    let value = value.trim_start_matches('#');
    if value.len() != 6 {
        return [0.35, 0.47, 0.35];
    }
    let parsed = u32::from_str_radix(value, 16).unwrap_or(0x5a785a);
    [
        ((parsed >> 16) & 0xff) as f64 / 255.0,
        ((parsed >> 8) & 0xff) as f64 / 255.0,
        (parsed & 0xff) as f64 / 255.0,
    ]
}

pub(super) fn primary_compressed_uv_channel(uvs: &[f64], vertex_count: usize) -> &[f64] {
    uvs.get(..vertex_count.saturating_mul(2)).unwrap_or(uvs)
}

pub(super) fn compressed_submesh_start(
    first_byte: usize,
    index_count: usize,
    total_indices: usize,
) -> Option<usize> {
    [2_usize, 4_usize, 1_usize]
        .into_iter()
        .filter(|stride| first_byte % stride == 0)
        .map(|stride| first_byte / stride)
        .find(|start| start.saturating_add(index_count) <= total_indices)
}

pub(super) fn strip_to_triangles(indices: &[u32]) -> Vec<u32> {
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
            tris.extend([t3, t2, t1]);
        }
    }
    tris
}

pub fn summarize_transform(path_id: i64, body: &UnityValue) -> JsonValue {
    let position = vector(body.get("m_LocalPosition"))
        .map(unity_to_native_vec3)
        .map(|value| json!({ "x": value.0, "y": value.1, "z": value.2 }));
    let scale = vector(body.get("m_LocalScale"))
        .map(unity_to_native_scale)
        .map(|value| json!({ "x": value.0, "y": value.1, "z": value.2 }));
    json!({
        "coordinateSpace": "native",
        "coordinateContract": native_coordinate_contract_json(),
        "pathId": path_id,
        "gameObject": pointer_summary(body.get("m_GameObject")),
        "position": position,
        "rotation": body.get("m_LocalRotation").map(|value| quaternion_json(converted_quaternion(Some(value)))),
        "scale": scale,
        "children": value_array(body.get("m_Children")).len(),
        "parent": pointer_summary(body.get("m_Father")),
    })
}
