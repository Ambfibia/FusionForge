use super::*;

pub(super) fn summarize_terrain(path_id: i64, body: &UnityValue) -> JsonValue {
    let heightmap = body.get("m_Heightmap");
    let heights = value_array(heightmap.and_then(|value| value.get("m_Heights")));
    json!({
        "pathId": path_id,
        "name": object_name(body),
        "width": heightmap.and_then(|value| value.get("m_Width")).and_then(UnityValue::as_i64),
        "height": heightmap.and_then(|value| value.get("m_Height")).and_then(UnityValue::as_i64),
        "heightSamples": heights.len(),
        "minHeight": heights.iter().filter_map(UnityValue::as_f64).reduce(f64::min),
        "maxHeight": heights.iter().filter_map(UnityValue::as_f64).reduce(f64::max),
    })
}
