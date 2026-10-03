use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticMaterialShaderMetadata {
    pub publish_report: String,
    pub material_count: u64,
    pub texture_count: u64,
    pub sampler_count: u64,
    pub shaders: Vec<SemanticShader>,
    pub textures: Vec<SemanticTexture>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticShader {
    pub material_name: String,
    pub shader_name: String,
    pub shader_sha256: String,
    pub effective_render_queue: i64,
    pub render_pass_count: u64,
}
