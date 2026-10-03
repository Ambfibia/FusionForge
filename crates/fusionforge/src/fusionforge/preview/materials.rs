use super::*;

pub(super) fn shader_script_text(shader: &UnityValue) -> Option<String> {
    shader.get("m_Script").and_then(unity_value_text)
}

pub(super) fn shader_declared_name(script: &str) -> Option<String> {
    let marker = "Shader \"";
    let start = script.find(marker)? + marker.len();
    let rest = script.get(start..)?;
    let end = rest.find('"')?;
    let name = rest[..end].trim();
    (!name.is_empty()).then(|| name.to_string())
}

pub(super) fn compact_shader_label(value: &str) -> String {
    value
        .to_lowercase()
        .replace(['_', '-', ' ', '/', '\\', '\r', '\n', '\t'], "")
}

pub(super) fn blend_mode_hint(
    shader_name: &str,
    alpha_mode: &str,
    src_blend: Option<f64>,
    dst_blend: Option<f64>,
) -> &'static str {
    let compact_label = compact_shader_label(shader_name);
    let src_blend = src_blend.map(|value| value.round() as i32);
    let dst_blend = dst_blend.map(|value| value.round() as i32);
    if alpha_mode == "additive"
        || compact_label.contains("blendoneone")
        || compact_label.contains("blendsrcalphaone")
        || matches!(
            (src_blend, dst_blend),
            (Some(5), Some(1)) | (Some(1), Some(1))
        )
    {
        "additive"
    } else if alpha_mode == "blend"
        || compact_label.contains("blendsrcalphainvsrcalpha")
        || matches!(
            (src_blend, dst_blend),
            (Some(5), Some(10)) | (Some(5), Some(6))
        )
    {
        "blend"
    } else if alpha_mode == "cutout" {
        "cutout"
    } else {
        "opaque"
    }
}

pub(super) fn shader_z_write_hint(shader_name: &str) -> Option<bool> {
    let compact_label = compact_shader_label(shader_name);
    if compact_label.contains("zwriteoff") {
        Some(false)
    } else if compact_label.contains("zwriteon") {
        Some(true)
    } else {
        None
    }
}

pub(super) fn shader_cull_off_hint(shader_name: &str) -> Option<bool> {
    compact_shader_label(shader_name)
        .contains("culloff")
        .then_some(true)
}

pub fn material_preview(
    env: &UnityEnvironment,
    material_pointer: &Pointer,
    cache: &mut HashMap<(String, i64), JsonValue>,
) -> Option<JsonValue> {
    if !pointer_ref_loaded_for_preview(env, material_pointer) {
        return None;
    }
    let key = env.pointer_key(material_pointer)?;
    if let Some(value) = cache.get(&key) {
        return Some(value.clone());
    }
    let material = env.resolve_value(material_pointer).ok()?;
    let saved = material.get("m_SavedProperties")?;
    let mut color = "#8b8f76".to_string();
    let mut texture_data_url = JsonValue::Null;
    let mut texture_scale = JsonValue::Null;
    let mut texture_offset = JsonValue::Null;
    let mut texture_pivot = JsonValue::Null;
    let mut texture_rotation = JsonValue::Null;
    let mut texture_width = JsonValue::Null;
    let mut texture_height = JsonValue::Null;
    let mut material_alpha = 1.0_f64;
    let mut shader_name = String::new();

    let mut color_priority = 99;
    for entry in value_array(saved.get("m_Colors")) {
        let Some((name, value)) = pair_name_value(entry) else {
            continue;
        };
        let priority = match name {
            "_Color" => 0,
            "_TintColor" => 1,
            "_MainColor" => 2,
            _ => continue,
        };
        if priority < color_priority {
            color = color_to_hex(Some(value), &color);
            material_alpha = color_alpha(Some(value)).unwrap_or(material_alpha);
            color_priority = priority;
        }
    }

    if let Some(shader_pointer) = material.get("m_Shader").and_then(UnityValue::as_pointer) {
        if pointer_ref_loaded_for_preview(env, shader_pointer) {
            if let Ok(shader) = env.resolve_value(shader_pointer) {
                shader_name = object_name(&shader);
                let shader_script = shader_script_text(&shader).unwrap_or_default();
                if shader_name.is_empty() {
                    if let Some(name) = shader_declared_name(&shader_script) {
                        shader_name = name;
                    }
                }
            }
        }
    }

    let mut main_texture_priority = i32::MAX;
    let mut main_texture_image = None::<RgbaImage>;
    let mut alpha_texture_priority = i32::MAX;
    let mut alpha_texture_image = None::<RgbaImage>;
    for entry in value_array(saved.get("m_TexEnvs")) {
        let Some((name, tex_env)) = pair_name_value(entry) else {
            continue;
        };
        let Some(texture_pointer) = tex_env.get("m_Texture").and_then(UnityValue::as_pointer)
        else {
            continue;
        };
        if !pointer_ref_loaded_for_preview(env, texture_pointer) {
            continue;
        }
        let Ok(texture) = env.resolve_value(texture_pointer) else {
            continue;
        };
        let Some(decoded) = decode_texture(env, &texture) else {
            continue;
        };

        if is_alpha_texture_slot(name) {
            let priority = texture_slot_priority(name);
            if priority < alpha_texture_priority {
                alpha_texture_priority = priority;
                alpha_texture_image = decoded.image();
            }
        }

        let priority =
            texture_slot_priority(name) + if is_alpha_texture_slot(name) { 20 } else { 0 };
        if priority >= main_texture_priority {
            continue;
        }
        main_texture_priority = priority;
        texture_width = json!(decoded.width);
        texture_height = json!(decoded.height);
        main_texture_image = decoded.image();
        if let Some(value) = vector2(tex_env.get("m_Scale")) {
            texture_scale = json!({ "x": value.0, "y": value.1 });
        }
        if let Some(value) = vector2(tex_env.get("m_Offset")) {
            texture_offset = json!({ "x": value.0, "y": value.1 });
        }
        if let Some(value) = vector2(tex_env.get("m_Pivot")) {
            texture_pivot = json!({ "x": value.0, "y": value.1 });
        }
        if let Some(value) = tex_env.get("m_Rotation").and_then(UnityValue::as_f64) {
            texture_rotation = json!(value);
        }
    }

    let cutoff = saved_float(saved, "_Cutoff");
    let src_blend = saved_float(saved, "_SrcBlend");
    let dst_blend = saved_float(saved, "_DstBlend");
    let z_write = saved_float(saved, "_ZWrite");
    let cull_mode = saved_float(saved, "_Cull");
    let material_name = object_name(&material);
    let initial_texture_has_alpha =
        main_texture_image.as_ref().is_some_and(image_has_alpha) || alpha_texture_image.is_some();
    let initial_texture_has_partial_alpha = main_texture_image
        .as_ref()
        .is_some_and(image_has_partial_alpha)
        || alpha_texture_image
            .as_ref()
            .is_some_and(image_has_partial_alpha);
    // Do not classify materials from the complete compiled shader program.
    // Ordinary GPU assembly contains opcodes such as `ADD`, which used to
    // make unrelated character and vehicle materials look additive. FusionFall
    // shader names and saved blend properties contain the useful render-state
    // hints without that noise.
    let shader_behavior = shader_name.clone();
    let alpha_mode = alpha_mode_hint(
        &material_name,
        &shader_behavior,
        initial_texture_has_alpha,
        material_alpha,
        cutoff,
        src_blend,
        dst_blend,
    );
    let blend_mode = blend_mode_hint(&shader_behavior, alpha_mode, src_blend, dst_blend);
    let z_write_value = z_write
        .map(|value| value >= 0.5)
        .or_else(|| shader_z_write_hint(&shader_behavior));
    let cull_off = shader_cull_off_hint(&shader_behavior);
    let (texture_has_alpha, texture_has_partial_alpha) = if let Some(mut image) = main_texture_image
    {
        if let Some(alpha_image) = alpha_texture_image.as_ref() {
            apply_alpha_mask(&mut image, alpha_image);
        }
        if should_apply_black_key_alpha(&material_name, &shader_behavior, alpha_mode) {
            apply_black_key_alpha(&mut image, alpha_mode == "additive");
        }
        repair_transparent_rgb(&mut image, 4);
        let texture_has_alpha = image_has_alpha(&image);
        let texture_has_partial_alpha = image_has_partial_alpha(&image);
        if let Some(data_url) = image_to_data_url(image, MATERIAL_TEXTURE_PREVIEW_MAX_SIZE, true) {
            texture_data_url = json!(data_url);
        }
        (texture_has_alpha, texture_has_partial_alpha)
    } else {
        (initial_texture_has_alpha, initial_texture_has_partial_alpha)
    };

    let preview = json!({
        "id": pointer_id(&key),
        "name": material_name,
        "color": color,
        "colorAlpha": material_alpha,
        "shaderName": if shader_name.is_empty() { JsonValue::Null } else { json!(shader_name) },
        "alphaMode": alpha_mode,
        "blendMode": blend_mode,
        "cutoff": cutoff,
        "zWrite": z_write_value,
        "cullMode": cull_mode,
        "cullOff": cull_off,
        "textureDataUrl": texture_data_url,
        "textureWidth": texture_width,
        "textureHeight": texture_height,
        "hasAlpha": texture_has_alpha,
        "hasPartialAlpha": texture_has_partial_alpha,
        "textureScale": texture_scale,
        "textureOffset": texture_offset,
        "texturePivot": texture_pivot,
        "textureRotation": texture_rotation,
        "textureTint": false,
    });
    cache.insert(key, preview.clone());
    Some(preview)
}
