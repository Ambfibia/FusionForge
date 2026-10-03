//! Append explicitly selected native clips without republishing a model's render assets.
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

fn read(path: &Path) -> Result<(Value, Vec<u8>), String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() < 28 || &bytes[..4] != b"glTF" {
        return Err("not a GLB".into());
    }
    let n = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let end = 20usize.checked_add(n).ok_or("GLB overflow")?;
    if end + 8 > bytes.len() {
        return Err("truncated GLB".into());
    }
    let doc = serde_json::from_slice(&bytes[20..end]).map_err(|e| e.to_string())?;
    let len = u32::from_le_bytes(bytes[end..end + 4].try_into().unwrap()) as usize;
    let bin = bytes
        .get(end + 8..end + 8 + len)
        .ok_or("truncated BIN")?
        .to_vec();
    Ok((doc, bin))
}

fn paths(doc: &Value) -> Result<BTreeMap<Vec<String>, usize>, String> {
    fn visit(
        doc: &Value,
        i: usize,
        mut path: Vec<String>,
        out: &mut BTreeMap<Vec<String>, usize>,
    ) -> Result<(), String> {
        if out.values().any(|j| *j == i) {
            return Err("node has multiple owners or a cycle".into());
        }
        let node = &doc["nodes"][i];
        path.push(node["name"].as_str().ok_or("unnamed node")?.to_owned());
        if out.insert(path.clone(), i).is_some() {
            return Err("ambiguous hierarchy path".into());
        }
        if let Some(children) = node["children"].as_array() {
            for child in children {
                visit(
                    doc,
                    child.as_u64().ok_or("invalid child")? as usize,
                    path.clone(),
                    out,
                )?;
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    let scene = doc["scene"].as_u64().unwrap_or(0) as usize;
    for root in doc["scenes"][scene]["nodes"]
        .as_array()
        .ok_or("missing scene roots")?
    {
        visit(
            doc,
            root.as_u64().ok_or("invalid root")? as usize,
            vec![],
            &mut out,
        )?;
    }
    Ok(out)
}

fn append(
    mut target: Value,
    mut bin: Vec<u8>,
    donor: &Value,
    donor_bin: &[u8],
    names: &[String],
    retain_static_root_scale: bool,
) -> Result<Vec<u8>, String> {
    let before = target.clone();
    let old_paths = paths(&target)?;
    let new_paths = paths(donor)?;
    let mut mapping = BTreeMap::new();
    for (path, &i) in &new_paths {
        let &j = old_paths
            .get(path)
            .ok_or_else(|| format!("donor node absent: {path:?}"))?;
        for field in ["translation", "rotation", "scale", "matrix"] {
            if donor["nodes"][i][field] != target["nodes"][j][field] {
                // An explicit native size override is outside the rig's local
                // curves. It survives only when no selected clip animates it.
                let static_root = path.len() == 1
                    && field == "scale"
                    && retain_static_root_scale
                    && donor["animations"]
                        .as_array()
                        .ok_or("donor animations")?
                        .iter()
                        .filter(|a| names.iter().any(|name| a["name"] == *name))
                        .all(|a| {
                            a["channels"].as_array().is_some_and(|channels| {
                                channels.iter().all(|c| c["target"]["node"] != json!(i))
                            })
                        });
                if static_root {
                    continue;
                }
                return Err(format!("rest pose differs: {path:?}/{field}"));
            }
        }
        mapping.insert(i, j);
    }
    for name in names {
        if target["animations"]
            .as_array()
            .ok_or("target animations")?
            .iter()
            .any(|a| a["name"] == *name)
        {
            return Err(format!("refusing to replace existing clip {name}"));
        }
        let candidates: Vec<_> = donor["animations"]
            .as_array()
            .ok_or("donor animations")?
            .iter()
            .filter(|a| a["name"] == *name)
            .collect();
        let [source] = candidates.as_slice() else {
            return Err(format!("expected exactly one donor clip {name}"));
        };
        let mut animation = (*source).clone();
        let mut accessors = BTreeMap::new();
        for sampler in animation["samplers"].as_array_mut().ok_or("samplers")? {
            for key in ["input", "output"] {
                let index = sampler[key].as_u64().ok_or("accessor index")? as usize;
                let mapped = if let Some(&mapped) = accessors.get(&index) {
                    mapped
                } else {
                    let mut accessor = donor["accessors"][index].clone();
                    if !accessor["sparse"].is_null() {
                        return Err("sparse animation accessor unsupported".into());
                    }
                    let mut view = donor["bufferViews"]
                        [accessor["bufferView"].as_u64().ok_or("view index")? as usize]
                        .clone();
                    if view["buffer"] != 0 {
                        return Err("external animation buffer unsupported".into());
                    }
                    let start = view["byteOffset"].as_u64().unwrap_or(0) as usize;
                    let len = view["byteLength"].as_u64().ok_or("view length")? as usize;
                    let payload = donor_bin
                        .get(start..start.checked_add(len).ok_or("view overflow")?)
                        .ok_or("view outside donor buffer")?;
                    while bin.len() % 4 != 0 {
                        bin.push(0);
                    }
                    view["byteOffset"] = json!(bin.len());
                    bin.extend_from_slice(payload);
                    let views = target["bufferViews"].as_array_mut().ok_or("target views")?;
                    accessor["bufferView"] = json!(views.len());
                    views.push(view);
                    let array = target["accessors"]
                        .as_array_mut()
                        .ok_or("target accessors")?;
                    let mapped = array.len();
                    array.push(accessor);
                    accessors.insert(index, mapped);
                    mapped
                };
                sampler[key] = json!(mapped);
            }
        }
        for channel in animation["channels"].as_array_mut().ok_or("channels")? {
            let node = channel["target"]["node"].as_u64().ok_or("target node")? as usize;
            channel["target"]["node"] = json!(mapping.get(&node).ok_or("unmapped animated node")?);
        }
        target["animations"].as_array_mut().unwrap().push(animation);
    }
    target["buffers"][0]["byteLength"] = json!(bin.len());
    for (key, value) in before.as_object().ok_or("GLB object")? {
        if !["animations", "accessors", "bufferViews", "buffers"].contains(&key.as_str())
            && &target[key] != value
        {
            return Err(format!("render contract changed: {key}"));
        }
    }
    let mut doc = serde_json::to_vec(&target).map_err(|e| e.to_string())?;
    while doc.len() % 4 != 0 {
        doc.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let mut bytes = b"glTF".to_vec();
    for word in [
        2,
        (28 + doc.len() + bin.len()) as u32,
        doc.len() as u32,
        0x4e4f534a,
    ] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend(doc);
    bytes.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&0x004e4942u32.to_le_bytes());
    bytes.extend(bin);
    Ok(bytes)
}

pub fn run(mut args: Vec<String>) -> Result<(), String> {
    let retain_static_root_scale = args.iter().any(|arg| arg == "--retain-static-root-scale");
    args.retain(|arg| arg != "--retain-static-root-scale");
    if args.len() < 4 {
        return Err(
            "usage: append_model_animations <native.glb> <donor.glb> <fresh-output.glb> <clip>..."
                .into(),
        );
    }
    if Path::new(&args[2]).exists() {
        return Err("output must be fresh".into());
    }
    let (target, bin) = read(Path::new(&args[0]))?;
    let (donor, donor_bin) = read(Path::new(&args[1]))?;
    let bytes = append(
        target,
        bin,
        &donor,
        &donor_bin,
        &args[3..],
        retain_static_root_scale,
    )?;
    // Animation-only publication must not rewrite existing authored geometry
    // to satisfy a newer renderer audit. Surface an unchanged baseline finding;
    // reject any new finding. Original mesh accessors/views and BIN are retained.
    let baseline = fs::read(&args[0]).map_err(|e| e.to_string())?;
    let previous_error = ffone_skinned_model::validate_glb_render_contract(&baseline)
        .err()
        .map(|e| e.to_string());
    let next_error = ffone_skinned_model::validate_glb_render_contract(&bytes)
        .err()
        .map(|e| e.to_string());
    if next_error != previous_error {
        return Err(format!(
            "render audit changed: {previous_error:?} -> {next_error:?}"
        ));
    }
    if let Some(error) = previous_error {
        eprintln!("Unchanged baseline render finding: {error}");
    }
    fs::write(&args[2], bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
