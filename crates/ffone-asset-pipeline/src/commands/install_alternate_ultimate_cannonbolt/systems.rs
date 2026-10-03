use super::*;

pub(super) fn update_avatar_visual(
    avatar: &mut Value,
    spec: &SetSpec,
    model: &ModelSpec,
    model_artifact: &ResourceSetArtifact,
    texture_artifact: &ResourceSetArtifact,
) -> Result<(), String> {
    let item = avatar["items"]
        .as_array_mut()
        .ok_or_else(|| "avatar catalog has no items".to_owned())?
        .iter_mut()
        .find(|item| {
            item["category"] == spec.category
                && item["itemNumber"].as_i64() == Some(spec.item_number)
        })
        .ok_or_else(|| {
            format!(
                "patched {} item {} is absent",
                spec.category, spec.item_number
            )
        })?;
    let visual = item
        .get_mut(model.gender)
        .ok_or_else(|| format!("item has no {} visual", model.gender))?;
    let alias = match (spec.category, model.gender) {
        ("pants", "female") => "f_pants_ultimatecannonbolt",
        ("pants", "male") => "m_pants_ultimatecannonbolt",
        ("shirt", "female") => "f_shirt_ultimatecannonbolt",
        ("shirt", "male") => "m_shirt_ultimatecannonbolt",
        ("shoes", _) => "shoes_ultimatecannonbolt",
        _ => return Err("unexpected Ultimate Cannonbolt visual mapping".to_owned()),
    };
    visual["models"] = json!([{
        "exactRoute": model.exact_route,
        "nativeAsset": model_artifact,
        "trueName": model.true_name,
    }]);
    visual["modelStatus"] = json!("verified_unique");
    visual["sourceModelTrueName"] = json!(model.true_name);
    visual["primaryTexture"] = json!({
        "candidates": [texture_artifact],
        "status": "verified_unique",
        "trueName": alias,
    });
    Ok(())
}
