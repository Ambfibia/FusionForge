use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColliderCoordinateContract {
    pub center: &'static str,
    pub size: &'static str,
    pub scalar_dimensions: &'static str,
    pub mesh_and_terrain_geometry: &'static str,
    pub gameplay_facing_rotation_applied: bool,
}

impl Default for ColliderCoordinateContract {
    fn default() -> Self {
        Self {
            center: "[-unity.x,unity.y,unity.z]",
            size: "unchanged",
            scalar_dimensions: "radius-and-height-unchanged",
            mesh_and_terrain_geometry: "same-H-contract-as-render-geometry",
            gameplay_facing_rotation_applied: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColliderException {
    pub asset: String,
    pub path_id: i64,
    pub collider_type: String,
    pub game_object: String,
    pub kinds: Vec<String>,
    pub native_center: Option<[f64; 3]>,
    pub unchanged_size: Option<[f64; 3]>,
    pub unchanged_radius: Option<f64>,
    pub unchanged_height: Option<f64>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn audit_collider(
    env: &UnityEnvironment,
    asset_name: &str,
    path_id: i64,
    collider_type: &str,
    body: &UnityValue,
    counts: &mut MapTransformAuditCounts,
    exceptions: &mut Vec<ColliderException>,
) {
    counts.colliders += 1;
    *counts
        .collider_types
        .entry(collider_type.to_string())
        .or_default() += 1;
    let center = vector(body.get("m_Center")).map(unity_to_native_vec3);
    let size = vector(body.get("m_Size")).map(unity_to_native_scale);
    let radius = body.get("m_Radius").and_then(UnityValue::as_f64);
    let height = body.get("m_Height").and_then(UnityValue::as_f64);
    if center.is_some() {
        counts.collider_centers += 1;
    }
    if size.is_some() {
        counts.collider_sizes += 1;
    }
    if radius.is_some() {
        counts.collider_radii += 1;
    }
    if height.is_some() {
        counts.collider_heights += 1;
    }
    let center = center.map(|value| [value.0, value.1, value.2]);
    let size = size.map(|value| [value.0, value.1, value.2]);
    let mut kinds = Vec::<String>::new();
    if center.is_some_and(|values| values.iter().any(|value| !value.is_finite())) {
        kinds.push("nonfiniteCenter".to_string());
    }
    if size.is_some_and(|values| values.iter().any(|value| !value.is_finite())) {
        kinds.push("nonfiniteSize".to_string());
    }
    if size.is_some_and(|values| values.iter().any(|value| *value <= 0.0)) {
        kinds.push("nonpositiveSize".to_string());
    }
    if radius.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        kinds.push("invalidRadius".to_string());
    }
    if height.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        kinds.push("invalidHeight".to_string());
    }
    if !kinds.is_empty() {
        counts.invalid_colliders += 1;
        exceptions.push(ColliderException {
            asset: asset_name.to_string(),
            path_id,
            collider_type: collider_type.to_string(),
            game_object: game_object_name(env, body),
            kinds,
            native_center: center,
            unchanged_size: size,
            unchanged_radius: radius,
            unchanged_height: height,
        });
    }
}
