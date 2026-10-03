use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvatarModelReference {
    pub true_name: String,
    pub exact_route: String,
    pub native_asset: CharacterCreationAssetReference,
}

pub(super) fn model_alias(category: AvatarItemCategory, true_name: &str) -> Option<&'static str> {
    match (category, true_name.to_ascii_lowercase().as_str()) {
        // Retrobution TableData uses this historical spelling for the recolours while the
        // serialized mesh route and native GLB are both named `back_angelswing`.
        (AvatarItemCategory::Back, "back_angelwing") => Some("back_angelswing"),
        _ => None,
    }
}

pub(super) fn equipment_model_index(
    models: &[PlayerEquipmentCatalogModel],
    manifest: &[ProjectAssetFile],
) -> Result<BTreeMap<(String, String), AvatarModelReference>> {
    let manifest = manifest
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut output = BTreeMap::new();
    for model in models {
        let entry = manifest.get(model.glb.as_str()).ok_or_else(|| {
            invalid_error(format!(
                "equipment model {} is not manifest-listed",
                model.glb
            ))
        })?;
        let key = (
            model.category.to_ascii_lowercase(),
            model.true_name.to_ascii_lowercase(),
        );
        let value = AvatarModelReference {
            true_name: model.true_name.clone(),
            exact_route: model.exact_route.clone(),
            native_asset: CharacterCreationAssetReference {
                path: model.glb.clone(),
                bytes: entry.bytes,
                blake3: model.glb_blake3.clone(),
            },
        };
        if output.insert(key.clone(), value).is_some() {
            return invalid(format!(
                "duplicate player-equipment category/true-name: {}/{}",
                key.0, key.1
            ));
        }
    }
    Ok(output)
}
