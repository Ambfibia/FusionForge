use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DumpObject {
    pub(super) asset: String,
    pub(super) name: String,
    pub(super) path_id: i64,
    #[serde(rename = "type")]
    pub(super) object_type: String,
    pub(super) value: Box<RawValue>,
}

pub(super) fn classified_unity_alias_warning(route: &str, warning: &str) -> bool {
    warning
        .strip_prefix(route)
        .and_then(|rest| rest.strip_prefix(": "))
        .is_some_and(|detail| {
            detail == "could not extract NIF bytes" || detail == "NIF parse failed: binrw error"
        })
}
