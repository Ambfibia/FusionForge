use super::*;

#[derive(Clone, Debug)]
pub(super) struct MapObject {
    pub(super) old_relative: String,
    pub(super) category: String,
    pub(super) prefix: String,
    pub(super) family: String,
    pub(super) id: String,
    pub(super) name: String,
    pub(super) definition: JsonValue,
    pub(super) files: Vec<PathBuf>,
    pub(super) textures: Vec<TextureUse>,
}
