use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SerializedObjectIdentity {
    pub asset_name: String,
    pub path_id: i64,
    pub object_type: String,
}
