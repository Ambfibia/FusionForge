use super::*;

pub(super) fn is_preserved_effect_payload_path(path: &str) -> bool {
    path.starts_with(&format!("{TUTORIAL_EFFECT_ROOT}/models/"))
        || path.starts_with(&format!("{TUTORIAL_EFFECT_ROOT}/textures/"))
        || (path.starts_with(&format!("{TUTORIAL_EFFECT_ROOT}/"))
            && path.ends_with(".nif-animation.json"))
        || path.starts_with(&format!("{TUTORIAL_PROJECTILE_ROOT}/effects/models/"))
}
