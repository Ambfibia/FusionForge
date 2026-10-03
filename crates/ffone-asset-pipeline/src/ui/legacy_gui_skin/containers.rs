use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyGuiObjectPointer {
    pub file_id: i64,
    pub path_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_type: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ObjectIdentity {
    pub(super) name: String,
    pub(super) asset_type: String,
}

#[derive(Clone, Debug)]
pub(super) enum ObjectIdentityMatch {
    Unique(ObjectIdentity),
    Ambiguous,
}

pub(super) fn object_identities(objects: &[Value]) -> ObjectIdentityIndex {
    let mut identities = ObjectIdentityIndex::new();
    for object in objects {
        let Some(path_id) = object.get("pathId").and_then(Value::as_i64) else {
            continue;
        };
        let source_asset = string_field(object, "asset");
        if source_asset.is_empty() {
            continue;
        }
        let value = object.get("value").unwrap_or(&Value::Null);
        let key = (source_asset, path_id);
        let identity = ObjectIdentity {
            name: object_name(object, value),
            asset_type: string_field(object, "type"),
        };
        match identities.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(ObjectIdentityMatch::Unique(identity));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                entry.insert(ObjectIdentityMatch::Ambiguous);
            }
        }
    }
    identities
}

pub(super) fn object_name(object: &Value, value: &Value) -> String {
    let name = string_field(object, "name");
    if name.is_empty() {
        string_field(value, "m_Name")
    } else {
        name
    }
}
