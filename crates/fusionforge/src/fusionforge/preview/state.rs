
pub(super) fn alpha_mode_hint(
    material_name: &str,
    shader_name: &str,
    has_alpha: bool,
    color_alpha: f64,
    cutoff: Option<f64>,
    src_blend: Option<f64>,
    dst_blend: Option<f64>,
) -> &'static str {
    let label = format!("{material_name} {shader_name}").to_lowercase();
    let compact_label = label.replace(['_', '-', ' ', '/', '\\'], "");
    let src_blend = src_blend.map(|value| value.round() as i32);
    let dst_blend = dst_blend.map(|value| value.round() as i32);

    let additive = label.contains("glow")
        || label.contains("flare")
        || label.contains("particle")
        || label.contains("color_circle")
        || label.contains("eff_")
        || compact_label.contains("blendoneone")
        || compact_label.contains("blendsrcalphaone")
        || matches!(
            (src_blend, dst_blend),
            (Some(5), Some(1)) | (Some(1), Some(1))
        );
    if additive {
        return "additive";
    }

    if compact_label.contains("alphatest") {
        return "cutout";
    }

    let blended = label.contains("water")
        || label.contains("glass")
        || label.contains("fluid")
        || label.contains("cloud")
        || label.contains("smoke")
        || label.contains("particle")
        || label.contains("transparent")
        || compact_label.contains("blendsrcalphainvsrcalpha")
        || compact_label.contains("zwriteoff")
        || color_alpha < 0.995
        || matches!(
            (src_blend, dst_blend),
            (Some(5), Some(10)) | (Some(5), Some(6))
        );
    if blended {
        return "blend";
    }

    if has_alpha || cutoff.is_some_and(|value| value > 0.0) {
        return "cutout";
    }
    "opaque"
}
