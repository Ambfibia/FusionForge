use super::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceBundleProof {
    pub(super) source_path: String,
    pub(super) byte_length: u64,
    pub(super) sha256: String,
}

pub(super) fn object_field<'a>(value: &'a Value, field: &str) -> Result<&'a Value, String> {
    value
        .get(field)
        .filter(|value| value.is_object())
        .ok_or_else(|| format!("XDT field {field:?} is missing or not an object"))
}
