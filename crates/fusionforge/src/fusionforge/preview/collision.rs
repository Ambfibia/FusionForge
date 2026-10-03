use super::*;

pub fn summarize_collider(path_id: i64, obj_type: &str, body: &UnityValue) -> JsonValue {
    let center = vector(body.get("m_Center"))
        .map(unity_to_native_vec3)
        .map(|value| json!({ "x": value.0, "y": value.1, "z": value.2 }));
    let size = vector(body.get("m_Size"))
        .map(unity_to_native_scale)
        .map(|value| json!({ "x": value.0, "y": value.1, "z": value.2 }));
    json!({
        "coordinateSpace": "native",
        "coordinateContract": native_coordinate_contract_json(),
        "pathId": path_id,
        "type": obj_type,
        "name": object_name(body),
        "gameObject": pointer_summary(body.get("m_GameObject")),
        "isTrigger": body.get("m_IsTrigger").and_then(UnityValue::as_i64).unwrap_or(0) != 0,
        "center": center,
        "size": size,
        "radius": body.get("m_Radius").and_then(UnityValue::as_f64),
        "height": body.get("m_Height").and_then(UnityValue::as_f64),
        "mesh": pointer_summary(body.get("m_Mesh")),
        "terrainData": pointer_summary(body.get("m_TerrainData")),
        "material": pointer_summary(body.get("m_Material")),
    })
}
