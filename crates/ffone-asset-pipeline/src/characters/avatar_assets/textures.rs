
pub(super) fn semantic_texture_path(category: &str, name: &str, texture: &str) -> String {
    format!("characters/player/{category}/{name}/textures/{texture}.png")
}
