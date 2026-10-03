use super::*;

pub(super) const TERRAIN_LAYER_TEXTURE_PREVIEW_MAX_SIZE: u32 = 256;

pub(super) const TERRAIN_ALPHA_TEXTURE_PREVIEW_MAX_SIZE: u32 = 256;

pub(super) const TERRAIN_COMPOSITE_TEXTURE_SIZE: u32 = 768;

pub fn terrain_to_document(
    env: &UnityEnvironment,
    contents: &UnityValue,
    world_matrix: Option<Matrix4>,
) -> Option<JsonValue> {
    let heightmap = contents.get("m_Heightmap")?;
    let width = heightmap.get("m_Width")?.as_i64()? as usize;
    let height = heightmap.get("m_Height")?.as_i64()? as usize;
    let raw_heights = value_array(heightmap.get("m_Heights"))
        .iter()
        .filter_map(UnityValue::as_f64)
        .collect::<Vec<_>>();
    if width == 0 || height == 0 || raw_heights.len() != width * height {
        return None;
    }

    let terrain_size = vector(contents.get("m_Size"));
    let terrain_scale = vector(heightmap.get("m_Scale"));
    let mut world_size = terrain_size.map(|value| value.0).unwrap_or(512.0);
    let mut world_size_z = terrain_size.map(|value| value.2).unwrap_or(world_size);
    let mut max_height = terrain_size.map(|value| value.1).unwrap_or(90.0);
    if let Some(scale) = terrain_scale {
        if width > 1 {
            world_size = scale.0 * (width - 1) as f64;
            world_size_z = scale.2 * (height - 1) as f64;
            max_height = scale.1;
        }
    }

    let mut display_heights = Vec::with_capacity(width * height);
    for z in 0..height {
        for x in 0..width {
            let raw = raw_heights[x * height + z];
            display_heights.push((raw / 32767.0).clamp(0.0, 1.0) * max_height);
        }
    }

    let splat_uv_mode = "rotate180MirrorX";
    let (layers, alphas, preview_texture) = terrain_splat_preview_data(
        env,
        contents.get("m_SplatDatabase"),
        world_size,
        splat_uv_mode,
    );
    let mut document = json!({
        "width": width,
        "height": height,
        "worldSize": world_size,
        "maxHeight": max_height,
        "heights": display_heights,
        "textureResolution": 128,
        "textureMap": vec![0_u8; 128 * 128],
        "textureLayers": if layers.is_empty() {
            json!([
                {"id": "grass", "name": "Grass", "color": "#4f8f57", "unityMaterialPathId": JsonValue::Null},
                {"id": "dirt", "name": "Dirt", "color": "#8c6a47", "unityMaterialPathId": JsonValue::Null},
                {"id": "rock", "name": "Rock", "color": "#777d82", "unityMaterialPathId": JsonValue::Null},
                {"id": "sand", "name": "Sand", "color": "#c2ad72", "unityMaterialPathId": JsonValue::Null},
                {"id": "painted_water_edge", "name": "Water Edge", "color": "#4a93a8", "unityMaterialPathId": JsonValue::Null}
            ])
        } else {
            JsonValue::Array(layers)
        },
        "previewTextureDataUrl": preview_texture,
        "splatAlphaDataUrls": alphas,
        "splatUvMode": splat_uv_mode,
    });

    if let Some(matrix) = world_matrix {
        let origin = transform_point(matrix, (0.0, 0.0, 0.0));
        let axis_x = vec_sub(transform_point(matrix, (-world_size, 0.0, 0.0)), origin);
        let axis_z = vec_sub(transform_point(matrix, (0.0, 0.0, world_size_z)), origin);
        let height_axis = vec_sub(transform_point(matrix, (0.0, 1.0, 0.0)), origin);
        let reverse_winding = vec_dot(vec_cross(axis_x, axis_z), height_axis) > 0.0;
        if let Some(object) = document.as_object_mut() {
            object.insert("origin".into(), vec_json(origin, 6));
            object.insert("axisX".into(), vec_json(axis_x, 6));
            object.insert("axisZ".into(), vec_json(axis_z, 6));
            object.insert("heightAxis".into(), vec_json(height_axis, 6));
            object.insert("flipX".into(), json!(false));
            object.insert("reverseWinding".into(), json!(reverse_winding));
        }
    }
    Some(document)
}

pub(super) struct TerrainSplatLayerPreview {
    pub(super) document: JsonValue,
    pub(super) image: Option<RgbaImage>,
    pub(super) tile_size: (f64, f64),
    pub(super) color: [f64; 3],
}

pub(super) fn terrain_splat_preview_data(
    env: &UnityEnvironment,
    splat_database: Option<&UnityValue>,
    world_size: f64,
    uv_mode: &str,
) -> (Vec<JsonValue>, Vec<String>, Option<String>) {
    let Some(splat_database) = splat_database else {
        return (Vec::new(), Vec::new(), None);
    };
    let mut layers = Vec::<TerrainSplatLayerPreview>::new();
    let mut alpha_data_urls = Vec::new();
    let mut alpha_images = Vec::new();

    for (index, splat) in value_array(splat_database.get("m_Splats"))
        .iter()
        .enumerate()
    {
        let texture_pointer = splat.get("texture").and_then(UnityValue::as_pointer);
        let texture = texture_pointer.and_then(|pointer| env.resolve_value(pointer).ok());
        let decoded = texture
            .as_ref()
            .and_then(|texture| decode_texture(env, texture));
        let mut layer_image = decoded.as_ref().and_then(DecodedTexture::image);
        if let Some(image) = layer_image.as_mut() {
            repair_transparent_rgb(image, 3);
        }
        let color = layer_image
            .as_ref()
            .map(average_image_color)
            .unwrap_or_else(|| "#5a785a".to_string());
        let color_rgb = hex_color_to_rgb(&color);
        let texture_data_url = layer_image.clone().and_then(|image| {
            image_to_data_url(image, TERRAIN_LAYER_TEXTURE_PREVIEW_MAX_SIZE, true)
        });
        let mut composite_image = layer_image;
        if let Some(image) = composite_image.as_mut() {
            image::imageops::flip_vertical_in_place(image);
        }
        let tile_size = vector2(splat.get("tileSize")).unwrap_or((8.0, 8.0));
        layers.push(TerrainSplatLayerPreview {
            document: json!({
            "id": format!("splat_{index}"),
            "name": texture.as_ref().map(object_name).unwrap_or_else(|| format!("Splat {index}")),
            "color": color,
            "textureDataUrl": texture_data_url,
            "unityMaterialPathId": texture_pointer.map(|pointer| pointer.path_id),
            "tileSize": { "x": tile_size.0, "y": tile_size.1 },
            }),
            image: composite_image,
            tile_size,
            color: color_rgb,
        });
    }

    for pointer in value_array(splat_database.get("m_AlphaTextures")) {
        let Some(pointer) = pointer.as_pointer() else {
            continue;
        };
        let Ok(alpha) = env.resolve_value(pointer) else {
            continue;
        };
        if let Some(data_url) =
            texture_to_data_url(env, &alpha, TERRAIN_ALPHA_TEXTURE_PREVIEW_MAX_SIZE, true)
        {
            alpha_data_urls.push(data_url);
        }
        if let Some(decoded) = decode_texture(env, &alpha) {
            if let Some(mut image) = decoded.image() {
                image::imageops::flip_vertical_in_place(&mut image);
                alpha_images.push(image);
            }
        }
    }

    let preview_texture = composite_terrain_splats(
        &layers,
        &alpha_images,
        TERRAIN_COMPOSITE_TEXTURE_SIZE,
        world_size,
        uv_mode,
    );
    (
        layers.into_iter().map(|layer| layer.document).collect(),
        alpha_data_urls,
        preview_texture,
    )
}

pub(super) fn composite_terrain_splats(
    layers: &[TerrainSplatLayerPreview],
    alpha_images: &[RgbaImage],
    resolution: u32,
    world_size: f64,
    uv_mode: &str,
) -> Option<String> {
    if layers.is_empty() || alpha_images.is_empty() || resolution == 0 {
        return None;
    }

    let mut image = RgbaImage::new(resolution, resolution);
    let denom = resolution.saturating_sub(1).max(1) as f64;
    for y in 0..resolution {
        for x in 0..resolution {
            let logical_u = x as f64 / denom;
            let logical_v = y as f64 / denom;
            let (splat_u, splat_v) = splat_uv(logical_u, logical_v, uv_mode);
            let base_color = sample_layer(layers, 0, splat_u, splat_v, world_size);
            let mut color = [0.0_f64; 3];
            let mut total_weight = 0.0_f64;

            for (layer_index, _) in layers.iter().enumerate() {
                let weight = sample_alpha(alpha_images, layer_index, splat_u, splat_v);
                if weight <= 0.0 {
                    continue;
                }
                let texel = sample_layer(layers, layer_index, splat_u, splat_v, world_size);
                let effective_weight = (weight * texel[3]).clamp(0.0, 1.0);
                color[0] += texel[0] * effective_weight;
                color[1] += texel[1] * effective_weight;
                color[2] += texel[2] * effective_weight;
                total_weight += effective_weight;
            }

            if total_weight < 1.0 {
                let remainder = 1.0 - total_weight;
                color[0] += base_color[0] * remainder;
                color[1] += base_color[1] * remainder;
                color[2] += base_color[2] * remainder;
            } else if total_weight > 1.0 {
                color[0] /= total_weight;
                color[1] /= total_weight;
                color[2] /= total_weight;
            }

            image.put_pixel(
                x,
                y,
                image::Rgba([
                    (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
                    255,
                ]),
            );
        }
    }
    image_to_data_url(image, resolution, false)
}

pub(super) fn splat_uv(u: f64, v: f64, mode: &str) -> (f64, f64) {
    match mode {
        "unityTranspose" => (v, u),
        "rotate90" => (v, 1.0 - u),
        "rotate270" => (1.0 - v, u),
        "rotate180" => (1.0 - u, 1.0 - v),
        "rotate180MirrorX" => (u, 1.0 - v),
        "rotate180MirrorY" => (1.0 - u, v),
        _ => (u, v),
    }
}
