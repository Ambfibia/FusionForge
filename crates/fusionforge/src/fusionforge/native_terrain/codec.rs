use super::*;

pub(super) fn encode_gray8_png(width: u32, height: u32, values: &[u8]) -> Result<Vec<u8>, String> {
    let image = GrayImage::from_raw(width, height, values.to_vec())
        .ok_or_else(|| format!("could not construct {width}x{height} Gray8 image"))?;
    encode_dynamic_png(DynamicImage::ImageLuma8(image))
}

pub(super) fn encode_gray16_png(width: u32, height: u32, values: &[u16]) -> Result<Vec<u8>, String> {
    let image = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(width, height, values.to_vec())
        .ok_or_else(|| format!("could not construct {width}x{height} Gray16 heightmap"))?;
    encode_dynamic_png(DynamicImage::ImageLuma16(image))
}

pub(super) fn encode_rgba8_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let image = RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| format!("could not construct {width}x{height} RGBA8 image"))?;
    encode_dynamic_png(DynamicImage::ImageRgba8(image))
}

pub(super) fn encode_dynamic_png(image: DynamicImage) -> Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|err| format!("could not encode native terrain PNG: {err}"))?;
    Ok(cursor.into_inner())
}
