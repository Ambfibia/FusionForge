use super::*;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum GlyphRenderScaleApplication {
    ApplyExactlyOnceToGlyphRasterBeforeMeasurement,
}
