use super::super::*;

pub(in super::super) fn texture_preview_from_key(
    env: &fusionforge::UnityEnvironment,
    asset_index: usize,
    path_id: i64,
    max_size: u32,
) -> Option<serde_json::Value> {
    let mut visited = BTreeSet::new();
    texture_preview_from_key_inner(env, asset_index, path_id, max_size, &mut visited)
}

pub(in super::super) fn texture_preview_from_key_inner(
    env: &fusionforge::UnityEnvironment,
    asset_index: usize,
    path_id: i64,
    max_size: u32,
    visited: &mut BTreeSet<(usize, i64)>,
) -> Option<serde_json::Value> {
    use fusionforge::{decode_texture, image_to_data_url, object_name, UnityValue};

    if !visited.insert((asset_index, path_id)) {
        return None;
    }
    if visited.len() > 128 {
        return None;
    }
    let asset = env.assets.get(asset_index)?;
    let info = asset.objects.get(&path_id)?;
    let object_type = asset.object_type_name(info);
    let body = asset.read_object(asset_index, info).ok()?;

    if object_type == "Texture2D" {
        let decoded = decode_texture(env, &body)?;
        let mut image = decoded.image()?;
        image::imageops::flip_vertical_in_place(&mut image);
        let data_url = image_to_data_url(image, max_size, false)?;
        return Some(json!({
            "asset": asset.name,
            "pathId": path_id,
            "name": object_name(&body),
            "objectType": object_type,
            "width": body.get("m_Width").and_then(UnityValue::as_i64),
            "height": body.get("m_Height").and_then(UnityValue::as_i64),
            "format": body.get("m_TextureFormat").and_then(UnityValue::as_i64),
            "dataUrl": data_url,
        }));
    }

    let mut pointers = Vec::new();
    collect_value_pointers(&body, &mut pointers);
    for pointer in pointers {
        let Ok(key) = env.resolve_pointer(&pointer) else {
            continue;
        };
        let Some(preview) =
            texture_preview_from_key_inner(env, key.asset, key.path_id, max_size, visited)
        else {
            continue;
        };
        return Some(preview);
    }
    None
}

#[derive(Clone)]
pub(in super::super) struct NifTextureCandidate {
    pub(in super::super) slot: String,
    pub(in super::super) path: String,
}

pub(in super::super) fn nif_texturing_property_texture_paths(
    nif: &nif::Nif,
    texturing: &nif::blocks::NiTexturingProperty,
) -> Vec<NifTextureCandidate> {
    let mut candidates = Vec::new();
    for (slot, tex_desc) in [
        ("base", texturing.base_texture.as_ref()),
        ("detail", texturing.detail_texture.as_ref()),
        ("decal0", texturing.decal0_texture.as_ref()),
        ("dark", texturing.dark_texture.as_ref()),
        ("gloss", texturing.gloss_texture.as_ref()),
        ("glow", texturing.glow_texture.as_ref()),
        ("bump", texturing.bump_map_texture.as_ref()),
    ] {
        if let Some(path) = nif_tex_desc_path(nif, tex_desc) {
            push_unique_nif_texture_candidate(&mut candidates, slot, path);
        }
    }
    for shader_texture in &texturing.shader_textures {
        if let Some(path) = nif_tex_desc_path(nif, shader_texture.map.as_ref()) {
            push_unique_nif_texture_candidate(&mut candidates, "shader", path);
        }
    }
    candidates
}

pub(in super::super) fn push_unique_nif_texture_candidate(
    candidates: &mut Vec<NifTextureCandidate>,
    slot: &str,
    path: String,
) {
    let normalized = normalized_asset_path(&path);
    if normalized.is_empty()
        || candidates
            .iter()
            .any(|candidate| normalized_asset_path(&candidate.path) == normalized)
    {
        return;
    }
    candidates.push(NifTextureCandidate {
        slot: slot.to_string(),
        path,
    });
}

pub(in super::super) fn normalized_nif_texture_name(path: &str) -> String {
    let mut value = path.trim().replace('\\', "/");
    for (from, to) in [
        (".tga", ".dds"),
        (".TGA", ".dds"),
        (".bmp", ".dds"),
        (".BMP", ".dds"),
    ] {
        value = value.replace(from, to);
    }
    value
}

pub(in super::super) fn texture_preview_for_nif_texture(
    env: &fusionforge::UnityEnvironment,
    texture_path: &str,
    black_key_alpha: bool,
) -> Option<JsonValue> {
    let candidates = nif_texture_candidate_paths(texture_path);
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let Ok(body) = asset.read_object(asset_index, info) else {
                continue;
            };
            for entry in fusionforge::value_array(body.get("m_Container")) {
                let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                    continue;
                };
                if !nif_texture_path_matches(&candidates, path) {
                    continue;
                }
                let Some(pointer) = metadata
                    .get("asset")
                    .and_then(fusionforge::UnityValue::as_pointer)
                else {
                    continue;
                };
                let Ok(key) = env.resolve_pointer(pointer) else {
                    continue;
                };
                if let Some(mut preview) =
                    nif_texture_preview_from_key(env, key.asset, key.path_id, 512, black_key_alpha)
                {
                    if let Some(object) = preview.as_object_mut() {
                        object.insert("containerPath".to_string(), json!(path));
                    }
                    return Some(preview);
                }
            }
        }
    }
    None
}

pub(in super::super) fn nif_texture_preview_from_key(
    env: &fusionforge::UnityEnvironment,
    asset_index: usize,
    path_id: i64,
    max_size: u32,
    black_key_alpha: bool,
) -> Option<serde_json::Value> {
    let mut visited = BTreeSet::new();
    nif_texture_preview_from_key_inner(
        env,
        asset_index,
        path_id,
        max_size,
        black_key_alpha,
        &mut visited,
    )
}

pub(in super::super) fn nif_texture_preview_from_key_inner(
    env: &fusionforge::UnityEnvironment,
    asset_index: usize,
    path_id: i64,
    max_size: u32,
    black_key_alpha: bool,
    visited: &mut BTreeSet<(usize, i64)>,
) -> Option<serde_json::Value> {
    use fusionforge::{decode_texture, image_to_data_url, object_name, UnityValue};

    if !visited.insert((asset_index, path_id)) || visited.len() > 128 {
        return None;
    }
    let asset = env.assets.get(asset_index)?;
    let info = asset.objects.get(&path_id)?;
    let object_type = asset.object_type_name(info);
    let body = asset.read_object(asset_index, info).ok()?;

    if object_type == "Texture2D" {
        let decoded = decode_texture(env, &body)?;
        let mut image = decoded.image()?;
        if black_key_alpha {
            apply_nif_black_key_alpha(&mut image);
        }
        let has_alpha = nif_image_has_alpha(&image);
        let has_partial_alpha = nif_image_has_partial_alpha(&image);
        image::imageops::flip_vertical_in_place(&mut image);
        let data_url = image_to_data_url(image, max_size, false)?;
        return Some(json!({
            "asset": asset.name,
            "pathId": path_id,
            "name": object_name(&body),
            "objectType": object_type,
            "width": body.get("m_Width").and_then(UnityValue::as_i64),
            "height": body.get("m_Height").and_then(UnityValue::as_i64),
            "format": body.get("m_TextureFormat").and_then(UnityValue::as_i64),
            "dataUrl": data_url,
            "hasAlpha": has_alpha,
            "hasPartialAlpha": has_partial_alpha,
        }));
    }

    let mut pointers = Vec::new();
    collect_value_pointers(&body, &mut pointers);
    for pointer in pointers {
        let Ok(key) = env.resolve_pointer(&pointer) else {
            continue;
        };
        let Some(preview) = nif_texture_preview_from_key_inner(
            env,
            key.asset,
            key.path_id,
            max_size,
            black_key_alpha,
            visited,
        ) else {
            continue;
        };
        return Some(preview);
    }
    None
}

pub(in super::super) fn nif_texture_candidate_paths(texture_path: &str) -> BTreeSet<String> {
    let normalized = normalized_asset_path(texture_path);
    let file_name = Path::new(&normalized)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&normalized)
        .to_string();
    let mut values = BTreeSet::new();
    values.insert(normalized.clone());
    values.insert(file_name.clone());
    values.insert(format!("texture/{file_name}"));
    values.insert(format!("textures/{file_name}"));
    values
}

pub(in super::super) fn nif_texture_path_matches(candidates: &BTreeSet<String>, container_path: &str) -> bool {
    let normalized = normalized_asset_path(container_path);
    if candidates.contains(&normalized) {
        return true;
    }
    let file_name = Path::new(&normalized)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&normalized);
    candidates.contains(file_name)
}

pub(in super::super) fn fill_icon_preview_from_texture_value(
    preview: &serde_json::Value,
    icon_path: &str,
    bundle_path: &str,
    container_path: &str,
) -> NpcIconPreview {
    NpcIconPreview {
        icon_path: icon_path.to_string(),
        bundle_path: bundle_path.to_string(),
        container_path: Some(container_path.to_string()),
        asset: json_string(preview, "asset"),
        path_id: preview.get("pathId").and_then(JsonValue::as_i64),
        name: json_string(preview, "name"),
        width: preview.get("width").and_then(JsonValue::as_i64),
        height: preview.get("height").and_then(JsonValue::as_i64),
        data_url: json_string(preview, "dataUrl"),
    }
}

pub(in super::super) fn preview_bundle_container_texture(
    bundle_path: String,
    project_dir: Option<String>,
    container_paths: Vec<String>,
) -> EditorResult<serde_json::Value> {
    let bundle = PathBuf::from(&bundle_path);
    let project = project_dir
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::repository_root().to_path_buf().join("work").join("fusionforge"));
    let extract_dir = if bundle.is_dir() {
        bundle.clone()
    } else {
        extract_bundle_cached(&project, &bundle)?
    };
    let env = fusionforge::UnityEnvironment::from_dir(&extract_dir);
    let wanted = container_paths
        .iter()
        .map(|path| normalized_asset_path(path))
        .filter(|path| !path.is_empty())
        .collect::<BTreeSet<_>>();

    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let body = asset.read_object(asset_index, info)?;
            let preload_table = fusionforge::value_array(body.get("m_PreloadTable")).to_vec();
            for entry in fusionforge::value_array(body.get("m_Container")) {
                let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                    continue;
                };
                let lower = normalized_asset_path(path);
                if !wanted.is_empty()
                    && !wanted.iter().any(|expected| {
                        lower == *expected || lower.ends_with(&format!("/{expected}"))
                    })
                {
                    continue;
                }

                if let Some(pointer) = metadata
                    .get("asset")
                    .and_then(fusionforge::UnityValue::as_pointer)
                {
                    if let Ok(key) = env.resolve_pointer(pointer) {
                        if let Some(mut preview) =
                            texture_preview_from_key(&env, key.asset, key.path_id, 512)
                        {
                            if let Some(object) = preview.as_object_mut() {
                                object.insert("containerPath".to_string(), json!(path));
                                object.insert("bundlePath".to_string(), json!(bundle_path));
                            }
                            return Ok(preview);
                        }
                    }
                }

                let (start, end) = metadata_preload_range(metadata, preload_table.len());
                for preload in &preload_table[start..end] {
                    let Some(pointer) = preload.as_pointer() else {
                        continue;
                    };
                    let Ok(key) = env.resolve_pointer(pointer) else {
                        continue;
                    };
                    if let Some(mut preview) =
                        texture_preview_from_key(&env, key.asset, key.path_id, 512)
                    {
                        if let Some(object) = preview.as_object_mut() {
                            object.insert("containerPath".to_string(), json!(path));
                            object.insert("bundlePath".to_string(), json!(bundle_path));
                        }
                        return Ok(preview);
                    }
                }
            }
        }
    }

    Ok(json!({
        "bundlePath": bundle_path,
        "containerPaths": container_paths,
        "dataUrl": null,
    }))
}

pub(in super::super) fn rename_assetbundle_texture_container_path(
    env: &fusionforge::UnityEnvironment,
    value: &mut fusionforge::UnityValue,
    texture_key: (usize, i64),
    target_texture_path: &str,
) -> usize {
    let mut renamed = 0usize;
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let Some(container) = object
        .get_mut("m_Container")
        .and_then(fusionforge::UnityValue::as_array_mut)
    else {
        return 0;
    };
    for entry in container {
        let Some((_path, metadata)) = fusionforge::pair_name_value(entry) else {
            continue;
        };
        let Some(pointer) = metadata
            .get("asset")
            .and_then(fusionforge::UnityValue::as_pointer)
        else {
            continue;
        };
        if pointer_candidate_keys(env, pointer).contains(&texture_key) {
            rename_pair_key(entry, target_texture_path);
            renamed += 1;
        }
    }
    renamed
}

pub(in super::super) fn material_main_texture_matches(
    env: &fusionforge::UnityEnvironment,
    value: &fusionforge::UnityValue,
    texture_key: (usize, i64),
) -> bool {
    let Some(saved) = value.get("m_SavedProperties") else {
        return false;
    };
    for entry in fusionforge::value_array(saved.get("m_TexEnvs")) {
        let Some((name, tex_env)) = fusionforge::pair_name_value(entry) else {
            continue;
        };
        if name != "_MainTex" {
            continue;
        }
        if tex_env
            .get("m_Texture")
            .is_some_and(|pointer| unity_value_points_to_key(env, pointer, texture_key))
        {
            return true;
        }
    }
    false
}

pub(in super::super) fn rewrite_texture_to_unity25_dxt3(value: &mut fusionforge::UnityValue) -> Result<(), String> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| "target texture is not an object".to_string())?;
    let width = object
        .get("m_Width")
        .and_then(fusionforge::UnityValue::as_i64)
        .ok_or_else(|| "texture has no m_Width".to_string())? as u32;
    let height = object
        .get("m_Height")
        .and_then(fusionforge::UnityValue::as_i64)
        .ok_or_else(|| "texture has no m_Height".to_string())? as u32;
    let rgba = object
        .get("image data")
        .and_then(fusionforge::UnityValue::as_bytes)
        .ok_or_else(|| "texture has no image data".to_string())?;
    let (dxt, mip_count) = encode_dxt3_rgba_mip_chain(width, height, rgba)?;
    object.insert(
        "m_TextureFormat".to_string(),
        fusionforge::UnityValue::Int(11),
    );
    object.insert(
        "image data".to_string(),
        fusionforge::UnityValue::Bytes(dxt.clone()),
    );
    object.insert(
        "m_CompleteImageSize".to_string(),
        fusionforge::UnityValue::Int(dxt.len() as i64),
    );
    object.insert("m_MipMap".to_string(), fusionforge::UnityValue::Bool(true));
    object.insert(
        "m_MipCount".to_string(),
        fusionforge::UnityValue::Int(mip_count as i64),
    );
    object.insert(
        "m_IsReadable".to_string(),
        fusionforge::UnityValue::Bool(false),
    );
    if let Some(stream_data) = object
        .get_mut("m_StreamData")
        .and_then(fusionforge::UnityValue::as_object_mut)
    {
        stream_data.insert("offset".to_string(), fusionforge::UnityValue::Int(0));
        stream_data.insert("size".to_string(), fusionforge::UnityValue::Int(0));
        stream_data.insert(
            "path".to_string(),
            fusionforge::UnityValue::String(String::new()),
        );
    }
    if let Some(settings) = object
        .get_mut("m_TextureSettings")
        .and_then(fusionforge::UnityValue::as_object_mut)
    {
        settings.insert("m_FilterMode".to_string(), fusionforge::UnityValue::Int(1));
        settings.insert("m_Aniso".to_string(), fusionforge::UnityValue::Int(1));
        settings.insert("m_MipBias".to_string(), fusionforge::UnityValue::Float(0.0));
    }
    if object.contains_key("m_Limit") {
        object.insert("m_Limit".to_string(), fusionforge::UnityValue::Int(-1));
    }
    Ok(())
}

pub(in super::super) fn npc_texture_token_allowed(token: &str) -> bool {
    !matches!(token, "fusion" | "character")
}

pub(in super::super) fn npc_texture_candidate_stems(
    hints: &NpcAssetHints,
    blueprint: &NpcBlueprint,
) -> BTreeSet<String> {
    let mut stems = hints
        .texture_paths
        .iter()
        .map(|path| npc_asset_basename(path))
        .filter(|stem| !stem.is_empty())
        .collect::<BTreeSet<_>>();
    for value in [
        blueprint.texture_asset.as_deref(),
        blueprint.model_asset.as_deref(),
        Some(blueprint.name.as_str()),
        Some(blueprint.internal_name.as_str()),
    ]
    .into_iter()
    .flatten()
    {
        let clean = npc_clean_asset_name(value).unwrap_or_else(|| value.replace('\\', "/"));
        let base = npc_asset_basename(&clean);
        if !base.is_empty() {
            stems.insert(base);
        }
        for token in npc_token_variants(value) {
            if (token.contains('_') || token.len() >= 5) && npc_texture_token_allowed(&token) {
                stems.insert(token);
            }
        }
    }
    stems
}

pub(in super::super) fn npc_texture_match_score(path: &str, candidate_stems: &BTreeSet<String>) -> usize {
    let normalized = normalized_asset_path(path);
    let basename = npc_asset_basename(&normalized);
    if basename.is_empty() {
        return 0;
    }
    let base_tokens = npc_token_variants(&basename)
        .into_iter()
        .filter(|token| npc_texture_token_allowed(token))
        .collect::<BTreeSet<_>>();
    let mut score = 0usize;
    for stem in candidate_stems {
        if stem.is_empty() {
            continue;
        }
        if basename == *stem {
            score = score.max(200);
            continue;
        }
        if basename.starts_with(&format!("{stem}_")) || stem.starts_with(&format!("{basename}_")) {
            score = score.max(120);
        }
        if base_tokens.contains(stem) {
            score = score.max(90);
        }
        let stem_tokens = npc_token_variants(stem)
            .into_iter()
            .filter(|token| npc_texture_token_allowed(token))
            .collect::<BTreeSet<_>>();
        let overlap = base_tokens.intersection(&stem_tokens).count();
        if overlap >= 2 {
            score = score.max(80);
        } else if overlap == 1
            && stem_tokens
                .iter()
                .any(|token| token.len() >= 5 && base_tokens.contains(token))
        {
            score = score.max(45);
        }
    }
    score
}

pub(in super::super) fn add_npc_texture_asset_hints_from_index(
    hints: &mut NpcAssetHints,
    blueprint: &NpcBlueprint,
    index: &ClientFileIndex,
) {
    let candidate_stems = npc_texture_candidate_stems(hints, blueprint);
    if candidate_stems.is_empty() {
        return;
    }

    let mut matches = Vec::<(usize, String)>::new();
    for bundle in &index.bundles {
        let bundle_name = bundle.name.to_ascii_lowercase();
        let prefer_texture_bundle =
            bundle_name.contains("texture") || bundle_name.contains("character_texture");
        for asset in &bundle.assets {
            for path in &asset.container_paths {
                let normalized = normalized_asset_path(path);
                if !normalized.starts_with("texture/") {
                    continue;
                }
                let already_known = hints.texture_paths.contains(&normalized);
                if !prefer_texture_bundle && !already_known {
                    continue;
                }
                let score = if already_known {
                    1_000
                } else {
                    npc_texture_match_score(&normalized, &candidate_stems)
                };
                if score > 0 {
                    matches.push((score, normalized));
                }
            }
        }
    }

    matches.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    matches.dedup_by(|left, right| left.1 == right.1);
    for (_, path) in matches.into_iter().take(12) {
        hints.exact_paths.insert(path.clone());
        hints.texture_paths.insert(path);
    }
}

pub(in super::super) fn npc_imported_texture_target_path(npc_id: i64, source_path: &str) -> String {
    format!(
        "texture/{}.dds",
        npc_imported_asset_unique_stem(npc_id, source_path)
    )
}

pub(in super::super) fn set_material_texenv_texture_pointer(
    value: &mut fusionforge::UnityValue,
    texenv_name: &str,
    pointer: fusionforge::UnityValue,
) {
    let Some(tex_envs) = value
        .get_mut("m_SavedProperties")
        .and_then(fusionforge::UnityValue::as_object_mut)
        .and_then(|saved| saved.get_mut("m_TexEnvs"))
        .and_then(fusionforge::UnityValue::as_array_mut)
    else {
        return;
    };
    let mut updated = false;
    for entry in tex_envs.iter_mut() {
        match entry {
            fusionforge::UnityValue::Pair(left, right) => {
                let name = material_texenv_key_name(left);
                if name == Some(texenv_name) {
                    if let Some(texture_slot) = right
                        .as_object_mut()
                        .and_then(|object| object.get_mut("m_Texture"))
                    {
                        *texture_slot = pointer.clone();
                        updated = true;
                    }
                }
            }
            fusionforge::UnityValue::Array(items) if items.len() >= 2 => {
                let name = items.first().and_then(material_texenv_key_name);
                if name == Some(texenv_name) {
                    if let Some(texture_slot) = items
                        .get_mut(1)
                        .and_then(fusionforge::UnityValue::as_object_mut)
                        .and_then(|object| object.get_mut("m_Texture"))
                    {
                        *texture_slot = pointer.clone();
                        updated = true;
                    }
                }
            }
            _ => {}
        }
    }
    if !updated && texenv_name == "_MainTex" {
        if let Some(first) = tex_envs.first_mut() {
            let slot = match first {
                fusionforge::UnityValue::Pair(_, right) => right
                    .as_object_mut()
                    .and_then(|object| object.get_mut("m_Texture")),
                fusionforge::UnityValue::Array(items) if items.len() >= 2 => items
                    .get_mut(1)
                    .and_then(fusionforge::UnityValue::as_object_mut)
                    .and_then(|object| object.get_mut("m_Texture")),
                _ => None,
            };
            if let Some(slot) = slot {
                *slot = pointer;
            }
        }
    }
}

pub(in super::super) fn npc_local_texture_output_pointers(
    env: &fusionforge::UnityEnvironment,
    selected: &BTreeSet<(usize, i64)>,
    externalized: &BTreeMap<(usize, i64), NpcTargetExternalRef>,
    output_path_ids: &BTreeMap<(usize, i64), i64>,
    object_name_overrides: &BTreeMap<(usize, i64), String>,
) -> BTreeMap<String, fusionforge::UnityValue> {
    let mut pointers = BTreeMap::new();
    let mut pointer_areas = BTreeMap::<String, i64>::new();
    for key in selected {
        if externalized.contains_key(key) {
            continue;
        }
        let Some(asset) = env.assets.get(key.0) else {
            continue;
        };
        let Some(info) = asset.objects.get(&key.1) else {
            continue;
        };
        if asset.object_type_name(info) != "Texture2D" {
            continue;
        }
        let body = asset.read_object(key.0, info).ok();
        let name = object_name_overrides
            .get(key)
            .cloned()
            .or_else(|| body.as_ref().map(fusionforge::object_name))
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let area = body
            .as_ref()
            .map(|body| {
                let width = body
                    .get("m_Width")
                    .and_then(fusionforge::UnityValue::as_i64);
                let height = body
                    .get("m_Height")
                    .and_then(fusionforge::UnityValue::as_i64);
                width.unwrap_or_default().max(0) * height.unwrap_or_default().max(0)
            })
            .unwrap_or_default();
        let normalized_name = normalized_asset_path(&name);
        if pointer_areas
            .get(&normalized_name)
            .is_some_and(|existing| *existing > area)
        {
            continue;
        }
        let output_path_id = output_path_ids.get(key).copied().unwrap_or(key.1);
        pointer_areas.insert(normalized_name.clone(), area);
        pointers.insert(
            normalized_name,
            fusionforge::UnityValue::Pointer(fusionforge::Pointer {
                source_asset: 0,
                file_id: 0,
                path_id: output_path_id,
            }),
        );
    }
    pointers
}
