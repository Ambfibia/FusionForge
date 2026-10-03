use super::super::*;

pub(in super::super) fn imported_material_preview_id(source_path: &Path, material_index: usize) -> String {
    format!(
        "authoring:{}:material:{material_index}",
        source_path.to_string_lossy()
    )
}

pub(in super::super) fn imported_material_color_hex(color: [f64; 4]) -> String {
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(color[0]),
        channel(color[1]),
        channel(color[2])
    )
}

pub(in super::super) fn imported_material_to_preview(
    material: &fusionforge::modding::ImportedMaterial,
    source_path: &Path,
) -> JsonValue {
    let mut texture_data_url = JsonValue::Null;
    let mut texture_width = JsonValue::Null;
    let mut texture_height = JsonValue::Null;
    let mut has_alpha = false;
    let mut has_partial_alpha = false;
    if let Some(texture) = material.base_color_texture.as_ref() {
        has_alpha = texture.rgba.chunks_exact(4).any(|pixel| pixel[3] < 255);
        has_partial_alpha = texture
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0 && pixel[3] < 255);
        if let Some(mut image) =
            image::RgbaImage::from_raw(texture.width, texture.height, texture.rgba.clone())
        {
            // ImportedTexture stores pixels in the vertically-flipped Unity
            // layout. The decoded preview is exposed as a regular PNG data URL.
            image::imageops::flip_vertical_in_place(&mut image);
            if let Some(data_url) = fusionforge::image_to_data_url(image, 512, false) {
                texture_data_url = json!(data_url);
                texture_width = json!(texture.width);
                texture_height = json!(texture.height);
            }
        }
    }
    let alpha_mode = match material.alpha_mode {
        fusionforge::modding::ImportedMaterialAlphaMode::Opaque => "opaque",
        fusionforge::modding::ImportedMaterialAlphaMode::Mask => "cutout",
        fusionforge::modding::ImportedMaterialAlphaMode::Blend => "transparent",
    };
    let blend_mode =
        if material.alpha_mode == fusionforge::modding::ImportedMaterialAlphaMode::Blend {
            "alpha"
        } else {
            "opaque"
        };
    json!({
        "id": imported_material_preview_id(source_path, material.index),
        "sourceMaterialIndex": material.index,
        "name": material.name,
        "color": imported_material_color_hex(material.base_color),
        "colorAlpha": material.base_color[3],
        "shaderName": "GLTF/PBR",
        "alphaMode": alpha_mode,
        "blendMode": blend_mode,
        "cutoff": material.alpha_cutoff,
        "zWrite": material.alpha_mode != fusionforge::modding::ImportedMaterialAlphaMode::Blend,
        "cullMode": if material.double_sided { 0 } else { 2 },
        "cullOff": material.double_sided,
        "textureDataUrl": texture_data_url,
        "textureWidth": texture_width,
        "textureHeight": texture_height,
        "hasAlpha": has_alpha,
        "hasPartialAlpha": has_partial_alpha,
        "textureScale": { "x": 1.0, "y": 1.0 },
        "textureOffset": { "x": 0.0, "y": 0.0 },
        "texturePivot": { "x": 0.0, "y": 0.0 },
        "textureRotation": 0.0,
        "textureTint": true,
        "metallic": material.metallic,
        "roughness": material.roughness,
    })
}

pub(in super::super) fn preload_material_previews(
    env: &fusionforge::UnityEnvironment,
    preload_table: &[fusionforge::UnityValue],
    start: usize,
    end: usize,
) -> Vec<JsonValue> {
    let mut cache = HashMap::<(String, i64), JsonValue>::new();
    let mut seen = BTreeSet::<(usize, i64)>::new();
    let mut materials = Vec::new();

    for preload in &preload_table[start..end] {
        let Some(pointer) = preload.as_pointer() else {
            continue;
        };
        let Ok(key) = env.resolve_pointer(pointer) else {
            continue;
        };
        if !seen.insert((key.asset, key.path_id)) {
            continue;
        }
        let Some(asset) = env.assets.get(key.asset) else {
            continue;
        };
        let Some(info) = asset.objects.get(&key.path_id) else {
            continue;
        };
        if asset.object_type_name(info) != "Material" {
            continue;
        }
        if let Some(material) = fusionforge::material_preview(env, pointer, &mut cache) {
            materials.push(material);
        }
    }

    materials
}

pub(in super::super) fn material_exact_name_key(material: &JsonValue) -> Option<String> {
    let value = material.get("name").and_then(JsonValue::as_str)?;
    let normalized = value.trim().trim_matches('"').replace('\\', "/");
    (!normalized.is_empty()).then(|| normalized.to_ascii_lowercase())
}

pub(in super::super) fn preview_material_semantic_value(value: Option<&JsonValue>) -> JsonValue {
    preview_value_without_fields(value, &["id"])
}

pub(in super::super) fn preview_material_reference_semantic(
    value: Option<&JsonValue>,
    materials: &BTreeMap<String, JsonValue>,
) -> JsonValue {
    let Some(value) = value else {
        return JsonValue::Null;
    };
    let Some(id) = value.as_str() else {
        return value.clone();
    };
    materials
        .get(id)
        .map(|material| preview_material_semantic_value(Some(material)))
        .unwrap_or_else(|| json!({ "unresolvedMaterialId": id }))
}

pub(in super::super) fn preview_material_references_semantic(
    mesh: &JsonValue,
    field: &str,
    materials: &BTreeMap<String, JsonValue>,
) -> JsonValue {
    mesh.get(field)
        .and_then(JsonValue::as_array)
        .map(|values| {
            JsonValue::Array(
                values
                    .iter()
                    .map(|value| preview_material_reference_semantic(Some(value), materials))
                    .collect(),
            )
        })
        .unwrap_or(JsonValue::Null)
}

pub(in super::super) fn preview_material_dependency_refs(
    env: &fusionforge::UnityEnvironment,
    wanted: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    let tokens = preview_name_tokens(wanted, &[]);
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            let object_type = asset.object_type_name(info);
            let Ok(body) = asset.read_object(asset_index, info) else {
                continue;
            };

            if object_type == "Material" {
                if !preview_named_value_matches_tokens(&body, &tokens) {
                    continue;
                }
                if let Some(pointer) = body
                    .get("m_Shader")
                    .and_then(fusionforge::UnityValue::as_pointer)
                {
                    preview_add_external_pointer_ref(asset, pointer, &mut refs);
                }

                if let Some(saved) = body.get("m_SavedProperties") {
                    for entry in fusionforge::value_array(saved.get("m_TexEnvs")) {
                        let Some((_, tex_env)) = fusionforge::pair_name_value(entry) else {
                            continue;
                        };
                        let Some(pointer) = tex_env
                            .get("m_Texture")
                            .and_then(fusionforge::UnityValue::as_pointer)
                        else {
                            continue;
                        };
                        preview_add_external_pointer_ref(asset, pointer, &mut refs);
                    }
                }
            }

            if matches!(object_type.as_str(), "MeshRenderer" | "SkinnedMeshRenderer") {
                if !preview_renderer_matches_tokens(env, &body, &tokens) {
                    continue;
                }
                for pointer in fusionforge::value_array(body.get("m_Materials"))
                    .iter()
                    .filter_map(fusionforge::UnityValue::as_pointer)
                {
                    preview_add_external_pointer_ref(asset, pointer, &mut refs);
                }
            }
        }
    }
    refs
}

pub(in super::super) fn material_name_suggests_glass(value: &fusionforge::UnityValue) -> bool {
    fusionforge::object_name(value)
        .to_ascii_lowercase()
        .contains("glass")
}

pub(in super::super) fn retarget_imported_material(value: &mut fusionforge::UnityValue, material_name: &str) {
    set_unity_object_string(value, "m_Name", material_name);
    let Some(saved) = value
        .get_mut("m_SavedProperties")
        .and_then(fusionforge::UnityValue::as_object_mut)
    else {
        return;
    };
    if let Some(colors) = saved
        .get_mut("m_Colors")
        .and_then(fusionforge::UnityValue::as_array_mut)
    {
        for entry in colors {
            let Some((name, _)) = fusionforge::pair_name_value(entry) else {
                continue;
            };
            if name == "_Color" {
                replace_pair_value(entry, unity_color(1.0, 1.0, 1.0, 1.0));
            } else if name == "_Emission" || name == "_AmbColor" {
                replace_pair_value(entry, unity_color(0.5, 0.5, 0.5, 1.0));
            }
        }
    }
}

pub(in super::super) fn renderer_uses_material_key(
    env: &fusionforge::UnityEnvironment,
    value: &fusionforge::UnityValue,
    material_key: (usize, i64),
) -> bool {
    fusionforge::value_array(value.get("m_Materials"))
        .iter()
        .any(|pointer| unity_value_points_to_key(env, pointer, material_key))
}

pub(in super::super) const NPC_SHARED_SHADER_PATH_ID: i64 = 1610;

pub(in super::super) fn clean_material(
    asset: &fusionforge::Asset,
    name: &str,
    texture_path_id: i64,
    shader_file_id: i32,
    shader_path_id: i64,
    shader_map_pointer: Option<(i32, i64)>,
    imported_material: Option<&fusionforge::modding::ImportedMaterial>,
) -> Result<fusionforge::UnityValue, String> {
    let mut value = asset.empty_object_value_for_class(21)?;
    set_unity_object_string(&mut value, "m_Name", name);
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "m_Shader".to_string(),
            unity_external_pointer(shader_file_id, shader_path_id),
        );
        let main_tex_env = fusionforge::UnityValue::Object(BTreeMap::from([
            (
                "m_Texture".to_string(),
                unity_local_pointer(0, texture_path_id),
            ),
            ("m_Scale".to_string(), unity_vec2(1.0, 1.0)),
            ("m_Offset".to_string(), unity_vec2(0.0, 0.0)),
            ("m_Pivot".to_string(), unity_vec2(0.0, 0.0)),
            (
                "m_Rotation".to_string(),
                fusionforge::UnityValue::Float(0.0),
            ),
        ]));
        // Original NPC materials bind the shared toon ramp (ToonRamp9) as
        // _ShaderMap; without it the toon shader has no light ramp.
        let shader_map_env = fusionforge::UnityValue::Object(BTreeMap::from([
            (
                "m_Texture".to_string(),
                shader_map_pointer
                    .map(|(file_id, path_id)| unity_external_pointer(file_id, path_id))
                    .unwrap_or_else(unity_null_pointer),
            ),
            ("m_Scale".to_string(), unity_vec2(1.0, 1.0)),
            ("m_Offset".to_string(), unity_vec2(0.0, 0.0)),
            ("m_Pivot".to_string(), unity_vec2(0.0, 0.0)),
            (
                "m_Rotation".to_string(),
                fusionforge::UnityValue::Float(0.0),
            ),
        ]));
        let spec_map_env = fusionforge::UnityValue::Object(BTreeMap::from([
            ("m_Texture".to_string(), unity_local_pointer(0, 0)),
            ("m_Scale".to_string(), unity_vec2(1.0, 1.0)),
            ("m_Offset".to_string(), unity_vec2(0.0, 0.0)),
            ("m_Pivot".to_string(), unity_vec2(0.0, 0.0)),
            (
                "m_Rotation".to_string(),
                fusionforge::UnityValue::Float(0.0),
            ),
        ]));
        let mut floats = vec![unity_pair(
            unity_named("_Outline"),
            fusionforge::UnityValue::Float(0.005),
        )];
        if let Some(cutoff) = imported_material.and_then(|material| material.alpha_cutoff) {
            floats.push(unity_pair(
                unity_named("_Cutoff"),
                fusionforge::UnityValue::Float(cutoff),
            ));
        }
        let (color, emission) = imported_material
            .map(|material| (material.base_color, material.emissive))
            .unwrap_or((
                [0.5, 0.5, 0.5, 1.0],
                [0.235_294_133, 0.235_294_133, 0.235_294_133],
            ));
        object.insert(
            "m_SavedProperties".to_string(),
            fusionforge::UnityValue::Object(BTreeMap::from([
                (
                    "m_TexEnvs".to_string(),
                    fusionforge::UnityValue::Array(vec![
                        unity_pair(unity_named("_MainTex"), main_tex_env),
                        unity_pair(unity_named("_ShaderMap"), shader_map_env),
                        unity_pair(unity_named("_SpecMap"), spec_map_env),
                    ]),
                ),
                (
                    "m_Floats".to_string(),
                    fusionforge::UnityValue::Array(floats),
                ),
                (
                    "m_Colors".to_string(),
                    // Preserve authored GLTF tint/alpha and emission. For old
                    // OBJ imports without material metadata, retain the
                    // original FusionFall NPC defaults.
                    fusionforge::UnityValue::Array(vec![
                        unity_pair(
                            unity_named("_Color"),
                            unity_color(color[0], color[1], color[2], color[3]),
                        ),
                        unity_pair(
                            unity_named("_Emission"),
                            unity_color(emission[0], emission[1], emission[2], 1.0),
                        ),
                        unity_pair(
                            unity_named("_OutlineColor"),
                            unity_color(0.0, 0.0, 0.0, 1.0),
                        ),
                        unity_pair(unity_named("_AmbColor"), unity_color(0.0, 0.0, 0.0, 1.0)),
                    ]),
                ),
            ])),
        );
    }
    Ok(value)
}

/// Finds the shared skinned toon shader (used by every original world NPC
/// material) and the shared toon ramp texture (ToonRamp9, bound as
/// _ShaderMap by original NPC materials) inside the given bundle.
/// Toon shader (and optional ToonRamp9 texture) decoded from a source client
/// bundle so they can be inlined as local objects in a generated NPC bundle.
pub(in super::super) struct NpcInlineToonShaderAssets {
    pub(in super::super) shader_value: fusionforge::UnityValue,
    pub(in super::super) shader_type_tree: fusionforge::TypeTree,
    pub(in super::super) ramp_value: Option<fusionforge::UnityValue>,
}

/// Reads the toon shader and ToonRamp9 texture out of `bundle_path`. The
/// shader is chosen by the first matching name in `shader_name_priority`
/// (lowercase names). The decoded values are meant to be re-serialized into
/// the generated NPC bundle as fully local objects, so the runtime never has
/// to resolve them across bundles.
pub(in super::super) fn find_npc_inline_toon_shader_assets(
    bundle_path: &Path,
    shader_name_priority: &[&str],
) -> Result<NpcInlineToonShaderAssets, String> {
    let temp = native_build_temp_dir("npc_shader_lookup")?;
    extract_bundle_native_to_dir(bundle_path, temp.path())?;
    let mut shader_by_name =
        BTreeMap::<String, (fusionforge::UnityValue, fusionforge::TypeTree)>::new();
    let mut ramp_value = None;
    for file in extracted_files_in_dir(temp.path()) {
        let path = PathBuf::from(&file.path);
        if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.ends_with(".json"))
        {
            continue;
        }
        let Ok(asset) = fusionforge::Asset::from_path(&path) else {
            continue;
        };
        for info in asset.objects.values() {
            if info.class_id == 48 {
                if let Ok(body) = asset.read_object(0, info) {
                    if let Some(name) = npc_shader_name_from_body(&body) {
                        if !shader_by_name.contains_key(&name) {
                            if let Some(tree) = asset
                                .tree
                                .type_trees
                                .get(&unity_object_type_tree_key(info))
                                .or_else(|| asset.tree.type_trees.get(&info.class_id))
                            {
                                shader_by_name.insert(name, (body, tree.clone()));
                            }
                        }
                    }
                }
            }
            if ramp_value.is_none() && info.class_id == 28 {
                if let Ok(body) = asset.read_object(0, info) {
                    let name = fusionforge::object_name(&body).to_ascii_lowercase();
                    if name == "toonramp9" || name.starts_with("toonramp9.") {
                        ramp_value = Some(body);
                    }
                }
            }
        }
    }
    let (shader_value, shader_type_tree) = shader_name_priority
        .iter()
        .find_map(|name| shader_by_name.remove(*name))
        .ok_or_else(|| {
            format!(
                "{}: none of the shaders [{}] were found",
                bundle_path.display(),
                shader_name_priority.join(", ")
            )
        })?;
    Ok(NpcInlineToonShaderAssets {
        shader_value,
        shader_type_tree,
        ramp_value,
    })
}

pub(in super::super) fn npc_shader_name_from_body(body: &fusionforge::UnityValue) -> Option<String> {
    let bytes = body.get("m_Script").and_then(unity_value_owned_bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let marker = "Shader \"";
    let start = text.find(marker)? + marker.len();
    let rest = text.get(start..)?;
    let end = rest.find('"')?;
    Some(rest[..end].trim().to_ascii_lowercase())
}

/// Shader used by every original world NPC/mob skinned material
/// (CharacterCreation #665 in the base build).
pub(in super::super) const NPC_SKINNED_TOON_SHADER_NAME: &str = "skinnedtoonshading_blendsrcalphainvsrcalpha";

/// Static toon shader that the working transferred NPC bundles
/// (Character_Rex, Character_Johnny_Test) reference from the world-shared
/// Tutorial bundle (#1610); it is resolvable while world NPCs spawn.
pub(in super::super) const NPC_WORLD_TOON_SHADER_NAME: &str = "toonshading_blendsrcalphainvsrcalpha";

pub(in super::super) fn default_npc_shared_shader_ref(
    target_index: &NpcTargetExternalIndex,
) -> Option<NpcTargetExternalRef> {
    // Original world NPC materials all point at the *skinned* toon shader
    // (SkinnedToonShading_blendSrcalphaInvsrcalpha in CharacterCreation), not
    // at the static ToonShading variant in the shared bundle.
    target_index
        .shader_by_name
        .get(NPC_SKINNED_TOON_SHADER_NAME)
        .cloned()
        .or_else(|| {
            target_index
                .shader_by_name
                .values()
                .find(|reference| reference.path_id == NPC_SHARED_SHADER_PATH_ID)
                .cloned()
        })
        .or_else(|| target_index.shader_by_name.values().next().cloned())
}

/// Returns true when a packed material's m_Shader pointer does not resolve to
/// a shader we know is valid in the output bundle (an externalized target
/// shader or a shader object kept locally). Only such materials should fall
/// back to the default shared NPC shader; a successfully externalized shader
/// (matched by fingerprint or name) must be preserved so shader variants
/// (fusion goo, alpha, bump) keep working.
pub(in super::super) fn material_shader_needs_default(
    value: &fusionforge::UnityValue,
    valid_shader_pointers: &BTreeSet<(i32, i64)>,
) -> bool {
    let Some(pointer) = value
        .get("m_Shader")
        .and_then(fusionforge::UnityValue::as_pointer)
    else {
        return true;
    };
    if pointer.is_null() {
        return true;
    }
    !valid_shader_pointers.contains(&(pointer.file_id, pointer.path_id))
}

pub(in super::super) fn material_texenv_key_name(value: &fusionforge::UnityValue) -> Option<&str> {
    value.as_str().or_else(|| {
        value
            .as_object()
            .and_then(|object| object.get("name"))
            .and_then(fusionforge::UnityValue::as_str)
    })
}

/// Nulls only texture-env pointers that no longer resolve inside the output
/// bundle (dangling local references). Resolved pointers - including
/// _ShaderMap toon ramps and external references - are preserved: original
/// NPC materials rely on _ShaderMap -> ToonRamp9 for toon shading.
pub(in super::super) fn clear_dangling_material_texenv_pointers(
    value: &mut fusionforge::UnityValue,
    local_output_path_ids: &BTreeSet<i64>,
    valid_external_file_ids: &BTreeSet<i32>,
) {
    let Some(tex_envs) = value
        .get_mut("m_SavedProperties")
        .and_then(fusionforge::UnityValue::as_object_mut)
        .and_then(|saved| saved.get_mut("m_TexEnvs"))
        .and_then(fusionforge::UnityValue::as_array_mut)
    else {
        return;
    };
    for entry in tex_envs {
        let slot = match entry {
            fusionforge::UnityValue::Pair(_, right) => right
                .as_object_mut()
                .and_then(|object| object.get_mut("m_Texture")),
            fusionforge::UnityValue::Array(items) if items.len() >= 2 => items
                .get_mut(1)
                .and_then(fusionforge::UnityValue::as_object_mut)
                .and_then(|object| object.get_mut("m_Texture")),
            _ => None,
        };
        let Some(slot) = slot else {
            continue;
        };
        let dangling = slot.as_pointer().is_some_and(|pointer| {
            pointer.path_id != 0
                && ((pointer.file_id == 0 && !local_output_path_ids.contains(&pointer.path_id))
                    || (pointer.file_id != 0
                        && !valid_external_file_ids.contains(&pointer.file_id)))
        });
        if dangling {
            *slot = unity_null_pointer();
        }
    }
}

pub(in super::super) fn normalize_imported_material_output(
    value: &mut fusionforge::UnityValue,
    shader_ref: Option<(&NpcTargetExternalRef, i32)>,
    local_output_path_ids: &BTreeSet<i64>,
    valid_external_file_ids: &BTreeSet<i32>,
    main_texture_pointer: Option<fusionforge::UnityValue>,
) {
    let material_name = fusionforge::object_name(value);
    // Keep the source material's saved color properties (_Color, _Emission,
    // _AmbColor) untouched: shaders like Skin_DirLight_AmbLight_* read them,
    // and overwriting them with flat values makes imported NPCs look washed
    // out compared to original NPCs. Only the name is normalized here.
    set_unity_object_string(value, "m_Name", &material_name);
    if let Some((reference, file_id)) = shader_ref {
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "m_Shader".to_string(),
                fusionforge::UnityValue::Pointer(fusionforge::Pointer {
                    source_asset: 0,
                    file_id,
                    path_id: reference.path_id,
                }),
            );
        }
    }
    clear_dangling_material_texenv_pointers(value, local_output_path_ids, valid_external_file_ids);
    if let Some(pointer) = main_texture_pointer {
        let lower_material_name = normalized_asset_path(&material_name);
        if material_texenv_pointer_is_null(value, "_MainTex")
            || lower_material_name.contains("main-link")
            || lower_material_name.contains("sub-link")
        {
            set_material_texenv_texture_pointer(value, "_MainTex", pointer);
        }
    }
}

pub(in super::super) fn material_texenv_pointer_is_null(value: &fusionforge::UnityValue, texenv_name: &str) -> bool {
    let Some(tex_envs) = value
        .get("m_SavedProperties")
        .and_then(fusionforge::UnityValue::as_object)
        .and_then(|saved| saved.get("m_TexEnvs"))
        .and_then(fusionforge::UnityValue::as_array)
    else {
        return false;
    };
    for entry in tex_envs {
        let (name, texture) = match entry {
            fusionforge::UnityValue::Pair(left, right) => {
                let name = material_texenv_key_name(left);
                let texture = right.as_object().and_then(|object| object.get("m_Texture"));
                (name, texture)
            }
            fusionforge::UnityValue::Array(items) if items.len() >= 2 => {
                let name = items.first().and_then(material_texenv_key_name);
                let texture = items
                    .get(1)
                    .and_then(fusionforge::UnityValue::as_object)
                    .and_then(|object| object.get("m_Texture"));
                (name, texture)
            }
            _ => (None, None),
        };
        if name == Some(texenv_name) {
            return texture.is_some_and(|texture| {
                texture
                    .as_pointer()
                    .is_some_and(fusionforge::Pointer::is_null)
            });
        }
    }
    false
}
