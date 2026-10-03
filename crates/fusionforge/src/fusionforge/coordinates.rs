use serde_json::{json, Value as JsonValue};

/// Canonical authored-space conversion used by FusionForge previews, world
/// documents and native asset export. `H` is a reflection over X:
/// `H = diag(-1, 1, 1)`. It changes handedness without changing units,
/// authored origins or authored scale.
pub const NATIVE_COORDINATE_CONTRACT_SCHEMA: &str = "ffone.native-coordinate-contract.v1";
pub const NATIVE_COORDINATE_BASIS: &str = "H=diag(-1,1,1)";

pub type Vec3 = (f64, f64, f64);
pub type Quaternion = (f64, f64, f64, f64);

#[inline]
pub fn unity_to_native_vec3(value: Vec3) -> Vec3 {
    (-value.0, value.1, value.2)
}

#[inline]
pub fn unity_to_native_quaternion(value: Quaternion) -> Quaternion {
    (value.0, -value.1, -value.2, value.3)
}

#[inline]
pub fn unity_to_native_scale(value: Vec3) -> Vec3 {
    value
}

/// Metadata attached to world-space summaries so consumers cannot mistake
/// their values for raw Unity coordinates or apply the conversion twice.
pub fn native_coordinate_contract_json() -> JsonValue {
    json!({
        "schema": NATIVE_COORDINATE_CONTRACT_SCHEMA,
        "basis": NATIVE_COORDINATE_BASIS,
        "translation": "[-unity.x,unity.y,unity.z]",
        "rotation": "[unity.x,-unity.y,-unity.z,unity.w]",
        "scale": "unchanged",
        "unitScale": "1-unity-unit-equals-1-bevy-unit",
        "originPolicy": "source-trs-unchanged-no-auto-centering",
        "gameplayFacingRotationApplied": false,
    })
}

#[cfg(test)]
mod tests;
