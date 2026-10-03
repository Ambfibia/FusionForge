use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn audit_materials_and_images(
    output_root: &Path,
    glb_relative: &str,
    materials: &[Value],
    images: &[Value],
    textures: &[Value],
    samplers: &[Value],
    pngs: &BTreeSet<String>,
    violations: &mut Vec<LogicalModelTreeViolation>,
) -> Result<BTreeSet<String>> {
    let mut used_textures = BTreeSet::new();
    for (material_index, material) in materials.iter().enumerate() {
        match material.get("name").and_then(Value::as_str) {
            Some(name) => validate_true_name(name, "material", glb_relative, violations),
            None => push_violation(
                violations,
                "missing_material_name",
                glb_relative,
                format!("material {material_index} has no exact name"),
            ),
        }
        let exact_material = material.pointer("/extras/ffone");
        let serialized_shader_name = exact_material
            .and_then(|value| value.get("serializedShaderName"))
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty());
        let declared_shader_name = exact_material
            .and_then(|value| value.get("declaredShaderName"))
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty());
        let runtime_shader_name = exact_material
            .and_then(|value| value.get("legacyShaderName"))
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty());
        if serialized_shader_name.is_none() || declared_shader_name.is_none() {
            push_violation(
                violations,
                "missing_exact_shader_identity",
                glb_relative,
                format!(
                    "material {material_index} does not preserve serializedShaderName and declaredShaderName"
                ),
            );
        }
        if runtime_shader_name != declared_shader_name {
            push_violation(
                violations,
                "contradictory_runtime_shader_identity",
                glb_relative,
                format!(
                    "material {material_index} legacyShaderName does not exactly equal declaredShaderName"
                ),
            );
        }
        let mut shader_default_slots = BTreeSet::new();
        match exact_material
            .and_then(|value| value.get("shaderTextureDefaults"))
            .cloned()
            .map(serde_json::from_value::<Vec<ShaderLabTextureDefaultProperty>>)
        {
            Some(Ok(defaults)) => {
                for property in defaults {
                    if property.slot.is_empty() || !shader_default_slots.insert(property.slot) {
                        push_violation(
                            violations,
                            "invalid_shader_texture_default",
                            glb_relative,
                            format!(
                                "material {material_index} has an empty or duplicate ShaderLab texture-default slot"
                            ),
                        );
                    }
                }
            }
            Some(Err(error)) => push_violation(
                violations,
                "invalid_shader_texture_default_schema",
                glb_relative,
                format!(
                    "material {material_index} ShaderLab texture defaults are not the typed native schema: {error}"
                ),
            ),
            None => push_violation(
                violations,
                "missing_shader_texture_defaults",
                glb_relative,
                format!("material {material_index} has no ShaderLab texture-default provenance"),
            ),
        }
        for pointer in [
            "/pbrMetallicRoughness/baseColorTexture/index",
            "/pbrMetallicRoughness/metallicRoughnessTexture/index",
            "/normalTexture/index",
            "/occlusionTexture/index",
            "/emissiveTexture/index",
        ] {
            if let Some(index) = material
                .pointer(pointer)
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
            {
                if index < textures.len() {
                    used_textures.insert(index);
                } else {
                    push_violation(
                        violations,
                        "invalid_material_texture",
                        glb_relative,
                        format!("material {material_index} references invalid texture {index}"),
                    );
                }
            }
        }
        let exact = material
            .pointer("/extras/ffone/textureBindings")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for binding in exact {
            let Some(object) = binding.as_object() else {
                push_violation(
                    violations,
                    "invalid_exact_texture_binding_schema",
                    glb_relative,
                    format!("material {material_index} exact binding is not an object"),
                );
                continue;
            };
            for required in [
                "slot",
                "texture",
                "sourceName",
                "uri",
                "sampler",
                "mipProvenance",
                "mipLevels",
                "scale",
                "offset",
                "pivot",
                "rotation",
                "colorSpace",
            ] {
                if !object.contains_key(required) {
                    push_violation(
                        violations,
                        "missing_exact_texture_binding_field",
                        glb_relative,
                        format!(
                            "material {material_index} exact binding omits required field {required}"
                        ),
                    );
                }
            }
            if let Some(slot) = binding.get("slot").and_then(Value::as_str)
                && !shader_default_slots.contains(slot)
                && !binding.get("texture").is_none_or(Value::is_null)
                && !binding
                    .get("ignoredStaleShaderBinding")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                push_violation(
                    violations,
                    "missing_binding_shader_texture_default",
                    glb_relative,
                    format!(
                        "material {material_index} assigned exact binding {slot:?} has no ShaderLab texture default"
                    ),
                );
            }
            if let Err(error) = serde_json::from_value::<MaterialTextureBinding>(binding.clone()) {
                push_violation(
                    violations,
                    "invalid_exact_texture_binding_schema",
                    glb_relative,
                    format!(
                        "material {material_index} exact binding is not the typed native schema: {error}"
                    ),
                );
            }
            if let Some(index) = binding
                .get("texture")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
            {
                if index < textures.len() {
                    used_textures.insert(index);
                } else {
                    push_violation(
                        violations,
                        "invalid_exact_texture_binding",
                        glb_relative,
                        format!(
                            "material {material_index} exact binding references texture {index}"
                        ),
                    );
                }
            }
        }
    }
    if used_textures.len() != textures.len() {
        push_violation(
            violations,
            "orphan_texture",
            glb_relative,
            format!(
                "{} of {} glTF textures are referenced by exact materials",
                used_textures.len(),
                textures.len()
            ),
        );
    }

    let mut used_images = BTreeSet::new();
    let mut used_samplers = BTreeSet::new();
    for (sampler_index, sampler) in samplers.iter().enumerate() {
        match sampler.get("name").and_then(Value::as_str) {
            Some(name) => validate_true_name(name, "sampler", glb_relative, violations),
            None => push_violation(
                violations,
                "missing_sampler_name",
                glb_relative,
                format!("sampler {sampler_index} has no exact name"),
            ),
        }
    }
    for (texture_index, texture) in textures.iter().enumerate() {
        let texture_name = texture.get("name").and_then(Value::as_str);
        match texture_name {
            Some(name) => validate_true_name(name, "texture", glb_relative, violations),
            None => push_violation(
                violations,
                "missing_texture_name",
                glb_relative,
                format!("texture {texture_index} has no exact name"),
            ),
        }
        let source = texture
            .get("source")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        match source {
            Some(source) if source < images.len() => {
                used_images.insert(source);
                if let (Some(texture_name), Some(source_name)) = (
                    texture_name,
                    images[source]
                        .pointer("/extras/ffone/sourceName")
                        .and_then(Value::as_str),
                ) && texture_name != source_name
                {
                    push_violation(
                        violations,
                        "texture_image_name_mismatch",
                        glb_relative,
                        format!(
                            "texture {texture_index} name {texture_name:?} differs from owning image {source} exact sourceName {source_name:?}"
                        ),
                    );
                }
            }
            _ => push_violation(
                violations,
                "invalid_texture_source",
                glb_relative,
                format!("texture {texture_index} has no valid source image"),
            ),
        }
        let sampler = texture
            .get("sampler")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        match sampler {
            Some(sampler) if sampler < samplers.len() => {
                used_samplers.insert(sampler);
                let sampler_name = samplers[sampler].get("name").and_then(Value::as_str);
                let image_source_name =
                    source
                        .filter(|source| *source < images.len())
                        .and_then(|source| {
                            images[source]
                                .pointer("/extras/ffone/sourceName")
                                .and_then(Value::as_str)
                        });
                if let (Some(sampler_name), Some(expected_name)) =
                    (sampler_name, image_source_name.or(texture_name))
                    && sampler_name != expected_name
                {
                    push_violation(
                        violations,
                        "sampler_texture_name_mismatch",
                        glb_relative,
                        format!(
                            "sampler {sampler} name {sampler_name:?} differs from texture {texture_index} exact source name {expected_name:?}"
                        ),
                    );
                }
            }
            _ => push_violation(
                violations,
                "invalid_texture_sampler",
                glb_relative,
                format!("texture {texture_index} has no valid explicit sampler"),
            ),
        }
    }
    if used_images.len() != images.len() {
        push_violation(
            violations,
            "orphan_image",
            glb_relative,
            format!(
                "{} of {} images are used by textures",
                used_images.len(),
                images.len()
            ),
        );
    }
    if used_samplers.len() != samplers.len() {
        push_violation(
            violations,
            "orphan_sampler",
            glb_relative,
            format!(
                "{} of {} samplers are used by textures",
                used_samplers.len(),
                samplers.len()
            ),
        );
    }

    let mut preferred_name_counts = BTreeMap::<String, usize>::new();
    for image in images {
        if let Some(source_name) = image
            .pointer("/extras/ffone/sourceName")
            .and_then(Value::as_str)
            && let Ok(preferred) = minimal_windows_png_filename(source_name)
        {
            *preferred_name_counts
                .entry(preferred.to_ascii_lowercase())
                .or_default() += 1;
        }
    }
    let mut referenced = BTreeSet::new();
    for (image_index, image) in images.iter().enumerate() {
        if image.get("bufferView").is_some() {
            push_violation(
                violations,
                "embedded_texture",
                glb_relative,
                format!("image {image_index} is embedded instead of a browsable PNG"),
            );
        }
        let source_name = image
            .pointer("/extras/ffone/sourceName")
            .and_then(Value::as_str);
        match source_name {
            Some(name) => validate_true_name(name, "image source", glb_relative, violations),
            None => push_violation(
                violations,
                "missing_image_source_name",
                glb_relative,
                format!("image {image_index} has no exact extras.ffone.sourceName"),
            ),
        }
        let image_name = image.get("name").and_then(Value::as_str);
        match image_name {
            Some(name) => validate_true_name(name, "image", glb_relative, violations),
            None => push_violation(
                violations,
                "missing_image_name",
                glb_relative,
                format!("image {image_index} has no exact glTF name"),
            ),
        }
        if let (Some(source_name), Some(image_name)) = (source_name, image_name)
            && source_name != image_name
        {
            push_violation(
                violations,
                "image_name_source_name_mismatch",
                glb_relative,
                format!(
                    "image {image_index} glTF name {image_name:?} differs from exact sourceName {source_name:?}"
                ),
            );
        }
        let Some(uri) = image.get("uri").and_then(Value::as_str) else {
            push_violation(
                violations,
                "missing_image_uri",
                glb_relative,
                format!("image {image_index} has no external PNG URI"),
            );
            continue;
        };
        let Some(image_relative) = safe_image_relative(glb_relative, uri) else {
            push_violation(
                violations,
                "unsafe_image_uri",
                glb_relative,
                format!("image {image_index} URI {uri:?} is unsafe or is not a PNG"),
            );
            continue;
        };
        if let Some(source_name) = source_name {
            let expected = minimal_windows_png_filename(source_name).and_then(|preferred| {
                if preferred_name_counts
                    .get(&preferred.to_ascii_lowercase())
                    .copied()
                    .unwrap_or_default()
                    > 1
                {
                    windows_png_filename_preserving_legacy_extension(source_name)
                } else {
                    Ok(preferred)
                }
            });
            match expected {
                Ok(expected)
                    if (uri.starts_with("../") && uri.split('/').any(|part| part == "textures"))
                        || Path::new(uri).file_name().and_then(|value| value.to_str())
                        == Some(expected.as_str()) => {}
                Ok(expected) => push_violation(
                    violations,
                    "texture_true_name_mismatch",
                    &image_relative,
                    format!(
                        "PNG filename must be minimally derived from exact source name {source_name:?}: {expected:?}"
                    ),
                ),
                Err(error) => push_violation(
                    violations,
                    "invalid_texture_source_name",
                    &image_relative,
                    error.to_string(),
                ),
            }
        }
        referenced.insert(image_relative.clone());
        if !pngs.contains(&image_relative) {
            push_violation(
                violations,
                "missing_png",
                &image_relative,
                format!("referenced by {glb_relative} image {image_index}"),
            );
            continue;
        }
        let png_path = output_root.join(Path::new(&image_relative));
        let bytes = fs::read(&png_path).map_err(|error| io_at(&png_path, error))?;
        let Some((width, height)) = png_dimensions(&bytes) else {
            push_violation(
                violations,
                "invalid_png",
                &image_relative,
                "referenced texture is not a valid PNG with an IHDR",
            );
            continue;
        };
        let expected_width = image.pointer("/extras/ffone/width").and_then(Value::as_u64);
        let expected_height = image
            .pointer("/extras/ffone/height")
            .and_then(Value::as_u64);
        if expected_width != Some(u64::from(width)) || expected_height != Some(u64::from(height)) {
            push_violation(
                violations,
                "png_dimension_mismatch",
                &image_relative,
                format!(
                    "PNG is {width}x{height}, image provenance says {expected_width:?}x{expected_height:?}"
                ),
            );
        }
        audit_exact_mip_pngs(
            output_root,
            glb_relative,
            image_index,
            image,
            uri,
            pngs,
            &mut referenced,
            violations,
        )?;
    }
    Ok(referenced)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn audit_animations(
    animations: &[Value],
    metadata_only: &[Value],
    nodes: &[Value],
    accessors: &[Option<AccessorInfo>],
    binary: &[u8],
    relative: &str,
    features: &mut LogicalModelFeatureCounts,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    let node_count = nodes.len();
    let hierarchy_paths = hierarchy_paths_from_glb_nodes(nodes);
    for (animation_index, animation) in animations.iter().enumerate() {
        match animation.get("name").and_then(Value::as_str) {
            Some(name) => validate_true_name(name, "animation", relative, violations),
            None => push_violation(
                violations,
                "missing_standard_animation_name",
                relative,
                format!("standard animation {animation_index} has no exact name"),
            ),
        }
        let samplers = animation
            .get("samplers")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let channels = animation
            .get("channels")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if samplers.is_empty() || channels.is_empty() {
            push_violation(
                violations,
                "empty_standard_animation",
                relative,
                format!("standard animation {animation_index} has no samplers/channels"),
            );
        }
        features.animation_channels += to_u64(channels.len());
        let mut standard_targets = BTreeSet::new();
        let mut source_channels = Vec::with_capacity(channels.len());
        for (channel_index, channel) in channels.iter().enumerate() {
            let context = format!("animation {animation_index} channel {channel_index}");
            let target = channel
                .pointer("/target/node")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok());
            if target.is_none_or(|target| target >= node_count) {
                push_violation(
                    violations,
                    "invalid_animation_target",
                    relative,
                    format!("{context} targets an invalid node"),
                );
            }
            let path = channel.pointer("/target/path").and_then(Value::as_str);
            let kind = path.and_then(|path| match path {
                "translation" => Some(EmptyTrsBindingKind::Translation),
                "rotation" => Some(EmptyTrsBindingKind::Rotation),
                "scale" => Some(EmptyTrsBindingKind::Scale),
                _ => None,
            });
            let expected_components = match path {
                Some("translation" | "scale") => Some(3),
                Some("rotation") => Some(4),
                _ => None,
            };
            if expected_components.is_none() {
                push_violation(
                    violations,
                    "invalid_animation_path",
                    relative,
                    format!("{context} is not an exact TRS channel"),
                );
            }
            if let (Some(target), Some(kind)) = (target, kind)
                && !standard_targets.insert((target, kind))
            {
                push_violation(
                    violations,
                    "duplicate_animation_target",
                    relative,
                    format!("{context} duplicates a standard TRS target"),
                );
            }
            let sampler = channel
                .get("sampler")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .and_then(|index| samplers.get(index));
            let Some(sampler) = sampler else {
                push_violation(
                    violations,
                    "invalid_animation_sampler",
                    relative,
                    format!("{context} references an invalid sampler"),
                );
                continue;
            };
            let input = sampler
                .get("input")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .and_then(|index| accessors.get(index))
                .and_then(Option::as_ref);
            audit_channel_key_provenance(
                channel,
                input.map(|input| input.count),
                relative,
                &context,
                features,
                violations,
            );
            if let (Some(target), Some(kind), Some(input)) = (target, kind, input)
                && let Some(source_channel) =
                    audited_animation_channel(channel, kind, target, *input, binary)
            {
                source_channels.push(source_channel);
            }
            let output = sampler
                .get("output")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .and_then(|index| accessors.get(index))
                .and_then(Option::as_ref);
            let interpolation = sampler
                .get("interpolation")
                .and_then(Value::as_str)
                .unwrap_or("LINEAR");
            let valid_interpolation = matches!(interpolation, "LINEAR" | "STEP" | "CUBICSPLINE");
            if !valid_interpolation {
                push_violation(
                    violations,
                    "invalid_animation_interpolation",
                    relative,
                    format!("{context} has unsupported interpolation {interpolation:?}"),
                );
            }
            match (input, output, expected_components) {
                (Some(input), Some(output), Some(components))
                    if input.component_type == 5_126
                        && input.component_count == 1
                        && output.component_type == 5_126
                        && output.component_count == components =>
                {
                    let multiplier = usize::from(interpolation == "CUBICSPLINE") * 2 + 1;
                    if output.count != input.count.saturating_mul(multiplier) {
                        push_violation(
                            violations,
                            "animation_accessor_count_mismatch",
                            relative,
                            format!("{context} input/output accessor counts disagree"),
                        );
                    }
                    let mut previous = None;
                    for key in 0..input.count {
                        let value = read_f32(*input, binary, key, 0).map(f64::from);
                        if value.is_none_or(|value| {
                            !value.is_finite()
                                || value < 0.0
                                || previous.is_some_and(|previous| previous >= value)
                        }) {
                            push_violation(
                                violations,
                                "invalid_animation_times",
                                relative,
                                format!(
                                    "{context} input times are not finite and strictly increasing"
                                ),
                            );
                            break;
                        }
                        previous = value;
                    }
                    features.animation_keyframes += to_u64(input.count);
                    if interpolation == "CUBICSPLINE" {
                        features.cubic_spline_keyframes += to_u64(input.count);
                    }
                }
                _ => push_violation(
                    violations,
                    "invalid_animation_accessors",
                    relative,
                    format!("{context} has invalid input/output accessors"),
                ),
            }
        }
        audit_animation_metadata(
            animation.pointer("/extras/nonTrs"),
            node_count,
            &hierarchy_paths,
            &standard_targets,
            &source_channels,
            animation
                .pointer("/extras/duration")
                .and_then(Value::as_f64),
            animation
                .pointer("/extras/sampleRate")
                .and_then(Value::as_f64),
            relative,
            &format!("animation {animation_index}"),
            features,
            violations,
        );
    }
    for (index, animation) in metadata_only.iter().enumerate() {
        if let Some(name) = animation.get("name").and_then(Value::as_str) {
            validate_true_name(name, "metadata-only animation", relative, violations);
        } else {
            push_violation(
                violations,
                "missing_metadata_animation_name",
                relative,
                format!("metadata-only animation {index} has no exact name"),
            );
        }
        audit_animation_metadata(
            animation.get("metadata"),
            node_count,
            &hierarchy_paths,
            &BTreeSet::new(),
            &[],
            animation.get("duration").and_then(Value::as_f64),
            animation.get("sampleRate").and_then(Value::as_f64),
            relative,
            &format!("metadata-only animation {index}"),
            features,
            violations,
        );
    }
}
