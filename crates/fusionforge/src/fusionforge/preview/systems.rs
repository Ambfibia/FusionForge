use super::*;

pub(super) fn apply_alpha_mask(image: &mut RgbaImage, mask: &RgbaImage) {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 || mask.width() == 0 || mask.height() == 0 {
        return;
    }
    let mask_has_alpha = image_has_alpha(mask);
    for y in 0..height {
        for x in 0..width {
            let mask_alpha = sampled_mask_alpha(mask, mask_has_alpha, x, y, width, height) as u16;
            let pixel = image.get_pixel_mut(x, y);
            pixel.0[3] = ((pixel.0[3] as u16 * mask_alpha + 127) / 255) as u8;
        }
    }
}

pub(super) fn should_apply_black_key_alpha(material_name: &str, shader_name: &str, alpha_mode: &str) -> bool {
    if alpha_mode == "opaque" {
        return false;
    }
    let label = format!("{material_name} {shader_name}").to_lowercase();
    alpha_mode == "additive"
        || label.contains("boxglass")
        || label.contains("glass")
        || label.contains("plane")
        || label.contains("color_circle")
        || label.contains("decal")
        || label.contains("particle")
        || label.contains("sprite")
        || label.contains("eff_")
        || label.contains("glow")
        || label.contains("flare")
}

pub(super) fn apply_black_key_alpha(image: &mut RgbaImage, aggressive: bool) {
    if !has_black_key_coverage(image, if aggressive { 1 } else { 10 }) {
        return;
    }
    let use_luminance_alpha = !image_has_alpha(image);
    for pixel in image.pixels_mut() {
        let max_rgb = pixel.0[0].max(pixel.0[1]).max(pixel.0[2]);
        if pixel.0[3] == 0 {
            continue;
        }
        if use_luminance_alpha {
            pixel.0[3] = if max_rgb <= 6 {
                0
            } else if max_rgb >= 220 {
                255
            } else {
                ((max_rgb as u16 * 255 + 90) / 180).min(255) as u8
            };
        } else if max_rgb < 40 {
            pixel.0[3] = if max_rgb <= 6 {
                0
            } else {
                ((pixel.0[3] as u16 * max_rgb as u16 + 20) / 40) as u8
            };
        } else {
            continue;
        }
    }
}
