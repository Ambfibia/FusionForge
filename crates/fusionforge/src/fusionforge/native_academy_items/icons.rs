//! Item icons are selected by matching item/mesh/icon identities in both tables.
use super::*;
use crate::fusionforge::unity::{ObjectKey, UnityEnvironment};
use base64::{Engine, engine::general_purpose::STANDARD};

pub(super) fn complete(
    source: &Path,
    root: &Path,
    donor: &Value,
    native: &Value,
    recipe: &Value,
    avatar: &mut Value,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let env = UnityEnvironment::from_assets(super::super::direct_input::read_assets(
        &source.join("Icons.resourceFile"),
    )?);
    let mut textures = BTreeMap::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for (id, info) in asset
            .objects
            .iter()
            .filter(|(_, i)| asset.object_type_name(i) == "Texture2D")
        {
            let name = super::super::object_name(&asset.read_object(asset_index, info)?)
                .to_ascii_lowercase();
            if textures
                .insert(
                    name.clone(),
                    ObjectKey {
                        asset: asset_index,
                        path_id: *id,
                    },
                )
                .is_some()
            {
                return Err(format!("ambiguous Academy icon {name}"));
            }
        }
    }
    let mut recovered = 0;
    for item in avatar["items"].as_array_mut().ok_or("avatar items")? {
        if item["icon"]["status"] != "missing" {
            continue;
        }
        let category = item["category"].as_str().ok_or("icon category")?;
        if !recipe["nativeIds"][category]
            .as_array()
            .is_some_and(|ids| ids.contains(&item["itemNumber"]))
        {
            continue;
        }
        let cat = CATEGORIES
            .iter()
            .find(|(cat, _)| serde_json::to_value(cat).ok().as_ref() == Some(&item["category"]))
            .ok_or("item category")?
            .0;
        let table = &native[cat.table_name()];
        let donor_table = &donor[cat.table_name()];
        let native_row = array(table, "m_pItemData")?
            .iter()
            .find(|row| row["m_iItemNumber"] == item["itemNumber"])
            .ok_or("native item")?;
        let native_mesh =
            &table["m_pItemMeshData"][native_row["m_iMesh"].as_u64().ok_or("mesh index")? as usize];
        let native_icon =
            &table["m_pItemIconData"][native_row["m_iIcon"].as_u64().ok_or("icon index")? as usize];
        let proven =
            array(donor_table, "m_pItemData")?.iter().any(|row| {
                donor_table["m_pItemStringData"][row["m_iItemName"].as_u64().unwrap_or(0) as usize]
                    ["m_strName"]
                    == item["name"]
                    && donor_table["m_pItemMeshData"][row["m_iMesh"].as_u64().unwrap_or(0) as usize]
                        == *native_mesh
                    && donor_table["m_pItemIconData"][row["m_iIcon"].as_u64().unwrap_or(0) as usize]
                        == *native_icon
            });
        if !proven {
            return Err(format!(
                "Academy icon identity is unproven for {category}/{}",
                item["itemNumber"]
            ));
        }
        let name = item["icon"]["trueName"].as_str().ok_or("icon name")?;
        let Some(key) = textures.get(&name.to_ascii_lowercase()) else {
            return Err(format!("Academy icon missing: {name}"));
        };
        let source = crate::logical_model_material::exact_texture(&env, *key)?;
        let folder = match item["icon"]["iconType"].as_u64() {
            Some(0) => "weapons",
            Some(3) => "cosmetics",
            Some(12) => "vehicles",
            _ => return Err(format!("unsupported equipment icon {name}")),
        };
        let route = format!("icons/items/{folder}/{name}.png");
        let decode = |value: &Value| {
            STANDARD
                .decode(
                    value["dataUrl"]
                        .as_str()
                        .ok_or("icon payload")?
                        .strip_prefix("data:image/png;base64,")
                        .ok_or("PNG payload")?,
                )
                .map_err(|e| e.to_string())
        };
        let retained = root.join(&route).is_file();
        let bytes = if retained {
            fs::read(root.join(&route)).map_err(|e| e.to_string())?
        } else {
            decode(&source["payload"])?
        };
        put(files, route.clone(), bytes.clone())?;
        for level in source["mipLevels"]
            .as_array()
            .into_iter()
            .flatten()
            .skip(1)
            .filter(|_| !retained)
        {
            let mip_route = format!(
                "{}.mips/mip-{:02}.png",
                route.trim_end_matches(".png"),
                level["level"].as_u64().ok_or("icon mip")?
            );
            put(files, mip_route, decode(&level["payload"])?)?;
        }
        item["icon"]["candidates"] = json!([reference(&route, &bytes)]);
        item["icon"]["status"] = json!("verified_unique");
        recovered += 1;
    }
    println!("Academy item icons completed: {recovered} references");
    Ok(())
}
