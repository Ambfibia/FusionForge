use super::*;

pub(super) fn cook_texture(
    env: &UnityEnvironment,
    body: &UnityValue,
    name: &str,
    state: &mut CookState,
) -> Result<EmittedAsset, ObjectCookError> {
    let decoded = decode_texture(env, body).ok_or_else(|| {
        ObjectCookError::Object("Texture2D format or pixel payload is unsupported".to_string())
    })?;
    let mut image = decoded.image().ok_or_else(|| {
        ObjectCookError::Object("Texture2D RGBA dimensions are invalid".to_string())
    })?;
    image::imageops::flip_vertical_in_place(&mut image);
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|err| ObjectCookError::Object(err.to_string()))?;
    state
        .emit_asset(
            "textures",
            semantic_or(name, "texture"),
            "png",
            ContentKind::Texture,
            &png,
        )
        .map_err(ObjectCookError::Output)
}
