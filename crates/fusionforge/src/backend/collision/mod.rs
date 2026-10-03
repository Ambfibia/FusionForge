use super::super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ColliderObject {
    pub(in super::super) id: String,
    pub(in super::super) name: String,
    pub(in super::super) kind: String,
    pub(in super::super) attached_object_id: Option<String>,
    pub(in super::super) position: Vec3,
    pub(in super::super) rotation: Vec3,
    pub(in super::super) scale: Vec3,
    pub(in super::super) is_trigger: bool,
    pub(in super::super) material: Option<String>,
    pub(in super::super) source_mode: String,
    pub(in super::super) source_path_id: Option<i64>,
}

pub(in super::super) fn rewrite_collision_flags_masks(data: &str) -> String {
    let mut fixed = data.to_string();
    for target in ["val13", "val21", "controller.collisionFlags"] {
        for mask in ["1", "2", "4"] {
            fixed = fixed.replace(
                &format!("({target} & {mask}) != 0"),
                &format!("({target} & (CollisionFlags){mask}) != (CollisionFlags)0"),
            );
        }
    }
    fixed
}
