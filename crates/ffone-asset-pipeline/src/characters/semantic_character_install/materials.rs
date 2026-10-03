use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishedCharacterMaterialSummary {
    pub material_count: u64,
    pub texture_count: u64,
    pub sampler_count: u64,
    pub shaders: Vec<SemanticShader>,
    pub textures: Vec<PublishedCharacterTexture>,
}
