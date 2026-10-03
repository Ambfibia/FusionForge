use super::*;

pub(super) fn ensure_placeholder_for_status(path: &Path, status: &str) -> Result<(), String> {
    if path.is_file() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let base: [u8; 3] = match status {
        "hidden_location_marker" => [45, 90, 115],
        "primary_interaction_placeholder" => [55, 105, 115],
        "texture_override_unresolved" => [150, 70, 30],
        "primary_ready_unpublished" => [204, 132, 31],
        "primary_blocked" => [170, 55, 55],
        "not_in_primary_catalog" => [112, 62, 148],
        "render_failed" => [220, 40, 70],
        "not_rendered" | "not_rendered_limit" => [65, 85, 105],
        _ => [110, 70, 70],
    };
    let image = RgbImage::from_fn(512, 512, |x, y| {
        let stripe = ((x / 24) + (y / 24)) % 2 == 0;
        let delta: u8 = if stripe { 18 } else { 0 };
        Rgb([
            base[0].saturating_add(delta),
            base[1].saturating_add(delta),
            base[2].saturating_add(delta),
        ])
    });
    image
        .save(path)
        .map_err(|error| format!("cannot write placeholder {}: {error}", path.display()))
}
