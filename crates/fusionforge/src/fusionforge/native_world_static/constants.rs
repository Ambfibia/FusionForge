use super::*;

pub(super) const HIERARCHY_SCHEMA: &str = "ffone.native-static-world-hierarchy.v1";

pub(super) const BEHAVIOUR_SCHEMA: &str = "ffone.native-static-world-behaviours.v1";

pub(super) const SCENE_SCHEMA: &str = "ffone.native-world-scene.v2";

pub(super) const IDENTITY_TRANSFORM: JsonTransform = JsonTransform {
    translation: [0.0, 0.0, 0.0],
    rotation: [0.0, 0.0, 0.0, 1.0],
    scale: [1.0, 1.0, 1.0],
};

/// Scene components whose exact serialized state is published beside the
/// geometry. `MeshFilter`, `MeshRenderer`, `SkinnedMeshRenderer`,
/// `MeshCollider` and `Transform` are deliberately absent: they are already
/// fully represented by the GLB payloads and the hierarchy.
pub(super) const BEHAVIOUR_COMPONENT_TYPES: [&str; 6] = [
    "Animation",
    "BoxCollider",
    "CapsuleCollider",
    "MonoBehaviour",
    "Rigidbody",
    "SphereCollider",
];

/// Serialized keys that identify the owning object rather than its state.
/// They are recorded once per record and removed from the field payload so a
/// runtime consumer never re-derives ownership from raw pointers.
pub(super) const BEHAVIOUR_OWNERSHIP_KEYS: [&str; 4] = ["m_Enabled", "m_GameObject", "m_Name", "m_Script"];
