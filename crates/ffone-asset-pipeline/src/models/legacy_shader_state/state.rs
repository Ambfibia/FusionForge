use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct RawState {
    pub(super) queue: Option<String>,
    pub(super) name: Option<String>,
    pub(super) blend: Option<RawBlend>,
    pub(super) blend_operation: Option<MaterialBlendOperation>,
    pub(super) cull: Option<MaterialCullMode>,
    pub(super) z_write: Option<bool>,
    pub(super) z_test: Option<MaterialCompareFunction>,
    pub(super) alpha_test: Option<RawAlphaTest>,
    pub(super) color_mask: Option<u8>,
}
