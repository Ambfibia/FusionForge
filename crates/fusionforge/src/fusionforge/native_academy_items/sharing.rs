//! Share exact native atlas files by guarded references, preserving accepted package locations.
use super::*;
use crate::fusionforge::native_texture_sharing as g;

pub(super) fn normalize(
    root: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
    preimages: &mut BTreeMap<String, Vec<u8>>,
    catalog: &mut Value,
    avatar: &mut Value,
    runtime: &mut Value,
) -> Result<(), String> {
    let mut documents = BTreeMap::new();
    let mut canonical = BTreeMap::<String, Value>::new();
    for row in array(catalog, "renderingTextures")? {
        canonical.insert(
            row["blake3"]
                .as_str()
                .ok_or("rendering texture hash")?
                .into(),
            row.clone(),
        );
    }
    for entry in array(catalog, "sets")? {
        let route = entry["definition"]["path"]
            .as_str()
            .ok_or("set path")?
            .to_owned();
        let bytes = files
            .get(&route)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| fs::read(root.join(&route)).map_err(|e| e.to_string()))?;
        let doc: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        for texture in array(&doc, "textures")?
            .iter()
            .filter(|t| root.join(t["path"].as_str().unwrap()).exists())
        {
            canonical
                .entry(texture["blake3"].as_str().ok_or("atlas hash")?.into())
                .or_insert_with(|| texture.clone());
        }
        documents.insert(route, doc);
    }
    let mut redirects = BTreeMap::<String, String>::new();
    for doc in documents.values_mut() {
        let mut textures = Vec::new();
        for texture in array(doc, "textures")? {
            let hash = texture["blake3"].as_str().ok_or("atlas hash")?.to_owned();
            let target = canonical
                .entry(hash)
                .or_insert_with(|| texture.clone())
                .clone();
            let from = texture["path"].as_str().ok_or("atlas path")?;
            let to = target["path"].as_str().unwrap();
            if from != to {
                if root.join(from).exists() {
                    return Err(format!(
                        "accepted atlas copies require explicit repair: {from}"
                    ));
                }
                redirects.insert(from.to_owned(), to.to_owned());
            }
            if !textures.iter().any(|t: &Value| t["path"] == target["path"]) {
                textures.push(target);
            }
        }
        doc["textures"] = json!(textures);
    }
    // A shared base image retains its complete exact source mip chain.
    for (from, to) in redirects.clone() {
        let prefix = format!("{}.mips/", from.trim_end_matches(".png"));
        let mips = files
            .iter()
            .filter(|(path, _)| path.starts_with(&prefix))
            .map(|(path, bytes)| (path.clone(), bytes.clone()))
            .collect::<Vec<_>>();
        for (path, bytes) in mips {
            let target = format!(
                "{}.mips/{}",
                to.trim_end_matches(".png"),
                path.trim_start_matches(&prefix)
            );
            if root.join(&target).exists()
                && fs::read(root.join(&target)).map_err(|e| e.to_string())? != bytes
            {
                return Err(format!(
                    "shared atlas has different exact source mip pixels: {target}"
                ));
            }
            put(files, target.clone(), bytes)?;
            redirects.insert(path, target);
        }
    }
    let resolve = |path: &str| {
        redirects
            .get(path)
            .cloned()
            .unwrap_or_else(|| path.to_owned())
    };
    let originals = std::mem::take(files);
    for (route, bytes) in originals {
        if redirects.contains_key(&route) {
            continue;
        }
        if route.ends_with(".glb") {
            let (mut doc, suffix) = g::decode(&bytes, true)?;
            let mut error = None;
            g::mutate(&mut doc, &mut |value| {
                if let Some(uri) = value["uri"]
                    .as_str()
                    .filter(|u| !u.starts_with("data:"))
                    .map(str::to_owned)
                {
                    let parent = route.rsplit_once('/').unwrap().0;
                    match g::normalized(&format!("{parent}/{uri}")) {
                        Some(path) => {
                            value["uri"] = json!(g::relative_uri(&route, &resolve(&path)))
                        }
                        None => error = Some("invalid model texture dependency"),
                    }
                }
            });
            if let Some(error) = error {
                return Err(error.into());
            }
            put(files, route, g::encode(&bytes, &doc, &suffix, true)?)?;
        } else {
            put(files, route, bytes)?;
        }
    }
    let update = |document: &mut Value, files: &BTreeMap<String, Vec<u8>>| -> Result<(), String> {
        let mut error = None;
        g::mutate(document, &mut |value| {
            if let Some(path) = value["path"].as_str().map(str::to_owned) {
                let target = resolve(&path);
                if target != path || files.contains_key(&target) {
                    let bytes =
                        files.get(&target).cloned().map(Ok).unwrap_or_else(|| {
                            fs::read(root.join(&target)).map_err(|e| e.to_string())
                        });
                    match bytes {
                        Ok(bytes) => *value = reference(&target, &bytes),
                        Err(e) => error = Some(e),
                    }
                }
            }
        });
        error.map_or(Ok(()), Err)
    };
    for row in catalog["models"].as_array_mut().unwrap() {
        update(row, files)?;
    }
    update(avatar, files)?;
    update(runtime, files)?;
    for (route, mut doc) in documents {
        if !files.contains_key(&route) {
            continue;
        }
        update(&mut doc, files)?;
        for member in doc["members"].as_array_mut().unwrap() {
            let item_route = member["definition"]["path"].as_str().unwrap().to_owned();
            if let Some(bytes) = files.get(&item_route) {
                let mut item: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
                update(&mut item, files)?;
                let bytes = p::encode(&item)?;
                member["definition"] = reference(&item_route, &bytes);
                files.insert(item_route, bytes);
            }
        }
        if root.join(&route).exists() && !preimages.contains_key(&route) {
            preimages.insert(
                route.clone(),
                fs::read(root.join(&route)).map_err(|e| e.to_string())?,
            );
        }
        let bytes = p::encode(&doc)?;
        let entry = catalog["sets"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|e| e["definition"]["path"] == route)
            .ok_or("set entry")?;
        entry["definition"] = reference(&route, &bytes);
        entry["textureCount"] = json!(doc["textures"].as_array().unwrap().len());
        files.insert(route, bytes);
    }
    let old_runtime = p::read_json(&root.join(RUNTIME))?;
    let mut contracts = old_runtime["textures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["nativeAsset"]["path"].as_str().unwrap().to_owned(),
                t.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for texture in runtime["textures"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .skip(old_runtime["textures"].as_array().unwrap().len())
    {
        let route = texture["nativeAsset"]["path"].as_str().unwrap().to_owned();
        if let Some(contract) = contracts.get(&route) {
            let name = texture["trueName"].clone();
            *texture = contract.clone();
            texture["trueName"] = name.clone();
            texture["sampler"]["name"] = name;
        } else {
            contracts.insert(route, texture.clone());
        }
    }
    println!(
        "Equipment sharing preflight: {} exact atlas references reused",
        redirects.len()
    );
    Ok(())
}
