use super::*;

pub const REVIEWED_TEXTURE_REBINDS_SCHEMA: &str = "ffone.reviewed-logical-model-texture-rebinds.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedTextureRebindReport {
    pub schema: String,
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub entries: Vec<AppliedTextureRebindReport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppliedTextureRebindReport {
    pub material_id: String,
    pub material_name: String,
    pub slot: u64,
    pub slot_name: String,
    pub stale_texture_id: String,
    pub stale_source_asset_index: u64,
    pub stale_file_id: i64,
    pub stale_path_id: i64,
    pub replacement_texture_id: String,
    pub replacement_texture_name: String,
    pub replacement_source_asset_index: u64,
    pub replacement_path_id: i64,
    pub replacement_texture_json: String,
    pub replacement_texture_json_sha256: String,
    pub reason: String,
    pub evidence: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishedTextureReport {
    pub source_name: String,
    pub uri: String,
    pub width: u32,
    pub height: u32,
    pub source_mip_count: u32,
    pub published_policy: String,
    pub byte_length: u64,
    pub sha256: String,
    pub mip_levels: Vec<PublishedTextureMipLevelReport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishedTextureMipLevelReport {
    pub level: u32,
    pub uri: String,
    pub width: u32,
    pub height: u32,
    pub source_byte_offset: u64,
    pub source_byte_length: u64,
    pub source_byte_sha256: String,
    pub decoded_rgba8_byte_length: u64,
    pub decoded_rgba8_sha256: String,
    pub png_byte_length: u64,
    pub png_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceTextureEnvironment {
    pub(super) slot: usize,
    pub(super) name: String,
    #[serde(default)]
    pub(super) unassigned_slot: bool,
    pub(super) texture_id: Option<String>,
    #[serde(default)]
    pub(super) dynamic_texture: Option<SourceDynamicTexture>,
    pub(super) scale: SourceVec2,
    pub(super) offset: SourceVec2,
    pub(super) pivot: Option<SourceVec2>,
    pub(super) rotation: Option<f64>,
    pub(super) texture_pointer: SourcePointer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceDynamicTexture {
    pub(super) id: String,
    pub(super) source: SourceObjectReference,
    pub(super) name: String,
    pub(super) object_type: String,
    pub(super) container: String,
    pub(super) mime_type: String,
    pub(super) looped: bool,
    pub(super) audio_clip_pointer: SourcePointer,
    pub(super) payload: SourceDynamicTexturePayload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceTexture {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) source: SourceObjectReference,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) texture_format: i32,
    pub(super) texture_format_name: String,
    pub(super) mip_count: u32,
    pub(super) mip_count_evidence: Value,
    pub(super) sampler: SourceSampler,
    pub(super) source_payload: SourceTextureChainPayload,
    pub(super) mip_levels: Vec<SourceTextureMipLevel>,
    pub(super) payload: SourceTexturePayload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceTextureMipLevel {
    pub(super) level: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) source_byte_offset: usize,
    pub(super) source_byte_length: usize,
    pub(super) source_byte_sha256: String,
    pub(super) decoded_rgba_byte_length: usize,
    pub(super) decoded_rgba_sha256: String,
    pub(super) payload: SourceTexturePayload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ReviewedTextureRebindManifest {
    pub(super) schema: String,
    pub(super) logical_name: String,
    pub(super) rebinds: Vec<ReviewedTextureRebind>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ReviewedTextureRebind {
    pub(super) material_id: String,
    pub(super) slot: usize,
    pub(super) slot_name: String,
    pub(super) expected_texture_id: String,
    pub(super) expected_source_asset_index: usize,
    pub(super) expected_file_id: i64,
    pub(super) expected_path_id: i64,
    pub(super) replacement_texture_json: String,
    pub(super) reason: String,
    pub(super) evidence: Value,
}

pub(super) struct TexturePublication {
    pub(super) uri: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) fn apply_reviewed_texture_rebinds(
    options: &LogicalModelPublishOptions,
    source: &mut SourceDocument,
) -> Result<Option<ReviewedTextureRebindReport>> {
    let Some(manifest_path) = options.reviewed_texture_rebinds.as_deref() else {
        return Ok(None);
    };
    let manifest_bytes = fs::read(manifest_path).map_err(|error| io_at(manifest_path, error))?;
    let manifest: ReviewedTextureRebindManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source_error| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source: source_error,
        })?;
    if manifest.schema != REVIEWED_TEXTURE_REBINDS_SCHEMA {
        return invalid(format!(
            "reviewed texture-rebind manifest schema is {:?}, expected {:?}",
            manifest.schema, REVIEWED_TEXTURE_REBINDS_SCHEMA
        ));
    }
    if manifest.logical_name != source.logical_name {
        return invalid(format!(
            "reviewed texture-rebind manifest targets logical model {:?}, source is {:?}",
            manifest.logical_name, source.logical_name
        ));
    }
    if manifest.rebinds.is_empty() {
        return invalid("reviewed texture-rebind manifest has no rebinds");
    }

    let manifest_parent = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut entries = Vec::with_capacity(manifest.rebinds.len());
    for rebind in manifest.rebinds {
        if rebind.reason.trim().is_empty()
            || !rebind.evidence.is_object()
            || rebind
                .evidence
                .as_object()
                .is_none_or(serde_json::Map::is_empty)
        {
            return invalid(format!(
                "reviewed texture rebind for material {:?} lacks a reason or structured evidence",
                rebind.material_id
            ));
        }
        let replacement_path = PathBuf::from(&rebind.replacement_texture_json);
        let replacement_path = if replacement_path.is_absolute() {
            replacement_path
        } else {
            manifest_parent.join(replacement_path)
        };
        let replacement_bytes =
            fs::read(&replacement_path).map_err(|error| io_at(&replacement_path, error))?;
        let replacement: SourceTexture =
            serde_json::from_slice(&replacement_bytes).map_err(|source_error| {
                PipelineError::Json {
                    path: replacement_path.display().to_string(),
                    source: source_error,
                }
            })?;
        validate_texture_source(&replacement.id, &replacement)?;
        if source.textures.contains_key(&replacement.id) {
            return invalid(format!(
                "reviewed replacement texture id {:?} already exists in the exact source",
                replacement.id
            ));
        }

        let replacement_id = replacement.id.clone();
        let replacement_name = replacement.name.clone();
        let replacement_asset_index = replacement.source.asset_index;
        let replacement_path_id = replacement.source.path_id;
        let material_name = {
            let material = source
                .materials
                .get_mut(&rebind.material_id)
                .ok_or_else(|| {
                    invalid_error(format!(
                        "reviewed texture rebind references missing material {:?}",
                        rebind.material_id
                    ))
                })?;
            let mut matching = material
                .saved_properties
                .texture_envs
                .iter_mut()
                .filter(|environment| {
                    environment.slot == rebind.slot && environment.name == rebind.slot_name
                })
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return invalid(format!(
                    "reviewed texture rebind expected exactly one material {:?} slot {} {:?}, found {}",
                    rebind.material_id,
                    rebind.slot,
                    rebind.slot_name,
                    matching.len()
                ));
            }
            let environment = matching.pop().expect("one matching texture environment");
            if environment.texture_id.as_deref() != Some(&rebind.expected_texture_id)
                || environment.texture_pointer.source_asset_index
                    != rebind.expected_source_asset_index
                || environment.texture_pointer.file_id != rebind.expected_file_id
                || environment.texture_pointer.path_id != rebind.expected_path_id
                || environment.texture_pointer.is_null
            {
                return invalid(format!(
                    "reviewed texture rebind stale identity does not match material {:?} slot {} {:?}",
                    rebind.material_id, rebind.slot, rebind.slot_name
                ));
            }
            environment.texture_id = Some(replacement_id.clone());
            environment.texture_pointer = SourcePointer {
                source_asset_index: replacement_asset_index,
                file_id: 0,
                path_id: replacement_path_id,
                is_null: false,
            };
            material.name.clone()
        };

        source.textures.insert(replacement_id.clone(), replacement);
        entries.push(AppliedTextureRebindReport {
            material_id: rebind.material_id,
            material_name,
            slot: u64_count(rebind.slot, "reviewed texture slot")?,
            slot_name: rebind.slot_name,
            stale_texture_id: rebind.expected_texture_id,
            stale_source_asset_index: u64_count(
                rebind.expected_source_asset_index,
                "stale texture source asset index",
            )?,
            stale_file_id: rebind.expected_file_id,
            stale_path_id: rebind.expected_path_id,
            replacement_texture_id: replacement_id,
            replacement_texture_name: replacement_name,
            replacement_source_asset_index: u64_count(
                replacement_asset_index,
                "replacement texture source asset index",
            )?,
            replacement_path_id,
            replacement_texture_json: slash_path(&replacement_path),
            replacement_texture_json_sha256: sha256_hex(&replacement_bytes),
            reason: rebind.reason,
            evidence: rebind.evidence,
        });
    }

    let referenced_texture_ids = source
        .materials
        .values()
        .flat_map(|material| material.saved_properties.texture_envs.iter())
        .filter_map(|environment| environment.texture_id.clone())
        .collect::<BTreeSet<_>>();
    source
        .textures
        .retain(|texture_id, _| referenced_texture_ids.contains(texture_id));

    Ok(Some(ReviewedTextureRebindReport {
        schema: REVIEWED_TEXTURE_REBINDS_SCHEMA.to_string(),
        manifest_path: slash_path(manifest_path),
        manifest_sha256: sha256_hex(&manifest_bytes),
        entries,
    }))
}

pub(super) fn validate_dynamic_texture_source(
    dynamic: &SourceDynamicTexture,
    pointer: &SourcePointer,
    material_name: &str,
    slot: &str,
) -> Result<()> {
    validate_source_object(&dynamic.id, &dynamic.id, &dynamic.source, "MovieTexture")?;
    validate_pointer_identity(pointer, &dynamic.source, "dynamic material texture slot")?;
    if dynamic.object_type != "MovieTexture"
        || dynamic.name.trim().is_empty()
        || dynamic.container != "ogg-theora"
        || dynamic.mime_type != "video/ogg"
        || dynamic.payload.encoding != "base64-data-url"
        || dynamic.payload.byte_length == 0
        || dynamic.payload.sha256.len() != 64
        || !dynamic
            .payload
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return invalid(format!(
            "material {material_name:?} slot {slot:?} has contradictory MovieTexture metadata"
        ));
    }
    if dynamic.audio_clip_pointer.is_null != (dynamic.audio_clip_pointer.path_id == 0) {
        return invalid(format!(
            "material {material_name:?} slot {slot:?} has contradictory MovieTexture audio pointer"
        ));
    }
    const PREFIX: &str = "data:video/ogg;base64,";
    let encoded = dynamic
        .payload
        .data_url
        .strip_prefix(PREFIX)
        .ok_or_else(|| invalid_error("MovieTexture payload is not a strict video/ogg data URL"))?;
    if encoded.is_empty() || encoded.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return invalid("MovieTexture base64 payload is empty or contains whitespace");
    }
    let bytes = BASE64_STANDARD.decode(encoded).map_err(|error| {
        invalid_error(format!("MovieTexture base64 payload is invalid: {error}"))
    })?;
    if u64_count(bytes.len(), "MovieTexture payload byte length")? != dynamic.payload.byte_length
        || sha256_hex(&bytes) != dynamic.payload.sha256
        || !bytes.starts_with(b"OggS")
    {
        return invalid(format!(
            "material {material_name:?} slot {slot:?} MovieTexture bytes contradict length/SHA/Ogg container"
        ));
    }
    Ok(())
}

pub(super) fn native_dynamic_texture_binding(dynamic: &SourceDynamicTexture) -> Result<DynamicTextureBinding> {
    Ok(DynamicTextureBinding {
        source_id: dynamic.id.clone(),
        source_asset_index: u64_count(
            dynamic.source.asset_index,
            "MovieTexture source asset index",
        )?,
        source_path_id: dynamic.source.path_id,
        source_name: dynamic.name.clone(),
        object_type: dynamic.object_type.clone(),
        container: dynamic.container.clone(),
        mime_type: dynamic.mime_type.clone(),
        looped: dynamic.looped,
        audio_clip_pointer: DynamicTexturePointer {
            source_asset_index: u64_count(
                dynamic.audio_clip_pointer.source_asset_index,
                "MovieTexture audio source asset index",
            )?,
            file_id: dynamic.audio_clip_pointer.file_id,
            path_id: dynamic.audio_clip_pointer.path_id,
            is_null: dynamic.audio_clip_pointer.is_null,
        },
        byte_length: dynamic.payload.byte_length,
        sha256: dynamic.payload.sha256.clone(),
        data_url: dynamic.payload.data_url.clone(),
    })
}

pub(super) fn validate_texture_source(texture_id: &str, texture: &SourceTexture) -> Result<()> {
    validate_source_object(texture_id, &texture.id, &texture.source, "Texture2D")?;
    if texture.name.trim().is_empty()
        || texture.width == 0
        || texture.height == 0
        || texture.mip_count == 0
        || texture.mip_count_evidence.is_null()
    {
        return invalid(format!(
            "texture {texture_id:?} lacks exact name/dimensions/mip evidence"
        ));
    }
    let expected_format_name =
        exact_texture_format_name(texture.texture_format).ok_or_else(|| {
            invalid_error(format!(
                "texture {:?} TextureFormat {} has no exact raw mip decoder",
                texture.name, texture.texture_format
            ))
        })?;
    if texture.texture_format_name != expected_format_name {
        return invalid(format!(
            "texture {:?} TextureFormat name {:?} contradicts numeric format {}",
            texture.name, texture.texture_format_name, texture.texture_format
        ));
    }
    if texture.source_payload.byte_length == 0
        || !is_sha256(&texture.source_payload.sha256)
        || texture.source_payload.embedded_in_exact_source
        || texture.source_payload.layout != "largest-to-smallest-contiguous"
        || texture.source_payload.level_count != texture.mip_count as usize
        || texture.mip_levels.len() != texture.mip_count as usize
    {
        return invalid(format!(
            "texture {:?} source chain provenance is incomplete or contradictory",
            texture.name
        ));
    }
    let max_mips = 32 - texture.width.max(texture.height).leading_zeros();
    if texture.mip_count > max_mips {
        return invalid(format!(
            "texture {:?} declares more mip levels than its dimensions allow",
            texture.name
        ));
    }
    let mut width = texture.width;
    let mut height = texture.height;
    let mut source_offset = 0usize;
    for (level_index, level) in texture.mip_levels.iter().enumerate() {
        let expected_level = u32::try_from(level_index)
            .map_err(|_| invalid_error("source mip level index overflow"))?;
        let expected_source_length =
            exact_texture_mip_byte_length(texture.texture_format, width, height)
                .ok_or_else(|| invalid_error("source mip byte length overflow"))?;
        let expected_rgba_length = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| invalid_error("decoded source mip byte length overflow"))?;
        if level.level != expected_level
            || level.width != width
            || level.height != height
            || level.source_byte_offset != source_offset
            || level.source_byte_length != expected_source_length
            || !is_sha256(&level.source_byte_sha256)
            || level.decoded_rgba_byte_length != expected_rgba_length
            || !is_sha256(&level.decoded_rgba_sha256)
            || level.payload.decoded_rgba_sha256 != level.decoded_rgba_sha256
        {
            return invalid(format!(
                "texture {:?} mip {level_index} order/dimensions/source/decoded provenance is invalid",
                texture.name
            ));
        }
        source_offset = source_offset
            .checked_add(level.source_byte_length)
            .ok_or_else(|| invalid_error("source mip chain byte length overflow"))?;
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    if source_offset != texture.source_payload.byte_length {
        return invalid(format!(
            "texture {:?} mip slices do not cover its exact source payload",
            texture.name
        ));
    }
    let base = texture
        .mip_levels
        .first()
        .ok_or_else(|| invalid_error("texture has no base mip level"))?;
    if texture.payload.kind != base.payload.kind
        || texture.payload.mime_type != base.payload.mime_type
        || texture.payload.data_url != base.payload.data_url
        || texture.payload.byte_length != base.payload.byte_length
        || texture.payload.sha256 != base.payload.sha256
        || texture.payload.decoded_rgba_sha256 != base.payload.decoded_rgba_sha256
        || texture.payload.pixel_transform != base.payload.pixel_transform
        || texture.payload.resized != base.payload.resized
        || texture.payload.rgb_repair != base.payload.rgb_repair
        || texture.payload.alpha_mask_applied != base.payload.alpha_mask_applied
        || texture.payload.tint_applied != base.payload.tint_applied
    {
        return invalid(format!(
            "texture {:?} payload is not identical to mipLevels[0]",
            texture.name
        ));
    }
    for (name, source_name, serialized) in [
        (
            "filterMode",
            texture.sampler.filter_mode.source.as_str(),
            texture.sampler.filter_mode.serialized,
        ),
        (
            "wrapMode",
            texture.sampler.wrap_mode.source.as_str(),
            texture.sampler.wrap_mode.serialized,
        ),
        (
            "aniso",
            texture.sampler.aniso.source.as_str(),
            texture.sampler.aniso.serialized,
        ),
        (
            "mipBias",
            texture.sampler.mip_bias.source.as_str(),
            texture.sampler.mip_bias.serialized,
        ),
    ] {
        let is_default = source_name == "unity-texture-serialized-default";
        if source_name.is_empty() || serialized == is_default {
            return invalid(format!(
                "texture {:?} sampler {name} has contradictory serialized provenance",
                texture.name
            ));
        }
    }
    Ok(())
}

pub(super) fn exact_texture_mip_byte_length(format: i32, width: u32, height: u32) -> Option<usize> {
    let pixels = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?;
    match format {
        1 => Some(pixels),
        3 => pixels.checked_mul(3),
        4 | 5 | 14 => pixels.checked_mul(4),
        2 | 7 | 13 => pixels.checked_mul(2),
        10 => usize::try_from(width.div_ceil(4))
            .ok()?
            .checked_mul(usize::try_from(height.div_ceil(4)).ok()?)?
            .checked_mul(8),
        11 | 12 => usize::try_from(width.div_ceil(4))
            .ok()?
            .checked_mul(usize::try_from(height.div_ceil(4)).ok()?)?
            .checked_mul(16),
        _ => None,
    }
}

pub(super) fn texture_color_space(slot: &str) -> TextureColorSpace {
    match slot.to_ascii_lowercase().as_str() {
        "_bumpmap" | "_normalmap" | "_shadermap" | "_specmap" | "_masktex" => {
            TextureColorSpace::Linear
        }
        _ => TextureColorSpace::Srgb,
    }
}
