use super::*;

#[derive(Debug, Default)]
pub(crate) struct ExactMaterialExport {
    /// Material slots, including explicit JSON nulls, keyed by renderer identity.
    pub mesh_materials: BTreeMap<(usize, i64), Vec<JsonValue>>,
    /// Unique resolved Material objects keyed by their stable source id.
    pub materials: BTreeMap<String, JsonValue>,
    /// Unique resolved Texture2D objects keyed by their stable source id.
    pub textures: BTreeMap<String, JsonValue>,
    /// Source renderer provenance. This is separate from mesh material ids so
    /// repeated/null slots are not erased by the unique-material map.
    pub renderer_bindings: Vec<JsonValue>,
    /// Mesh ids whose renderers use different ordered material slots. For
    /// these meshes renderer bindings are authoritative.
    pub non_uniform_mesh_materials: Vec<String>,
}

pub(super) fn exact_material(
    env: &UnityEnvironment,
    key: ObjectKey,
    textures: &mut BTreeMap<String, JsonValue>,
) -> Result<JsonValue, String> {
    ensure_object_type(env, key, "Material", "material")?;
    let material = env.read_object(key)?;
    let saved = material
        .get("m_SavedProperties")
        .ok_or_else(|| format!("{} has no m_SavedProperties", object_id(env, key)))?;
    let saved_object = saved
        .as_object()
        .ok_or_else(|| format!("{}.m_SavedProperties is not an object", object_id(env, key)))?;

    let colors = exact_named_pairs(saved.get("m_Colors"), "m_Colors")?;
    let floats = exact_named_pairs(saved.get("m_Floats"), "m_Floats")?;
    let mut texture_envs = Vec::new();
    for (slot_index, entry) in value_array(saved.get("m_TexEnvs")).iter().enumerate() {
        let (name, tex_env) = pair_name_value(entry).ok_or_else(|| {
            format!(
                "{}.m_SavedProperties.m_TexEnvs[{}] is not a named pair",
                object_id(env, key),
                slot_index
            )
        })?;
        let texture_value = tex_env.get("m_Texture").ok_or_else(|| {
            format!(
                "{}.m_SavedProperties.m_TexEnvs[{}] has no m_Texture",
                object_id(env, key),
                slot_index
            )
        })?;
        let pointer = match texture_value {
            UnityValue::Pointer(pointer) => pointer,
            _ => {
                return Err(format!(
                    "{}.m_SavedProperties.m_TexEnvs[{}].m_Texture is not a PPtr",
                    object_id(env, key),
                    slot_index
                ));
            }
        };
        let serialized_name_is_invalid =
            name.trim().is_empty() || name.len() > 1_024 || name.chars().any(char::is_control);
        let unassigned_slot = serialized_name_is_invalid && pointer.is_null();
        let (texture_id, texture_source, dynamic_texture) = if pointer.is_null() || unassigned_slot
        {
            (JsonValue::Null, JsonValue::Null, JsonValue::Null)
        } else {
            let texture_key = strict_non_null_pointer(env, pointer).map_err(|error| {
                format!(
                    "{} texture slot '{}' ({}/{}) could not be resolved: {}",
                    object_id(env, key),
                    name,
                    pointer.file_id,
                    pointer.path_id,
                    error
                )
            })?;
            let object_type = env.assets[texture_key.asset]
                .objects
                .get(&texture_key.path_id)
                .map(|info| env.assets[texture_key.asset].object_type_name(info))
                .ok_or_else(|| {
                    format!(
                        "material texture {} is missing",
                        object_id(env, texture_key)
                    )
                })?;
            match object_type.as_str() {
                "Texture2D" => {
                    let id = object_id(env, texture_key);
                    if !textures.contains_key(&id) {
                        textures.insert(id.clone(), exact_texture(env, texture_key)?);
                    }
                    (
                        json!(id),
                        source_object_json(env, texture_key),
                        JsonValue::Null,
                    )
                }
                "MovieTexture" => (
                    JsonValue::Null,
                    source_object_json(env, texture_key),
                    exact_movie_texture(env, texture_key)?,
                ),
                other => {
                    return Err(format!(
                        "material texture {} has type {other}, expected Texture2D or MovieTexture",
                        object_id(env, texture_key)
                    ));
                }
            }
        };
        texture_envs.push(json!({
            "slot": slot_index,
            "name": name,
            "texturePointer": pointer_json(pointer),
            "unassignedSlot": unassigned_slot,
            "textureId": texture_id,
            "textureSource": texture_source,
            "dynamicTexture": dynamic_texture,
            "scale": unity_to_lossless_json(tex_env.get("m_Scale").unwrap_or(&UnityValue::Object(BTreeMap::new()))),
            "offset": unity_to_lossless_json(tex_env.get("m_Offset").unwrap_or(&UnityValue::Object(BTreeMap::new()))),
            "pivot": tex_env.get("m_Pivot").map(unity_to_lossless_json).unwrap_or(JsonValue::Null),
            "rotation": tex_env.get("m_Rotation").map(unity_to_lossless_json).unwrap_or(JsonValue::Null),
            "raw": unity_to_lossless_json(tex_env),
        }));
    }

    let shader = exact_shader(env, material.get("m_Shader"), key)?;
    let shader_name = shader
        .as_ref()
        .and_then(|shader| shader.get("name"))
        .and_then(JsonValue::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    let render_queue = material
        .get("m_CustomRenderQueue")
        .or_else(|| material.get("m_RenderQueue"))
        .map(unity_to_lossless_json)
        .unwrap_or(JsonValue::Null);
    let raw_material_fields = material
        .as_object()
        .map(|object| {
            JsonValue::Object(
                object
                    .iter()
                    .filter(|(name, _)| {
                        !matches!(name.as_str(), "m_Name" | "m_Shader" | "m_SavedProperties")
                    })
                    .map(|(name, value)| (name.clone(), unity_to_lossless_json(value)))
                    .collect(),
            )
        })
        .unwrap_or(JsonValue::Null);

    let primary_color = colors
        .iter()
        .find(|entry| entry.get("name").and_then(JsonValue::as_str) == Some("_Color"))
        .and_then(|entry| entry.get("value"))
        .and_then(color_hex)
        .unwrap_or_else(|| "#8b8f76".to_string());
    let saved_render_state = floats
        .iter()
        .filter(|entry| {
            matches!(
                entry.get("name").and_then(JsonValue::as_str),
                Some("_SrcBlend" | "_DstBlend" | "_Cull" | "_ZWrite" | "_Cutoff")
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let shader_evidence = shader
        .as_ref()
        .and_then(|shader| shader.get("renderStateEvidence"))
        .cloned()
        .unwrap_or(JsonValue::Null);

    let mut unknown_saved = JsonMap::new();
    for (name, value) in saved_object {
        if !matches!(name.as_str(), "m_Colors" | "m_Floats" | "m_TexEnvs") {
            unknown_saved.insert(name.clone(), unity_to_lossless_json(value));
        }
    }

    Ok(json!({
        "id": object_id(env, key),
        "source": source_object_json(env, key),
        "name": object_name(&material),
        // Retained for mesh_to_preview's tiny material summary. It is derived
        // only for display; exact channel values remain below.
        "color": primary_color,
        "shaderName": shader_name,
        "shader": shader,
        "renderQueue": render_queue,
        "renderState": {
            "materialFields": raw_material_fields,
            "savedFloatEvidence": saved_render_state,
            "shaderEvidence": shader_evidence,
        },
        "savedProperties": {
            "colors": colors,
            "floats": floats,
            "textureEnvs": texture_envs,
            "unknownFields": unknown_saved,
        },
        "rawSavedProperties": unity_to_lossless_json(saved),
    }))
}

pub(super) fn exact_shader(
    env: &UnityEnvironment,
    shader_value: Option<&UnityValue>,
    material_key: ObjectKey,
) -> Result<Option<JsonValue>, String> {
    let Some(shader_value) = shader_value else {
        return Ok(None);
    };
    let pointer = match shader_value {
        UnityValue::Pointer(pointer) => pointer,
        _ => {
            return Err(format!(
                "{}.m_Shader is not a PPtr",
                object_id(env, material_key)
            ));
        }
    };
    if pointer.is_null() {
        return Ok(None);
    }
    let key = strict_non_null_pointer(env, pointer).map_err(|error| {
        format!(
            "{} shader ({}/{}) could not be resolved: {}",
            object_id(env, material_key),
            pointer.file_id,
            pointer.path_id,
            error
        )
    })?;
    ensure_object_type(env, key, "Shader", "material shader")?;
    let shader = env.read_object(key)?;
    let script = shader.get("m_Script").and_then(unity_text);
    let serialized_name = object_name(&shader);
    let declared_name = script.as_deref().and_then(shader_declared_name);
    let name = if serialized_name.trim().is_empty() {
        declared_name.clone().unwrap_or_default()
    } else {
        serialized_name.clone()
    };
    let script_bytes = shader
        .get("m_Script")
        .and_then(unity_bytes)
        .unwrap_or_default();
    let script_payload = match std::str::from_utf8(&script_bytes) {
        Ok(text) => json!({
            "encoding": "utf-8",
            "text": text,
        }),
        Err(_) => json!({
            "encoding": "base64",
            "data": STANDARD.encode(&script_bytes),
        }),
    };
    let raw_shader_fields = shader
        .as_object()
        .map(|object| {
            JsonValue::Object(
                object
                    .iter()
                    .filter(|(name, _)| !matches!(name.as_str(), "m_Name" | "m_Script"))
                    .map(|(name, value)| (name.clone(), unity_to_lossless_json(value)))
                    .collect(),
            )
        })
        .unwrap_or(JsonValue::Null);
    let evidence = script
        .as_deref()
        .map(shader_render_state_evidence)
        .unwrap_or_else(|| {
            json!({
                "blend": [], "cull": [], "zWrite": [], "tags": [], "passes": []
            })
        });
    Ok(Some(json!({
        "pointer": pointer_json(pointer),
        "source": source_object_json(env, key),
        "serializedName": serialized_name,
        "declaredName": declared_name,
        "name": name,
        "scriptByteLength": script_bytes.len(),
        "scriptSha256": sha256_hex(&script_bytes),
        "script": script_payload,
        "rawShaderFields": raw_shader_fields,
        "renderStateEvidence": evidence,
    })))
}

pub(super) fn shader_render_state_evidence(script: &str) -> JsonValue {
    let mut blend = Vec::new();
    let mut cull = Vec::new();
    let mut z_write = Vec::new();
    let mut tags = Vec::new();
    let mut passes = Vec::new();
    for (line_index, line) in script.replace('\0', "\n").lines().enumerate() {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        let lower = text.to_ascii_lowercase();
        let evidence = json!({ "line": line_index + 1, "text": text });
        if lower == "pass" || lower.starts_with("pass ") || lower.starts_with("pass{") {
            passes.push(evidence.clone());
        }
        if lower.starts_with("blend ") || lower.starts_with("blendop ") {
            blend.push(evidence.clone());
        }
        if lower.starts_with("cull ") {
            cull.push(evidence.clone());
        }
        if lower.starts_with("zwrite ") || lower.starts_with("ztest ") {
            z_write.push(evidence.clone());
        }
        if lower.starts_with("tags ") || lower.contains("\"queue\"") {
            tags.push(evidence);
        }
    }
    json!({
        "blend": blend,
        "cull": cull,
        "zWriteAndZTest": z_write,
        "tags": tags,
        "passes": passes,
    })
}

pub(super) fn shader_declared_name(script: &str) -> Option<String> {
    let marker = "Shader \"";
    let start = script.find(marker)? + marker.len();
    let rest = script.get(start..)?;
    let end = rest.find('"')?;
    let name = rest[..end].trim();
    (!name.is_empty()).then(|| name.to_string())
}

pub(super) fn material_value_id(value: &JsonValue) -> Option<&str> {
    value.get("id").and_then(JsonValue::as_str)
}
