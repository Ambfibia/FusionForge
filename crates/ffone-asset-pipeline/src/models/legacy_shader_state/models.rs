
// Retrobution's Tutorial dependency Shader PathID 1619, reached from the
// Effects.resourceFile ES739 prefab. The hologram mesh is the narrowly audited
// consumer. Its fixed-function program is additive One/One, RGB-only,
// depth-write disabled and two-sided; a name match without the exact 776-byte
// ShaderLab program hash is intentionally rejected.
pub(super) const ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD: &str =
    "normal_blendOneOne_zwriteOff_cullOff_vertexColorAD";

pub(super) const ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD_SHA256: &str =
    "4186fb05f40d4ca77d6c8aa5c40afb615b02c180229788f19c7c08515c429e01";

pub(super) const ADDITIVE_TRANSPARENT_CULL_OFF_VERTEX: &str = "normal_blendSrcalphaOne_zwriteOff_cullOff_vertexColorAD";

pub(super) const ADDITIVE_TRANSPARENT_CULL_OFF_VERTEX_SHA256: &str = "1db064d01ba0a9cd6972f0c6d4972fedd5418f29c58415cbcecad6e3ab2f585f";
