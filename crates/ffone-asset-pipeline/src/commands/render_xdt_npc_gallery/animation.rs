use super::*;

pub(super) fn build_animation_coverage_report(renders: &[ModelRenderRecord]) -> Vec<Value> {
    renders
        .iter()
        .filter_map(|render| {
            let issue = if render.available_animations.is_empty() {
                "no_animations"
            } else if !render.has_stand1 {
                "stand1_missing_alternate_clip_selected"
            } else {
                return None;
            };
            Some(json!({
                "code": issue,
                "registryId": render.registry_id,
                "logicalName": render.logical_name,
                "glb": render.glb,
                "selectedAnimation": render.selected_animation,
                "availableAnimations": render.available_animations,
                "renderStatus": render.status,
            }))
        })
        .collect()
}

pub(super) fn select_animation(animations: &[String]) -> Option<String> {
    animations
        .iter()
        .find(|name| name.eq_ignore_ascii_case("stand1"))
        .or_else(|| {
            animations
                .iter()
                .find(|name| name.eq_ignore_ascii_case("nif-default"))
        })
        .or_else(|| animations.first())
        .cloned()
}
