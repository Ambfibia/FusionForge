use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DumpObject {
    pub(super) name: String,
    pub(super) path_id: i64,
    #[serde(rename = "type")]
    pub(super) object_type: String,
    pub(super) value: Box<RawValue>,
}

pub(super) fn game_object_transform(catalog: &DumpCatalog, game_object: i64) -> Result<i64> {
    let transforms = component_path_ids(&catalog.value(game_object)?)?
        .into_iter()
        .filter(|path_id| {
            catalog
                .get(*path_id)
                .is_ok_and(|object| object.object_type == "Transform")
        })
        .collect::<Vec<_>>();
    let [transform] = transforms.as_slice() else {
        return rig_error(format!(
            "GameObject#{game_object} has {} Transform components",
            transforms.len()
        ));
    };
    Ok(*transform)
}
