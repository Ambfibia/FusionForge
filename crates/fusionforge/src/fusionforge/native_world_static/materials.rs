use super::*;

pub(super) const MATERIAL_SCHEMA: &str = "ffone.native-static-world-materials.v1";

pub(super) fn renderer_material_slots(
    exact: &ExactMaterialExport,
) -> Result<HashMap<ObjectKey, Vec<Option<String>>>, String> {
    let mut result = HashMap::new();
    for binding in &exact.renderer_bindings {
        let renderer = binding
            .get("renderer")
            .and_then(JsonValue::as_object)
            .ok_or_else(|| "exact renderer binding has no renderer object".to_string())?;
        let asset = renderer
            .get("assetIndex")
            .and_then(JsonValue::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| "exact renderer binding has invalid assetIndex".to_string())?;
        let path_id = renderer
            .get("pathId")
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| "exact renderer binding has invalid pathId".to_string())?;
        let slots = binding
            .get("materialSlots")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| "exact renderer binding has no materialSlots array".to_string())?
            .iter()
            .map(|slot| {
                slot.get("materialId")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        if result.insert(ObjectKey { asset, path_id }, slots).is_some() {
            return Err(format!(
                "duplicate exact renderer binding for asset {asset} pathId {path_id}"
            ));
        }
    }
    Ok(result)
}

pub(super) fn gltf_material_from_exact(
    material: &JsonValue,
    material_id: &str,
    texture_files: &BTreeMap<String, String>,
    images: &mut Vec<JsonValue>,
    textures: &mut Vec<JsonValue>,
    samplers: &mut Vec<JsonValue>,
    texture_index_by_id: &mut BTreeMap<String, usize>,
) -> Result<JsonValue, String> {
    let name = material
        .get("name")
        .and_then(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(material_id);
    let colors = material
        .pointer("/savedProperties/colors")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let color = colors
        .iter()
        .find(|entry| {
            matches!(
                entry.get("name").and_then(JsonValue::as_str),
                Some("_Color" | "_TintColor" | "_MainColor")
            )
        })
        .and_then(|entry| entry.get("value"))
        .map(color_factor)
        .transpose()?
        .unwrap_or([0.545, 0.56, 0.463, 1.0]);
    let floats = material
        .pointer("/savedProperties/floats")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let float_value = |wanted: &str| {
        floats
            .iter()
            .find(|entry| entry.get("name").and_then(JsonValue::as_str) == Some(wanted))
            .and_then(|entry| entry.get("value"))
            .and_then(json_lossless_number)
    };
    let cutoff = float_value("_Cutoff");
    let cull = float_value("_Cull");
    let texture_envs = material
        .pointer("/savedProperties/textureEnvs")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let choose_texture = |names: &[&str]| -> Option<(String, String)> {
        names.iter().find_map(|wanted| {
            texture_envs.iter().find_map(|entry| {
                let name = entry.get("name").and_then(JsonValue::as_str)?;
                if !name.eq_ignore_ascii_case(wanted) {
                    return None;
                }
                let id = entry.get("textureId").and_then(JsonValue::as_str)?;
                Some((name.to_string(), id.to_string()))
            })
        })
    };
    let shader_name_exact = material
        .get("shaderName")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let is_legacy_water = shader_name_exact == "ffWater";
    let is_legacy_glow = matches!(
        shader_name_exact,
        "normal_glow_blendSrcalphaInvsrcalpha"
            | "normal_glow_blendSrcalphaInvsrcalphaTest_cullOff"
            | "normal_glow_blendSrcalphaInvsrcalphaTest_cullOff_vertexColorAD"
            | "normal_glow_blendOneOne_zwriteOff_vertexColorAD"
    );
    // StandardMaterial is only a loader bridge for native materials. Water
    // needs three exact source textures, so give each one a stable glTF role:
    // base=reflective gradient, normal=bump, emissive=Fresnel. The native
    // ffWater pass consumes those handles with their real semantics.
    let base_texture = if is_legacy_water {
        choose_texture(&["_ReflectiveColor"])
    } else {
        choose_texture(&[
            "_MainTex",
            "_BaseMap",
            "_Diffuse",
            "_DiffuseTex",
            "_ColorTexture",
        ])
        .or_else(|| {
            texture_envs.iter().find_map(|entry| {
                let name = entry.get("name").and_then(JsonValue::as_str)?;
                let lower = name.to_ascii_lowercase();
                if lower.contains("alpha")
                    || lower.contains("normal")
                    || lower.contains("bump")
                    || lower.contains("emission")
                {
                    return None;
                }
                let id = entry.get("textureId").and_then(JsonValue::as_str)?;
                Some((name.to_string(), id.to_string()))
            })
        })
    };
    // `_BumpMap` is not universally a tangent-space normal in the source
    // shaders. Every audited normal_glow program declares it as `Glow (RGB)`
    // and consumes its alpha as a fixed-function lerp mask. Publishing that
    // DXT5 mask as glTF normalTexture made the corresponding props appear to
    // have corrupt normals. ffWater is the only static-world shader whose
    // `_BumpMap` is an actual bump texture (and the native water material uses
    // this glTF field as its typed transport handle).
    let normal_texture = if is_legacy_water {
        choose_texture(&["_BumpMap"])
    } else {
        choose_texture(&["_NormalMap", "_NormalTex"])
    };
    let emissive_texture = if is_legacy_water {
        choose_texture(&["_Fresnel"])
    } else if is_legacy_glow {
        // The StandardMaterial fallback keeps emissiveFactor at zero; the
        // native legacy material consumes this handle as the exact glow mask.
        choose_texture(&["_BumpMap"])
    } else {
        choose_texture(&["_EmissionMap", "_Emissive", "_GlowTex"])
    };

    let mut texture_slots = Vec::<JsonValue>::new();
    for entry in &texture_envs {
        texture_slots.push(json!({
            "slot": entry.get("slot"),
            "name": entry.get("name"),
            "textureId": entry.get("textureId"),
            "scale": entry.get("scale"),
            "offset": entry.get("offset"),
            "pivot": entry.get("pivot"),
            "rotation": entry.get("rotation"),
        }));
    }
    let mut texture_info =
        |selected: &Option<(String, String)>| -> Result<Option<JsonValue>, String> {
            let Some((slot, texture_id)) = selected else {
                return Ok(None);
            };
            let index = if let Some(index) = texture_index_by_id.get(texture_id).copied() {
                index
            } else {
                let path = texture_files.get(texture_id).ok_or_else(|| {
                    format!(
                        "material {material_id:?} references unpublished texture {texture_id:?}"
                    )
                })?;
                let marker = "/textures/";
                let suffix = path
                    .split_once(marker)
                    .map(|(_, suffix)| suffix)
                    .ok_or_else(|| {
                        format!("published texture path {path:?} has no textures segment")
                    })?;
                let sampler = samplers.len();
                // Unity TextureWrapMode: the one-dimensional water gradient
                // is clamped while Waterbump/Fresnel repeat. Preserve the
                // source ffWater sampling behavior instead of wrapping the
                // poison gradient through its opposite edge.
                let wrap = if is_legacy_water && slot == "_ReflectiveColor" {
                    33_071
                } else {
                    10_497
                };
                samplers.push(json!({
                    "magFilter": 9729,
                    "minFilter": 9987,
                    "wrapS": wrap,
                    "wrapT": wrap
                }));
                let image = images.len();
                images.push(json!({
                    "name": texture_id,
                    "uri": format!("textures/{suffix}"),
                    "mimeType": "image/png"
                }));
                let index = textures.len();
                textures.push(json!({
                    "name": texture_id,
                    "source": image,
                    "sampler": sampler
                }));
                texture_index_by_id.insert(texture_id.clone(), index);
                index
            };
            Ok(Some(json!({ "index": index, "texCoord": 0 })))
        };
    let base_info = texture_info(&base_texture)?;
    let normal_info = texture_info(&normal_texture)?;
    let emissive_info = texture_info(&emissive_texture)?;
    let mut pbr = JsonMap::new();
    pbr.insert("baseColorFactor".to_string(), json!(color));
    pbr.insert("metallicFactor".to_string(), json!(0.0));
    pbr.insert("roughnessFactor".to_string(), json!(1.0));
    if let Some(info) = base_info {
        pbr.insert("baseColorTexture".to_string(), info);
    }
    let shader_name = shader_name_exact.to_ascii_lowercase();
    let compact_shader_name = compact_shader_label(shader_name_exact);
    let shader_script = material
        .pointer("/shader/script/text")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let compact_shader_script = compact_shader_label(shader_script);
    let render_queue = material
        .get("renderQueue")
        .and_then(JsonValue::as_i64)
        .unwrap_or(-1);
    let source_blend = float_value("_SrcBlend");
    let destination_blend = float_value("_DstBlend");
    let saved_blending = source_blend.is_some_and(|value| value != 1.0)
        || destination_blend.is_some_and(|value| value != 0.0);
    let shader_blend_enabled = shader_directives(shader_script, "blend")
        .any(|arguments| !arguments.eq_ignore_ascii_case("off"));
    let shader_alpha_test = shader_directives(shader_script, "alphatest")
        .any(|arguments| !arguments.eq_ignore_ascii_case("off"));
    let shader_cull_off = shader_directives(shader_script, "cull")
        .any(|arguments| arguments.eq_ignore_ascii_case("off"));
    let encoded_blend = compact_shader_name.contains("blendsrcalpha")
        || compact_shader_name.contains("blendoneone")
        || compact_shader_name.contains("additive");
    let encoded_alpha_test =
        compact_shader_name.contains("alphatest") || compact_shader_name.contains("test");
    let transparent_evidence = shader_name.contains("transparent")
        || shader_name.contains("additive")
        || encoded_blend
        || shader_blend_enabled
        || compact_shader_script.contains("\"queue\"=\"transparent\"")
        || render_queue >= 3000
        || saved_blending;
    let mut gltf = JsonMap::new();
    gltf.insert("name".to_string(), json!(name));
    gltf.insert("pbrMetallicRoughness".to_string(), JsonValue::Object(pbr));
    if let Some(info) = normal_info {
        gltf.insert("normalTexture".to_string(), info);
    }
    if let Some(info) = emissive_info {
        gltf.insert("emissiveTexture".to_string(), info);
        gltf.insert(
            "emissiveFactor".to_string(),
            if is_legacy_glow {
                json!([0.0, 0.0, 0.0])
            } else {
                json!([1.0, 1.0, 1.0])
            },
        );
    }
    if cutoff.is_some_and(|value| value > 0.0) || shader_alpha_test || encoded_alpha_test {
        gltf.insert("alphaMode".to_string(), json!("MASK"));
        gltf.insert(
            "alphaCutoff".to_string(),
            json!(cutoff
                .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                .or_else(|| shader_alpha_test_reference(shader_script))
                .unwrap_or(0.5)),
        );
    } else if color[3] < 0.999 || transparent_evidence {
        gltf.insert("alphaMode".to_string(), json!("BLEND"));
    } else {
        gltf.insert("alphaMode".to_string(), json!("OPAQUE"));
    }
    if cull.is_some_and(|value| value == 0.0)
        || compact_shader_name.contains("culloff")
        || shader_cull_off
    {
        gltf.insert("doubleSided".to_string(), json!(true));
    }
    gltf.insert(
        "extras".to_string(),
        json!({
            "ffoneSourceMaterialId": material_id,
            "shaderName": material.get("shaderName"),
            "colors": material.pointer("/savedProperties/colors"),
            "floats": material.pointer("/savedProperties/floats"),
            "exactTextureSlots": texture_slots,
            "legacyRenderState": material.get("renderState"),
            "runtimeTextureContract": if is_legacy_water {
                Some(json!({
                    "schema": "ffone.legacy-water-texture-bindings.v1",
                    "baseColorTexture": "_ReflectiveColor",
                    "normalTexture": "_BumpMap",
                    "emissiveTexture": "_Fresnel"
                }))
            } else if is_legacy_glow {
                Some(json!({
                    "schema": "ffone.legacy-glow-texture-bindings.v1",
                    "emissiveTexture": "_BumpMap",
                    "semantic": "constantColor(0.5)-lerp-by-texture-alpha"
                }))
            } else {
                None
            },
        }),
    );
    Ok(JsonValue::Object(gltf))
}

pub(super) fn compact_shader_label(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .replace(['_', '-', ' ', '/', '\\', '\r', '\n', '\t'], "")
}

pub(super) fn shader_directives<'a>(
    script: &'a str,
    directive: &'a str,
) -> impl Iterator<Item = &'a str> + 'a {
    script.split(['\n', '\0']).filter_map(move |line| {
        let statement = line.split_once("//").map_or(line, |(code, _)| code).trim();
        let (command, arguments) = statement.split_once(char::is_whitespace)?;
        command
            .eq_ignore_ascii_case(directive)
            .then_some(arguments.trim())
    })
}

pub(super) fn shader_alpha_test_reference(script: &str) -> Option<f64> {
    shader_directives(script, "alphatest").find_map(|arguments| {
        arguments
            .split_whitespace()
            .skip(1)
            .find_map(|token| token.parse::<f64>().ok())
            .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
    })
}
