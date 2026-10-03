use std::{fs, path::Path};

use ffone_skinned_model::{
    MaterialTextureBinding, MaterialTextureSamplerBinding, NativeSampler, NativeTextureMipLevel,
    PublishedMipPolicy, PublishedPixelTransform, SamplerMagFilter, SamplerMinFilter,
    SamplerWrapMode, TextureColorSpace, TextureMipProvenance, TextureSourceMipLayout,
};
use serde_json::{Value, json};

const PRIMARY_CONTAINER: &str = "Tutorial.resourceFile";
const PRIMARY_CONTAINER_BYTES: u64 = 27_003_826;
const PRIMARY_CONTAINER_SHA256: &str =
    "49a684ff4236848d0b882a5d725ffbe99d5cfb5d2090dc8705350e97dd3fd024";
const PRIMARY_ASSET: &str = "CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a";

pub(super) fn run(command_args: &[String]) -> Result<(), String> {
    let args = command_args.iter().cloned().collect::<Vec<_>>();
    if args.len() < 3 || args.len() % 2 == 0 {
        return Err(
            "usage: publish-legacy-static-exact-mips-manifest <fresh-out.json> \
             <exact-report.json> <game-relative-base.png> [...]"
                .to_string(),
        );
    }
    let output = Path::new(&args[0]);
    if output.exists() {
        return Err(format!(
            "refusing to overwrite existing manifest {}",
            output.display()
        ));
    }
    let mut entries = Vec::new();
    for pair in args[1..].chunks_exact(2) {
        entries.push(entry(Path::new(&pair[0]), &pair[1])?);
    }
    entries.sort_by(|left, right| left["textureId"].as_str().cmp(&right["textureId"].as_str()));

    let manifest = json!({
        "schema": "ffone.legacy-static-exact-mips.v1",
        "source": {
            "alias": "primary",
            "relativeContainerPath": PRIMARY_CONTAINER,
            "byteLength": PRIMARY_CONTAINER_BYTES,
            "sha256": PRIMARY_CONTAINER_SHA256,
            "asset": PRIMARY_ASSET,
        },
        "extractor": {
            "name": "FusionForge",
            "version": "0.1.0",
            "command": "fusionforge fusionforge export-exact-texture <primary>/Tutorial.resourceFile <pathId> <out.json>",
        },
        "publisher": {
            "name": "ffone-asset-pipeline/publish_exact_texture_mips",
            "version": env!("CARGO_PKG_VERSION"),
            "command": "cargo run -p ffone-asset-pipeline --bin publish_exact_texture_mips -- <exact-report.json> <existing-base.png>",
        },
        "textures": entries,
    });
    let bytes = format!(
        "{}\n",
        serde_json::to_string_pretty(&manifest)
            .map_err(|error| format!("cannot serialize manifest: {error}"))?
    );
    fs::write(output, bytes)
        .map_err(|error| format!("cannot write manifest {}: {error}", output.display()))?;
    println!(
        "Published {} exact static texture bindings to {}",
        entries.len(),
        output.display()
    );
    Ok(())
}

fn entry(report_path: &Path, base_uri: &str) -> Result<Value, String> {
    let bytes = fs::read(report_path).map_err(|error| {
        format!(
            "cannot read exact report {}: {error}",
            report_path.display()
        )
    })?;
    let report: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid exact report {}: {error}", report_path.display()))?;
    let texture_id = required_str(&report, "id")?;
    let expected_prefix = format!("{PRIMARY_ASSET}:");
    let path_id = texture_id
        .strip_prefix(&expected_prefix)
        .ok_or_else(|| format!("texture id {texture_id:?} is not owned by {PRIMARY_ASSET}"))?
        .parse::<u32>()
        .map_err(|error| format!("texture id {texture_id:?} has invalid pathId: {error}"))?;
    let source_name = required_str(&report, "name")?.to_string();
    let source_mip_count = required_u32(&report, "mipCount")?;
    let source_texture_format = required_i32(&report, "textureFormat")?;
    let source_texture_format_name = required_str(&report, "textureFormatName")?.to_string();
    let raw_source = report
        .get("sourcePayload")
        .ok_or_else(|| "exact report sourcePayload is missing".to_string())?;
    let source_chain_byte_length = required_u64(raw_source, "byteLength")?;
    let source_chain_sha256 = required_str(raw_source, "sha256")?.to_string();
    if required_u32(raw_source, "levelCount")? != source_mip_count {
        return Err(format!(
            "texture {texture_id} rawSource levelCount is incomplete"
        ));
    }

    let sampler = report
        .get("sampler")
        .ok_or_else(|| "exact report sampler is missing".to_string())?;
    let filter_mode = setting_i32(sampler, "filterMode")?;
    let wrap_mode = setting_i32(sampler, "wrapMode")?;
    let anisotropy_level = setting_u32(sampler, "aniso")?;
    let mip_map_bias = setting_f64(sampler, "mipBias")?;
    if filter_mode != 1 || wrap_mode != 0 || anisotropy_level != 1 || mip_map_bias != 0.0 {
        return Err(format!(
            "texture {texture_id} sampler is not the audited FilterMode=1/WrapMode=0/Aniso=1/MipBias=0 contract"
        ));
    }

    let source_levels = report
        .get("mipLevels")
        .and_then(Value::as_array)
        .ok_or_else(|| "exact report mipLevels is missing".to_string())?;
    if source_levels.len() != source_mip_count as usize || source_levels.len() < 2 {
        return Err(format!(
            "texture {texture_id} exact mip chain is incomplete"
        ));
    }
    let base_path = Path::new(base_uri);
    if base_path.extension().and_then(|value| value.to_str()) != Some("png") {
        return Err(format!(
            "base URI {base_uri:?} is not a lowercase .png path"
        ));
    }
    let base_stem = base_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("base URI {base_uri:?} has no UTF-8 stem"))?;
    let parent = base_path
        .parent()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty());
    let mut mip_levels = Vec::with_capacity(source_levels.len());
    for (index, level) in source_levels.iter().enumerate() {
        if required_u32(level, "level")? != index as u32 {
            return Err(format!(
                "texture {texture_id} mip levels are not contiguous"
            ));
        }
        let payload = level
            .get("payload")
            .ok_or_else(|| format!("texture {texture_id} mip {index} payload is missing"))?;
        if required_str(payload, "pixelTransform")? != "vertical-flip-only-for-png-top-left-origin"
        {
            return Err(format!(
                "texture {texture_id} mip {index} pixel transform changed"
            ));
        }
        let uri = if index == 0 {
            base_uri.to_string()
        } else if let Some(parent) = parent {
            format!("{parent}/{base_stem}.mips/mip-{index:02}.png")
        } else {
            format!("{base_stem}.mips/mip-{index:02}.png")
        };
        mip_levels.push(NativeTextureMipLevel {
            level: index as u32,
            width: required_u32(level, "width")?,
            height: required_u32(level, "height")?,
            uri,
            source_byte_offset: required_u64(level, "sourceByteOffset")?,
            source_byte_length: required_u64(level, "sourceByteLength")?,
            source_byte_sha256: required_str(level, "sourceByteSha256")?.to_string(),
            decoded_rgba8_byte_length: required_u64(level, "decodedRgbaByteLength")?,
            decoded_rgba8_sha256: required_str(level, "decodedRgbaSha256")?.to_string(),
            png_byte_length: required_u64(payload, "byteLength")?,
            png_sha256: required_str(payload, "sha256")?.to_string(),
        });
    }

    let binding = MaterialTextureBinding {
        slot: "_MainTex".to_string(),
        unassigned_stale_null: false,
        ignored_stale_shader_binding: false,
        dynamic_texture: None,
        texture: Some(path_id),
        source_name: Some(source_name.clone()),
        uri: Some(base_uri.to_string()),
        sampler: Some(MaterialTextureSamplerBinding {
            index: 0,
            descriptor: NativeSampler {
                name: source_name,
                mag_filter: SamplerMagFilter::Linear,
                min_filter: SamplerMinFilter::LinearMipmapNearest,
                wrap_s: SamplerWrapMode::Repeat,
                wrap_t: SamplerWrapMode::Repeat,
                legacy_filter_mode: filter_mode,
                legacy_wrap_mode: wrap_mode,
                anisotropy_level,
                mip_map_bias,
            },
        }),
        mip_provenance: Some(TextureMipProvenance {
            source_texture_format,
            source_texture_format_name,
            source_mip_count,
            source_chain_byte_length,
            source_chain_sha256,
            source_chain_complete: true,
            source_layout: TextureSourceMipLayout::LargestToSmallestContiguous,
            published_pixel_transform: PublishedPixelTransform::VerticalFlipOnlyForPngTopLeftOrigin,
            published_policy: PublishedMipPolicy::ExactSourceLevels,
        }),
        mip_levels: Some(mip_levels),
        scale: [1.0, 1.0],
        offset: [0.0, 0.0],
        pivot: Some([0.0, 0.0]),
        rotation: Some(0.0),
        color_space: TextureColorSpace::Srgb,
    };
    Ok(json!({
        "textureId": texture_id,
        "sourceObjectPathId": path_id,
        "binding": binding,
    }))
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("exact report field {field:?} is missing or not a string"))
}

fn required_u64(value: &Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("exact report field {field:?} is missing or not a u64"))
}

fn required_u32(value: &Value, field: &str) -> Result<u32, String> {
    u32::try_from(required_u64(value, field)?)
        .map_err(|_| format!("exact report field {field:?} does not fit u32"))
}

fn required_i32(value: &Value, field: &str) -> Result<i32, String> {
    let value = value
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("exact report field {field:?} is missing or not an i64"))?;
    i32::try_from(value).map_err(|_| format!("exact report field {field:?} does not fit i32"))
}

fn setting<'a>(value: &'a Value, field: &str) -> Result<&'a Value, String> {
    value
        .get(field)
        .and_then(|setting| setting.get("value"))
        .ok_or_else(|| format!("exact report sampler {field:?}.value is missing"))
}

fn setting_i32(value: &Value, field: &str) -> Result<i32, String> {
    let value = setting(value, field)?
        .as_i64()
        .ok_or_else(|| format!("exact report sampler {field:?}.value is not an i64"))?;
    i32::try_from(value).map_err(|_| format!("exact report sampler {field:?} does not fit i32"))
}

fn setting_u32(value: &Value, field: &str) -> Result<u32, String> {
    let value = setting(value, field)?
        .as_u64()
        .ok_or_else(|| format!("exact report sampler {field:?}.value is not a u64"))?;
    u32::try_from(value).map_err(|_| format!("exact report sampler {field:?} does not fit u32"))
}

fn setting_f64(value: &Value, field: &str) -> Result<f64, String> {
    setting(value, field)?
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("exact report sampler {field:?}.value is not a finite f64"))
}
