use crate::fusionforge::{
    decode_texture, object_name, Asset, Pointer, UnityEnvironment, UnityValue,
};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

const SCHEMA: &str = "ffone.offline.chartexture-metadata.v1";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetadataDocument {
    schema: &'static str,
    source_path: String,
    source_file_bytes: u64,
    source_file_sha256: String,
    source_asset: String,
    raw_bundle: Option<SourceBundleProof>,
    texture_count: usize,
    unreadable_texture_count: usize,
    textures: Vec<TextureMetadata>,
    unreadable_textures: Vec<UnreadableTexture>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceBundleProof {
    source_path: String,
    byte_length: u64,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextureMetadata {
    true_name: String,
    path_id: i64,
    container_routes: Vec<String>,
    width: u32,
    height: u32,
    texture_format: i32,
    complete_image_size: u64,
    source_chain_sha256: String,
    native_png_bytes: u64,
    native_png_blake3: String,
    mip_map: bool,
    source_mip_count: u32,
    image_count: u32,
    texture_dimension: i32,
    filter_mode: i32,
    wrap_mode: i32,
    anisotropy_level: i32,
    mip_map_bias: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UnreadableTexture {
    path_id: i64,
    error: String,
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let mut args = args.iter().map(std::ffi::OsString::from);
    let source = args.next().map(PathBuf::from).ok_or_else(|| {
        "usage: fusionforge chartexture-metadata <asset> <out.json> [--raw-bundle <resourceFile>] [--png-dir <dir> [--png-name <trueName>] ...]"
            .to_owned()
    })?;
    let output = args.next().map(PathBuf::from).ok_or_else(|| {
        "usage: fusionforge chartexture-metadata <asset> <out.json> [--raw-bundle <resourceFile>] [--png-dir <dir> [--png-name <trueName>] ...]"
            .to_owned()
    })?;
    let mut raw_bundle = None;
    let mut png_dir = None;
    let mut png_names = BTreeSet::new();
    while let Some(flag) = args.next() {
        if flag == "--raw-bundle" {
            raw_bundle = Some(
                args.next()
                    .map(PathBuf::from)
                    .ok_or_else(|| "--raw-bundle requires a path".to_owned())?,
            );
        } else if flag == "--png-dir" {
            png_dir = Some(
                args.next()
                    .map(PathBuf::from)
                    .ok_or_else(|| "--png-dir requires a path".to_owned())?,
            );
        } else if flag == "--png-name" {
            let name = args
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or_else(|| "--png-name requires a UTF-8 trueName".to_owned())?;
            png_names.insert(name);
        } else {
            return Err(format!("unexpected argument {flag:?}"));
        }
    }

    let source_bytes =
        fs::read(&source).map_err(|error| format!("{}: {error}", source.display()))?;
    let raw_bundle = raw_bundle
        .map(|path| {
            let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            Ok::<_, String>(SourceBundleProof {
                source_path: path.to_string_lossy().replace('\\', "/"),
                byte_length: bytes.len() as u64,
                sha256: format!("{:x}", Sha256::digest(&bytes)),
            })
        })
        .transpose()?;
    let asset = Asset::from_path(&source)?;
    let container_routes = collect_container_routes(&asset)?;
    let environment = UnityEnvironment::from_assets(vec![asset]);
    let asset = environment
        .assets
        .first()
        .ok_or_else(|| "Texture2D environment lost its source asset".to_owned())?;
    let mut textures = Vec::new();
    let mut unreadable_textures = Vec::new();

    for (path_id, info) in &asset.objects {
        // Some Unity 2.x serialized files carry the canonical Texture2D
        // classId (28) through a type tree whose display name is only
        // `Texture`. Container-owned NPC variants in Retro_shared use this
        // exact shape, so class identity is the authoritative discriminator.
        if info.class_id != 28 && asset.object_type_name(info) != "Texture2D" {
            continue;
        }
        let body = match asset.read_object(0, info) {
            Ok(body) => body,
            Err(error) => {
                unreadable_textures.push(UnreadableTexture {
                    path_id: *path_id,
                    error,
                });
                continue;
            }
        };
        match texture_metadata(*path_id, &body, &container_routes, &environment) {
            Ok((metadata, native_png)) => {
                if png_names.is_empty() || png_names.contains(&metadata.true_name) {
                    if let Some(png_dir) = png_dir.as_ref() {
                        fs::create_dir_all(png_dir)
                            .map_err(|error| format!("{}: {error}", png_dir.display()))?;
                        let png_path = png_dir.join(format!(
                            "{}--{}.png",
                            metadata.path_id, metadata.native_png_blake3
                        ));
                        fs::write(&png_path, native_png)
                            .map_err(|error| format!("{}: {error}", png_path.display()))?;
                    }
                }
                textures.push(metadata);
            }
            Err(error) => unreadable_textures.push(UnreadableTexture {
                path_id: *path_id,
                error,
            }),
        }
    }
    textures.sort_by(|left, right| {
        left.true_name
            .to_ascii_lowercase()
            .cmp(&right.true_name.to_ascii_lowercase())
            .then_with(|| left.path_id.cmp(&right.path_id))
    });
    let document = MetadataDocument {
        schema: SCHEMA,
        source_path: source.to_string_lossy().replace('\\', "/"),
        source_file_bytes: source_bytes.len() as u64,
        source_file_sha256: format!("{:x}", Sha256::digest(&source_bytes)),
        source_asset: asset.name.clone(),
        raw_bundle,
        texture_count: textures.len(),
        unreadable_texture_count: unreadable_textures.len(),
        textures,
        unreadable_textures,
    };
    let bytes = serde_json::to_vec_pretty(&document).map_err(|error| error.to_string())?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    fs::write(&output, bytes).map_err(|error| format!("{}: {error}", output.display()))?;
    println!(
        "{}: wrote {} exact Texture2D metadata records ({} unreadable)",
        output.display(),
        document.texture_count,
        document.unreadable_texture_count
    );
    Ok(())
}

fn texture_metadata(
    path_id: i64,
    body: &UnityValue,
    container_routes: &BTreeMap<i64, Vec<String>>,
    environment: &UnityEnvironment,
) -> Result<(TextureMetadata, Vec<u8>), String> {
    let true_name = object_name(body);
    if true_name.is_empty() {
        return Err(format!("Texture2D pathId {path_id} has an empty m_Name"));
    }
    let value = body
        .as_object()
        .ok_or_else(|| format!("Texture2D {true_name} body is not an object"))?;
    let image = exact_bytes(value.get("image data"), &true_name)?;
    let complete_image_size = exact_u64(value.get("m_CompleteImageSize"), "m_CompleteImageSize")?;
    validate_complete_image_size(&true_name, complete_image_size, image.len() as u64)?;
    let width = exact_u32(value.get("m_Width"), "m_Width")?;
    let height = exact_u32(value.get("m_Height"), "m_Height")?;
    let mip_map = exact_bool(value.get("m_MipMap"), "m_MipMap")?;
    let source_mip_count = if mip_map {
        u32::BITS - width.max(height).leading_zeros()
    } else {
        1
    };
    let settings = value
        .get("m_TextureSettings")
        .and_then(UnityValue::as_object)
        .ok_or_else(|| format!("Texture2D {true_name} has no m_TextureSettings"))?;
    let decoded = decode_texture(environment, body)
        .ok_or_else(|| format!("Texture2D {true_name} cannot be decoded exactly"))?;
    let mut native_image = decoded
        .image()
        .ok_or_else(|| format!("Texture2D {true_name} has invalid decoded RGBA dimensions"))?;
    image::imageops::flip_vertical_in_place(&mut native_image);
    let mut native_png = Vec::new();
    PngEncoder::new(&mut native_png)
        .write_image(
            native_image.as_raw(),
            native_image.width(),
            native_image.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|error| format!("Texture2D {true_name} PNG encoding failed: {error}"))?;
    let metadata = TextureMetadata {
        true_name,
        path_id,
        container_routes: container_routes.get(&path_id).cloned().unwrap_or_default(),
        width,
        height,
        texture_format: exact_i32(value.get("m_TextureFormat"), "m_TextureFormat")?,
        complete_image_size,
        source_chain_sha256: format!("{:x}", Sha256::digest(&image)),
        native_png_bytes: native_png.len() as u64,
        native_png_blake3: blake3::hash(&native_png).to_hex().to_string(),
        mip_map,
        source_mip_count,
        image_count: exact_u32(value.get("m_ImageCount"), "m_ImageCount")?,
        texture_dimension: exact_i32(value.get("m_TextureDimension"), "m_TextureDimension")?,
        filter_mode: exact_i32(settings.get("m_FilterMode"), "m_FilterMode")?,
        wrap_mode: exact_i32(settings.get("m_WrapMode"), "m_WrapMode")?,
        anisotropy_level: exact_i32(settings.get("m_Aniso"), "m_Aniso")?,
        mip_map_bias: exact_f64(settings.get("m_MipBias"), "m_MipBias")?,
    };
    Ok((metadata, native_png))
}

fn collect_container_routes(asset: &Asset) -> Result<BTreeMap<i64, Vec<String>>, String> {
    let Some(info) = asset
        .objects
        .values()
        .find(|info| asset.object_type_name(info) == "AssetBundle")
    else {
        return Err("source asset has no AssetBundle container object".to_owned());
    };
    let body = asset.read_object(0, info)?;
    let container = body
        .get("m_Container")
        .and_then(UnityValue::as_array)
        .ok_or_else(|| "AssetBundle has no m_Container array".to_owned())?;
    let mut routes = BTreeMap::<i64, BTreeSet<String>>::new();
    for entry in container {
        let UnityValue::Pair(route, metadata) = entry else {
            continue;
        };
        let Some(route) = route.as_str() else {
            continue;
        };
        let Some(pointer) = metadata.get("asset").and_then(pointer_or_null) else {
            continue;
        };
        if pointer.file_id == 0 {
            routes
                .entry(pointer.path_id)
                .or_default()
                .insert(route.replace('\\', "/"));
        }
    }
    Ok(routes
        .into_iter()
        .map(|(path_id, routes)| (path_id, routes.into_iter().collect()))
        .collect())
}

fn pointer_or_null(value: &UnityValue) -> Option<&Pointer> {
    match value {
        UnityValue::Pointer(pointer) => Some(pointer),
        _ => None,
    }
}

fn exact_bytes(value: Option<&UnityValue>, label: &str) -> Result<Vec<u8>, String> {
    match value {
        Some(UnityValue::Bytes(bytes)) => Ok(bytes.clone()),
        Some(UnityValue::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_i64()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| format!("Texture2D {label} image payload is not byte-exact"))
            })
            .collect(),
        _ => Err(format!("Texture2D {label} has no byte-exact image payload")),
    }
}

fn exact_i64(value: Option<&UnityValue>, label: &str) -> Result<i64, String> {
    match value {
        Some(UnityValue::Int(value)) => Ok(*value),
        Some(UnityValue::UInt(value)) => {
            i64::try_from(*value).map_err(|_| format!("{label} does not fit i64"))
        }
        _ => Err(format!("{label} is not an exact integer")),
    }
}

fn exact_i32(value: Option<&UnityValue>, label: &str) -> Result<i32, String> {
    i32::try_from(exact_i64(value, label)?).map_err(|_| format!("{label} does not fit i32"))
}

fn exact_u32(value: Option<&UnityValue>, label: &str) -> Result<u32, String> {
    u32::try_from(exact_i64(value, label)?).map_err(|_| format!("{label} does not fit u32"))
}

fn exact_u64(value: Option<&UnityValue>, label: &str) -> Result<u64, String> {
    u64::try_from(exact_i64(value, label)?).map_err(|_| format!("{label} does not fit u64"))
}

fn exact_f64(value: Option<&UnityValue>, label: &str) -> Result<f64, String> {
    match value {
        Some(UnityValue::Float(value)) if value.is_finite() => Ok(*value),
        _ => Err(format!("{label} is not an exact finite float")),
    }
}

fn exact_bool(value: Option<&UnityValue>, label: &str) -> Result<bool, String> {
    match value {
        Some(UnityValue::Bool(value)) => Ok(*value),
        _ => Err(format!("{label} is not an exact bool")),
    }
}

fn validate_complete_image_size(
    true_name: &str,
    complete_image_size: u64,
    payload_size: u64,
) -> Result<(), String> {
    // Unity 2.x assets are not consistent about this field: some serialize the
    // complete mip-chain size, while others serialize only the base-level size
    // even though `image data` contains every mip (and occasionally additional
    // image faces). A few non-square DXT1 textures in the clean primary
    // CharTexture bundle instead store a decoded-footprint-like value that is
    // larger than the encoded payload. The payload itself remains byte-exact,
    // is fully hashed below, and must still decode successfully. Treat the
    // legacy size field as evidence rather than a byte-range boundary.
    if complete_image_size == 0 || payload_size == 0 {
        return Err(format!(
            "Texture2D {true_name} has an empty complete image size ({complete_image_size}) or payload ({payload_size})"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
