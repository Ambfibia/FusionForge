use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureImportFormat {
    Rgba32,
}

impl TextureImportFormat {
    pub fn unity_format(self) -> i32 {
        match self {
            Self::Rgba32 => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportedTexture {
    pub name: Option<String>,
    pub width: u32,
    pub height: u32,
    pub format: TextureImportFormat,
    pub rgba: Vec<u8>,
    pub clamp: bool,
}

impl ImportedTexture {
    pub fn from_png_path(path: &Path, name: Option<String>, clamp: bool) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
        Self::from_png_bytes(&bytes, name, clamp)
    }

    pub fn from_png_bytes(bytes: &[u8], name: Option<String>, clamp: bool) -> Result<Self, String> {
        let image = image::load_from_memory(bytes)
            .map_err(|err| format!("could not decode texture image: {err}"))?;
        let (width, height) = image.dimensions();
        let mut rgba = image.to_rgba8();
        image::imageops::flip_vertical_in_place(&mut rgba);
        Ok(Self {
            name,
            width,
            height,
            format: TextureImportFormat::Rgba32,
            rgba: rgba.into_raw(),
            clamp,
        })
    }

    pub fn from_gltf_base_color_path(
        path: &Path,
        name: Option<String>,
        clamp: bool,
    ) -> Result<Option<Self>, String> {
        match path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "glb" | "gltf" => {}
            _ => return Ok(None),
        }
        let (document, _, images) =
            gltf::import(path).map_err(|err| format!("{}: {err}", path.display()))?;
        let Some(texture_index) = document.materials().find_map(|material| {
            material
                .pbr_metallic_roughness()
                .base_color_texture()
                .map(|texture| texture.texture().source().index())
        }) else {
            return Ok(None);
        };
        let Some(image) = images.get(texture_index) else {
            return Ok(None);
        };
        Ok(Some(Self::from_gltf_image(image, name, clamp)?))
    }

    pub(super) fn from_gltf_image(
        image: &gltf::image::Data,
        name: Option<String>,
        clamp: bool,
    ) -> Result<Self, String> {
        let mut rgba = gltf_image_to_rgba(image)?;
        image::imageops::flip_vertical_in_place(&mut rgba);
        Ok(Self {
            name,
            width: image.width,
            height: image.height,
            format: TextureImportFormat::Rgba32,
            rgba: rgba.into_raw(),
            clamp,
        })
    }
}

pub(super) fn gltf_image_to_rgba(image: &gltf::image::Data) -> Result<image::RgbaImage, String> {
    use gltf::image::Format;
    let width = image.width;
    let height = image.height;
    let pixels = &image.pixels;
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    match image.format {
        Format::R8G8B8A8 => rgba.extend_from_slice(pixels),
        Format::R8G8B8 => {
            for chunk in pixels.chunks_exact(3) {
                rgba.extend([chunk[0], chunk[1], chunk[2], 255]);
            }
        }
        Format::R8G8 => {
            for chunk in pixels.chunks_exact(2) {
                rgba.extend([chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
        }
        Format::R8 => {
            for value in pixels {
                rgba.extend([*value, *value, *value, 255]);
            }
        }
        other => {
            return Err(format!(
                "GLTF embedded texture format {:?} is not supported yet",
                other
            ));
        }
    }
    image::RgbaImage::from_raw(width, height, rgba).ok_or_else(|| {
        format!(
            "GLTF embedded texture had unexpected byte length for {}x{}",
            width, height
        )
    })
}

pub fn apply_texture_import(
    target: &mut UnityValue,
    texture: ImportedTexture,
) -> Result<(), String> {
    let object = target
        .as_object_mut()
        .ok_or_else(|| "target texture is not an object".to_string())?;
    if let Some(name) = texture.name {
        object.insert("m_Name".to_string(), UnityValue::String(name));
    }
    object.insert("m_Width".to_string(), UnityValue::Int(texture.width as i64));
    object.insert(
        "m_Height".to_string(),
        UnityValue::Int(texture.height as i64),
    );
    object.insert(
        "m_TextureFormat".to_string(),
        UnityValue::Int(texture.format.unity_format() as i64),
    );
    object.insert(
        "image data".to_string(),
        UnityValue::Bytes(texture.rgba.clone()),
    );
    object.insert(
        "m_CompleteImageSize".to_string(),
        UnityValue::Int(texture.rgba.len() as i64),
    );
    object.insert("m_ImageCount".to_string(), UnityValue::Int(1));
    object.insert("m_TextureDimension".to_string(), UnityValue::Int(2));
    object.insert("m_MipMap".to_string(), UnityValue::Bool(false));
    object.insert("m_MipCount".to_string(), UnityValue::Int(1));

    if let Some(stream_data) = object
        .get_mut("m_StreamData")
        .and_then(UnityValue::as_object_mut)
    {
        stream_data.insert("offset".to_string(), UnityValue::Int(0));
        stream_data.insert("size".to_string(), UnityValue::Int(0));
        stream_data.insert("path".to_string(), UnityValue::String(String::new()));
    }

    if let Some(settings) = object
        .get_mut("m_TextureSettings")
        .and_then(UnityValue::as_object_mut)
    {
        settings.insert("m_FilterMode".to_string(), UnityValue::Int(1));
        settings.insert("m_Aniso".to_string(), UnityValue::Int(4));
        settings.insert("m_MipBias".to_string(), UnityValue::Float(0.0));
        settings.insert(
            "m_WrapMode".to_string(),
            UnityValue::Int(if texture.clamp { 1 } else { 0 }),
        );
    }
    Ok(())
}
