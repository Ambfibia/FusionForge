use super::*;

pub(super) fn attach_avatar_texture_routes(
    asset_root: &Path,
    models: &mut [PlayerModel],
    textures: &mut BTreeMap<String, PlayerTexture>,
) -> Result<()> {
    let avatar_path = asset_root.join("data/character_creation/avatar_items.json");
    let avatar = read_json(&avatar_path, "avatar item catalog")?;
    let model_index = models
        .iter()
        .enumerate()
        .map(|(index, model)| (model.old_rooted.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut links = Vec::<(usize, PathBuf, String)>::new();
    for item in avatar
        .get("items")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        for gender in ["male", "female"] {
            let Some(visual) = item.get(gender) else {
                continue;
            };
            let visual_models = visual
                .get("models")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten()
                .filter_map(|model| {
                    model
                        .get("nativeAsset")
                        .and_then(|asset| asset.get("path"))
                        .and_then(JsonValue::as_str)
                })
                .filter_map(|path| model_index.get(path).copied())
                .collect::<BTreeSet<_>>();
            for key in ["primaryTexture", "secondaryTexture"] {
                for candidate in visual
                    .get(key)
                    .and_then(|texture| texture.get("candidates"))
                    .and_then(JsonValue::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(path) = candidate.get("path").and_then(JsonValue::as_str) else {
                        continue;
                    };
                    let absolute = asset_root.join(Path::new(path));
                    if !absolute.is_file() {
                        continue;
                    }
                    let name = file_stem(&absolute)?;
                    for index in &visual_models {
                        links.push((*index, absolute.clone(), name.clone()));
                    }
                }
            }
        }
    }
    for (index, path, name) in links {
        let hash = register_player_texture(textures, &path, name, false)?;
        models[index].atlas_hashes.insert(hash);
    }
    Ok(())
}
