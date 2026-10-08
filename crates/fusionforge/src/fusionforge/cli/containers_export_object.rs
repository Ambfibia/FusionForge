use super::*;

pub(super) fn cli_container_expected_export_types(path: &str) -> &'static [&'static str] {
    let normalized = cli_normalized_asset_path(path);
    if normalized.starts_with("texture/")
        || normalized.starts_with("icons/")
        || normalized.ends_with(".dds")
        || normalized.ends_with(".png")
        || normalized.ends_with(".tga")
        || normalized.ends_with(".jpg")
        || normalized.ends_with(".psd")
    {
        &["Texture2D"]
    } else if normalized.starts_with("sound/")
        || normalized.ends_with(".wav")
        || normalized.ends_with(".mp3")
        || normalized.ends_with(".ogg")
    {
        &["AudioClip"]
    } else if normalized.starts_with("mob/")
        || normalized.ends_with(".kfm")
        || normalized.ends_with(".nif")
    {
        &["TextAsset"]
    } else {
        &[]
    }
}

pub(super) fn export_object(
    env: &UnityEnvironment,
    asset_index: usize,
    path_id: i64,
    info: &ObjectInfo,
    options: &ExtractOptions,
    forced_path: Option<&Path>,
) -> Result<(), String> {
    let asset = &env.assets[asset_index];
    let obj_type = asset.object_type_name(info);
    if !options.types.contains(&obj_type) {
        return Ok(());
    }
    let body = asset.read_object(asset_index, info)?;
    let name = object_name(&body);
    if !options.filters.is_empty()
        && !options
            .filters
            .iter()
            .any(|filter| name.to_lowercase().contains(filter))
    {
        return Ok(());
    }

    match obj_type.as_str() {
        "Mesh" => {
            if let Some(obj) = mesh_to_obj(&body) {
                let path = forced_path.map(Path::to_path_buf).unwrap_or_else(|| {
                    options
                        .outdir
                        .join(format!("{}.obj", safe_name(&name, path_id)))
                });
                write_output(&path, obj.as_bytes(), options.dry_run)?;
            }
        }
        "Texture2D" => {
            if let Some(texture) = decode_texture(env, &body).and_then(|texture| texture.image()) {
                let mut image = texture;
                image::imageops::flip_vertical_in_place(&mut image);
                let path = forced_path.map(Path::to_path_buf).unwrap_or_else(|| {
                    options
                        .outdir
                        .join(format!("{}.png", safe_name(&name, path_id)))
                });
                write_png_output(&path, &image, options.dry_run)?;
            }
        }
        "TextAsset" | "Shader" => {
            let data = body
                .get("m_Script")
                .and_then(value_to_bytes)
                .unwrap_or_default();
            let ext = if obj_type == "Shader" {
                "cg"
            } else if String::from_utf8(data.clone()).is_ok() {
                "txt"
            } else {
                "bin"
            };
            let path = forced_path.map(Path::to_path_buf).unwrap_or_else(|| {
                options
                    .outdir
                    .join(format!("{}.{}", safe_name(&name, path_id), ext))
            });
            write_output(&path, &data, options.dry_run)?;
        }
        "Font" => {
            let data = body
                .get("m_FontData")
                .and_then(value_to_bytes)
                .unwrap_or_default();
            let path = forced_path.map(Path::to_path_buf).unwrap_or_else(|| {
                options
                    .outdir
                    .join(format!("{}.ttf", safe_name(&name, path_id)))
            });
            write_output(&path, &data, options.dry_run)?;
        }
        "MovieTexture" => {
            let data = body
                .get("m_MovieData")
                .and_then(value_to_bytes)
                .unwrap_or_default();
            let path = forced_path.map(Path::to_path_buf).unwrap_or_else(|| {
                options
                    .outdir
                    .join(format!("{}.ogv", safe_name(&name, path_id)))
            });
            write_output(&path, &data, options.dry_run)?;
        }
        "AudioClip" => {
            let data = body
                .get("audio data")
                .and_then(value_to_bytes)
                .or_else(|| body.get("m_AudioData").and_then(value_to_bytes))
                .unwrap_or_default();
            if !data.is_empty() {
                let path = forced_path.map(Path::to_path_buf).unwrap_or_else(|| {
                    options
                        .outdir
                        .join(format!("{}.ogg", safe_name(&name, path_id)))
                });
                write_output(&path, &data, options.dry_run)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn container_entries(body: &UnityValue) -> Vec<(String, &UnityValue)> {
    value_array(body.get("m_Container"))
        .iter()
        .filter_map(|entry| {
            pair_name_value(entry).map(|(path, metadata)| (path.to_string(), metadata))
        })
        .collect()
}

pub(super) fn resolved_object_info<'a>(
    env: &'a UnityEnvironment,
    pointer: &Pointer,
) -> Option<(&'a Asset, &'a ObjectInfo)> {
    let key = env.resolve_pointer(pointer).ok()?;
    let asset = env.assets.get(key.asset)?;
    let info = asset.objects.get(&key.path_id)?;
    Some((asset, info))
}

pub(super) fn find_object(
    env: &UnityEnvironment,
    path_id: i64,
) -> Result<(usize, &Asset, &ObjectInfo), String> {
    for (asset_index, asset) in env.assets.iter().enumerate() {
        if let Some(info) = asset.objects.get(&path_id) {
            return Ok((asset_index, asset, info));
        }
    }
    Err(format!("path id not found: {path_id}"))
}

pub(crate) fn unity_to_json(value: &UnityValue) -> JsonValue {
    match value {
        UnityValue::Bool(value) => json!(value),
        UnityValue::Int(value) => json!(value),
        UnityValue::UInt(value) => json!(value),
        UnityValue::Float(value) => json!(value),
        UnityValue::String(value) => json!(value),
        UnityValue::Bytes(value) => {
            json!({ "bytes": value.len(), "base64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, value) })
        }
        UnityValue::Array(values) => JsonValue::Array(values.iter().map(unity_to_json).collect()),
        UnityValue::Object(values) => JsonValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), unity_to_json(value)))
                .collect(),
        ),
        UnityValue::Pair(left, right) => json!([unity_to_json(left), unity_to_json(right)]),
        UnityValue::Pointer(pointer) => {
            json!({ "fileId": pointer.file_id, "pathId": pointer.path_id })
        }
    }
}
