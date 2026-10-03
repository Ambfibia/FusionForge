use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PreflightTexture {
    pub(super) name: String,
    pub(super) mip_levels: Vec<Value>,
}

pub(super) fn texture_format(detail: &str) -> Option<u32> {
    let suffix = detail.split("TextureFormat ").nth(1)?;
    suffix.split_whitespace().next()?.parse().ok()
}
