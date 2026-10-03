use super::*;

pub(super) fn summarize_texture(env: &UnityEnvironment, path_id: i64, body: &UnityValue) -> JsonValue {
    let decoded = decode_texture(env, body);
    let data_url = decoded
        .as_ref()
        .and_then(|texture| texture.image())
        .and_then(|mut image| {
            image::imageops::flip_vertical_in_place(&mut image);
            image_to_data_url(image, 96, false)
        });
    json!({
        "pathId": path_id,
        "name": object_name(body),
        "width": body.get("m_Width").and_then(UnityValue::as_i64),
        "height": body.get("m_Height").and_then(UnityValue::as_i64),
        "format": body.get("m_TextureFormat").and_then(UnityValue::as_i64),
        "mipCount": body.get("m_MipCount").and_then(UnityValue::as_i64),
        "dataSize": body.get("image data").and_then(UnityValue::as_bytes).map(|value| value.len()),
        "previewDataUrl": data_url,
        "hasAlpha": decoded.as_ref().is_some_and(|texture| texture.has_alpha()),
        "hasPartialAlpha": decoded.as_ref().is_some_and(|texture| texture.has_partial_alpha()),
        "decoded": decoded.is_some(),
    })
}
