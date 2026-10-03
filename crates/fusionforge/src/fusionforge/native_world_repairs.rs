use super::{
    native_json_patch::rewrite,
    native_publication::{read_json, relative},
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn numbers(v: &Value) -> Result<Vec<f64>, String> {
    v.as_array()
        .ok_or("expected numeric array")?
        .iter()
        .map(|n| {
            n.as_f64()
                .filter(|n| n.is_finite())
                .ok_or("non-finite numeric value".into())
        })
        .collect()
}
fn dot(a: &Value, b: &Value) -> Result<f64, String> {
    let a = numbers(a)?;
    let b = numbers(b)?;
    require(a.len() == 4 && b.len() == 4, "expected quaternions")?;
    let length =
        (a.iter().map(|v| v * v).sum::<f64>() * b.iter().map(|v| v * v).sum::<f64>()).sqrt();
    require(length > 0., "zero quaternion")?;
    Ok(a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>() / length)
}
pub(super) fn vortex(document: &mut Value) -> Result<(), String> {
    let models = json!((1067..1079)
        .map(|i| format!("static-map_12_10-{i:05}"))
        .collect::<Vec<_>>());
    let matches = document["animations"]
        .as_array()
        .ok_or("animations")?
        .iter()
        .enumerate()
        .filter(|(_, a)| a["models"] == models)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    require(matches.len() == 1, "expected one twelve-shell vortex owner")?;
    let index = matches[0];
    require(
        matches!(
            document["animations"][index]["wrapMode"].as_u64(),
            Some(0 | 2)
        ),
        "unexpected vortex wrap",
    )?;
    let id = document["animations"][index]["defaultClipId"].clone();
    let clips = document["animationClips"]
        .as_array_mut()
        .ok_or("clips")?
        .iter_mut()
        .filter(|c| c["id"] == id)
        .collect::<Vec<_>>();
    require(clips.len() == 1, "ambiguous vortex clip")?;
    let clip = clips.into_iter().next().unwrap();
    require(
        (clip["duration"].as_f64().ok_or("duration")? - 3.99).abs() < 0.0001,
        "vortex duration differs",
    )?;
    let rate = clip["sampleRate"].as_f64().ok_or("sampleRate")?;
    require(rate > 0. && rate.is_finite(), "invalid sample rate")?;
    let channels = clip["channels"].as_array_mut().ok_or("channels")?;
    require(channels.len() == 12, "vortex needs twelve channels")?;
    for channel in channels {
        require(
            channel["property"] == "rotation",
            "non-rotation vortex track",
        )?;
        let values = channel["values"].as_array().ok_or("rotation values")?;
        require(values.len() >= 2, "short rotation track")?;
        let first = values[0].clone();
        let d = dot(&first, values.last().unwrap())?;
        let times = numbers(&channel["times"])?;
        require(times.len() == values.len(), "track samples differ")?;
        if d.abs() < 0.9995 {
            let gap = 2. * d.abs().min(1.).acos();
            let travel = values
                .windows(2)
                .map(|w| dot(&w[0], &w[1]).map(|d| 2. * d.abs().min(1.).acos()))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .sum::<f64>();
            let end = *times.last().unwrap();
            require(end > 0. && travel > 0., "zero vortex travel")?;
            let close = ((end + gap / (travel / end)) * rate).round_ties_even() / rate;
            require(
                end < close && close <= end + 0.1,
                "closing sample outside recovered frame",
            )?;
            channel["times"].as_array_mut().unwrap().push(json!(close));
            channel["values"]
                .as_array_mut()
                .unwrap()
                .push(json!(numbers(&first)?
                    .iter()
                    .map(|v| v * if d >= 0. { 1. } else { -1. })
                    .collect::<Vec<_>>()));
            for key in ["inTangents", "outTangents"] {
                if channel[key]
                    .as_array()
                    .is_some_and(|v| v.len() == times.len())
                {
                    channel[key]
                        .as_array_mut()
                        .unwrap()
                        .push(json!([0., 0., 0., 0.]));
                }
            }
        }
        channel["preInfinity"] = json!(2);
        channel["postInfinity"] = json!(2);
    }
    document["animations"][index]["wrapMode"] = json!(2);
    Ok(())
}

fn refresh(value: &mut Value, outputs: &[(String, Vec<u8>)]) {
    match value {
        Value::Object(map) => {
            if let Some((_, bytes)) = map
                .get("path")
                .and_then(Value::as_str)
                .and_then(|p| outputs.iter().find(|(route, _)| route == p))
            {
                if map.contains_key("blake3") {
                    map.insert(
                        "blake3".into(),
                        json!(blake3::hash(bytes).to_hex().to_string()),
                    );
                    map.insert("bytes".into(), json!(bytes.len()));
                }
            }
            for child in map.values_mut() {
                refresh(child, outputs);
            }
        }
        Value::Array(rows) => {
            for child in rows {
                refresh(child, outputs);
            }
        }
        _ => {}
    }
}

pub(super) fn candy(root: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    const TILE: &str = "map/tiles/map_08_06/";
    const FOLIAGE: [&str; 3] = [
        "static-map_08_06-02106",
        "static-map_08_06-02107",
        "static-map_08_06-02108",
    ];
    const SHELLS: [&str; 2] = ["BuildPlayer-Map_08_06#11211", "BuildPlayer-Map_08_06#11271"];
    let paths = [
        format!("{TILE}behaviour.json"),
        format!("{TILE}scene.json"),
        format!("{TILE}objects.json"),
        format!("{TILE}tile.json"),
        "map/catalog.json".into(),
    ];
    let mut docs = BTreeMap::new();
    let mut originals = BTreeMap::new();
    for path in &paths {
        let full = relative(root, path)?;
        docs.insert(path.clone(), read_json(&full)?);
        originals.insert(path.clone(), fs::read(full).map_err(|e| e.to_string())?);
    }
    let behavior = docs.get_mut(&paths[0]).unwrap();
    let matches = behavior["animations"]
        .as_array_mut()
        .ok_or("animations")?
        .iter_mut()
        .filter(|a| a["node"] == "BuildPlayer-Map_08_06#13249")
        .collect::<Vec<_>>();
    require(matches.len() == 1, "ambiguous Candy Cove owner")?;
    let animation = matches.into_iter().next().unwrap();
    let targets = animation["targets"].as_array_mut().ok_or("targets")?;
    let mut changed = 0;
    let mut removed_models = Vec::new();
    for t in targets {
        let models = t["models"].as_array().ok_or("target models")?;
        if models
            .iter()
            .any(|v| v.as_str().is_some_and(|s| FOLIAGE.contains(&s)))
        {
            require(
                t["parentPath"] == "Object01"
                    && models
                        .iter()
                        .all(|m| m.as_str().is_some_and(|s| FOLIAGE.contains(&s))),
                "unexpected foliage ownership",
            )?;
            removed_models.extend(models.iter().filter_map(Value::as_str).map(str::to_owned));
            t["models"] = json!([]);
            changed += 1;
        }
    }
    removed_models.sort();
    require(
        (changed == 3 && removed_models == FOLIAGE) || changed == 0,
        "expected three foliage targets",
    )?;
    animation["models"]
        .as_array_mut()
        .ok_or("animation models")?
        .retain(|v| !v.as_str().is_some_and(|s| FOLIAGE.contains(&s)));
    let scene = docs.get_mut(&paths[1]).unwrap();
    let visuals = scene["visuals"].as_array_mut().ok_or("visuals")?;
    let shells = visuals
        .iter()
        .filter(|v| {
            SHELLS.iter().any(|s| {
                v["name"]
                    .as_str()
                    .is_some_and(|n| n.contains(&format!("[{s} visual]")))
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    require(
        shells.len() == 2 || shells.is_empty(),
        "expected two Candy Cove shells",
    )?;
    let octopus = visuals
        .iter()
        .filter(|v| {
            v["name"]
                .as_str()
                .is_some_and(|s| s.contains("ep_exsh_iumppad_octopus_01"))
        })
        .collect::<Vec<_>>();
    require(octopus.len() == 2, "expected two octopus visuals")?;
    for shell in &shells {
        require(
            shell["name"]
                .as_str()
                .is_some_and(|s| s.contains("ep_h_object_jumppad01-standard_1"))
                && octopus.iter().any(|o| o["transform"] == shell["transform"]),
            "shell not an exact redundant visual",
        )?;
    }
    visuals.retain(|v| !shells.contains(v));
    let objects = docs.get_mut(&paths[2]).unwrap()["objects"]
        .as_array_mut()
        .ok_or("objects")?;
    let removed = objects
        .iter()
        .filter(|o| {
            o["sourceNode"]
                .as_str()
                .is_some_and(|s| SHELLS.contains(&s))
        })
        .cloned()
        .collect::<Vec<_>>();
    require(
        removed.len() == shells.len(),
        "visual/object ownership differs",
    )?;
    for o in &removed {
        let g = o["sourceGeometry"].as_object().ok_or("geometry")?;
        require(
            g.len() == 1
                && shells
                    .iter()
                    .any(|v| v["model"].as_str().is_some_and(|s| g.contains_key(s))),
            "never remove a collider owner",
        )?;
    }
    objects.retain(|o| !removed.contains(o));
    let count = objects.len();
    let entries = docs.get_mut(&paths[4]).unwrap()["tiles"]
        .as_array_mut()
        .ok_or("tiles")?
        .iter_mut()
        .filter(|t| t["tileId"] == "map_08_06")
        .collect::<Vec<_>>();
    require(entries.len() == 1, "tile catalog ownership")?;
    let entry = entries.into_iter().next().unwrap();
    require(
        entry["instanceCount"] == count + removed.len(),
        "instance count differs",
    )?;
    entry["instanceCount"] = json!(count);
    let mut outputs = Vec::new();
    for path in paths {
        let doc = docs.get_mut(&path).unwrap();
        refresh(doc, &outputs);
        let bytes = rewrite(&originals[&path], doc)?;
        outputs.push((path, bytes));
    }
    Ok(outputs)
}
