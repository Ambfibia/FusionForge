//! Complete guarded newly imported mip references; retain previous runtime aliases verbatim.
use super::*;
use crate::fusionforge::native_texture_sharing as g;

pub(super) fn complete(
    root: &Path,
    recipe: &Value,
    files: &mut BTreeMap<String, Vec<u8>>,
    preimages: &mut BTreeMap<String, Vec<u8>>,
    catalog: &mut Value,
    runtime: &mut Value,
) -> Result<BTreeSet<String>, String> {
    let mut replace = BTreeSet::new();
    for row in array(recipe, "retainedRuntimeContracts")? {
        for contract in runtime["textures"]
            .as_array_mut()
            .ok_or("runtime contracts")?
        {
            if *contract == row["observed"] {
                *contract = row["retained"].clone();
            }
        }
    }
    let retained_count = recipe["retainedRuntimeTextureCount"]
        .as_u64()
        .ok_or("retained runtime count")? as usize;
    for contract in runtime["textures"]
        .as_array_mut()
        .ok_or("runtime contracts")?
        .iter_mut()
        .skip(retained_count)
    {
        contract["sampler"]["name"] = contract["trueName"].clone();
    }
    for artifact in array(recipe, "importedModels")? {
        let route = artifact["path"].as_str().ok_or("retained model path")?;
        if !root.join(route).is_file() {
            continue;
        }
        let bytes = fs::read(root.join(route)).map_err(|e| e.to_string())?;
        if reference(route, &bytes) != *artifact {
            continue;
        }
        let (mut doc, suffix) = g::decode(&bytes, true)?;
        let mut redirects = BTreeMap::new();
        g::visit(&doc, &mut |binding| {
            let (Some(base), Some(levels)) =
                (binding["uri"].as_str(), binding["mipLevels"].as_array())
            else {
                return;
            };
            for level in levels.iter().skip(1) {
                let Some(old) = level["uri"].as_str() else {
                    continue;
                };
                let expected = format!(
                    "{}.mips/mip-{:02}.png",
                    base.trim_end_matches(".png"),
                    level["level"].as_u64().unwrap_or(0)
                );
                if old != expected {
                    redirects.insert(old.to_owned(), expected);
                }
            }
        });
        for (old, new) in &redirects {
            let parent = route.rsplit_once('/').ok_or("model parent")?.0;
            let source = g::normalized(&format!("{parent}/{old}")).ok_or("invalid mip source")?;
            let target = g::normalized(&format!("{parent}/{new}")).ok_or("invalid mip target")?;
            let payload = fs::read(root.join(&source)).map_err(|e| e.to_string())?;
            if root.join(&target).exists()
                && fs::read(root.join(&target)).map_err(|e| e.to_string())? != payload
            {
                return Err(format!(
                    "shared atlas has different exact mip pixels: {target}"
                ));
            }
            put(files, target, payload)?;
        }
        if redirects.is_empty() {
            continue;
        }
        g::mutate(&mut doc, &mut |value| {
            if let Some(uri) = value["uri"].as_str() {
                if let Some(target) = redirects.get(uri) {
                    value["uri"] = json!(target);
                }
            }
        });
        let updated = g::encode(&bytes, &doc, &suffix, true)?;
        preimages.insert(route.into(), bytes);
        put(files, route.into(), updated.clone())?;
        replace.insert(route.to_owned());
        let model = catalog["models"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|m| m["model"]["path"] == route)
            .ok_or("model catalog row")?;
        model["model"] = reference(route, &updated);
        let owner = model["resourceSet"].clone();
        let set_route = catalog["sets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == owner)
            .ok_or("set owner")?["definition"]["path"]
            .as_str()
            .ok_or("set path")?
            .to_owned();
        if !files.contains_key(&set_route) {
            put(
                files,
                set_route.clone(),
                fs::read(root.join(&set_route)).map_err(|e| e.to_string())?,
            )?;
        }
        let set: Value = serde_json::from_slice(&files[&set_route]).map_err(|e| e.to_string())?;
        for member in array(&set, "members")? {
            let item_route = member["definition"]["path"].as_str().ok_or("item path")?;
            if !files.contains_key(item_route) {
                put(
                    files,
                    item_route.into(),
                    fs::read(root.join(item_route)).map_err(|e| e.to_string())?,
                )?;
            }
        }
    }
    println!("Guarded Academy mip completion: {} models", replace.len());
    Ok(replace)
}
