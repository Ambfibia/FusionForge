//! Scoped Academy voice-event donor, without changing existing render resources.
use super::{native_publication as p, native_texture_sharing as glb};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
pub(super) fn recover(
    root: &Path,
    work: &Path,
    alias: &str,
    bundle: &str,
    asset: &str,
    kind: &str,
    id: i64,
    label: &str,
    sha: &str,
) -> Result<Value, String> {
    require(
        p::digest(&fs::read(p::relative(root, bundle)?).map_err(|e| e.to_string())?) == sha,
        "raw donor container changed",
    )?;
    let out = work.join("evidence").join(format!("{label}.json"));
    super::cli::run_cli(vec![
        "dump-object-evidence".into(),
        alias.into(),
        root.display().to_string(),
        bundle.into(),
        id.to_string(),
        "--serialized-asset".into(),
        asset.into(),
        "--type".into(),
        kind.into(),
        "--out".into(),
        out.display().to_string(),
    ])?;
    let evidence = p::read_json(&out)?;
    require(
        evidence["source"]["sha256"] == sha
            && evidence["object"]["serializedAsset"] == asset
            && evidence["object"]["type"] == kind
            && evidence["object"]["pathId"] == id
            && evidence["triage"]["unresolvedPointerCount"] == 0,
        "scoped donor evidence mismatch",
    )?;
    Ok(evidence)
}

fn paths(doc: &Value) -> Result<BTreeMap<usize, Vec<String>>, String> {
    fn visit(
        doc: &Value,
        index: usize,
        mut parent: Vec<String>,
        out: &mut BTreeMap<usize, Vec<String>>,
    ) -> Result<(), String> {
        parent.push(
            doc["nodes"][index]["name"]
                .as_str()
                .ok_or("unnamed node")?
                .into(),
        );
        require(
            !out.contains_key(&index) && !out.values().any(|p| p == &parent),
            "ambiguous hierarchy",
        )?;
        out.insert(index, parent.clone());
        if let Some(children) = doc["nodes"][index]["children"].as_array() {
            for child in children {
                visit(
                    doc,
                    child.as_u64().ok_or("node index")? as usize,
                    parent.clone(),
                    out,
                )?;
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    let scene = doc["scene"].as_u64().unwrap_or(0) as usize;
    for n in doc["scenes"][scene]["nodes"]
        .as_array()
        .ok_or("scene roots")?
    {
        visit(
            doc,
            n.as_u64().ok_or("root node")? as usize,
            vec![],
            &mut out,
        )?;
    }
    require(
        out.len() == doc["nodes"].as_array().ok_or("nodes")?.len(),
        "unowned nodes",
    )?;
    Ok(out)
}
fn curves(doc: &Value, suffix: &[u8], a: &Value) -> Result<Vec<String>, String> {
    require(
        suffix.get(4..8) == Some(&0x004e4942u32.to_le_bytes()),
        "missing BIN",
    )?;
    let binary = suffix.get(8..).ok_or("missing BIN payload")?;
    let paths = paths(doc)?;
    let values = |index: &Value| -> Result<Value, String> {
        let a = &doc["accessors"][index.as_u64().ok_or("accessor index")? as usize];
        require(
            a["componentType"] == 5126 && a.get("sparse").is_none(),
            "unsupported clip accessor",
        )?;
        let size = match a["type"].as_str() {
            Some("SCALAR") => 4,
            Some("VEC3") => 12,
            Some("VEC4") => 16,
            _ => return Err("clip accessor shape".into()),
        };
        let view = &doc["bufferViews"][a["bufferView"].as_u64().ok_or("view index")? as usize];
        require(view["buffer"] == 0, "external clip buffer")?;
        let view_start = view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let view_len = view["byteLength"].as_u64().ok_or("view length")? as usize;
        let data = binary
            .get(view_start..view_start.checked_add(view_len).ok_or("view overflow")?)
            .ok_or("view outside BIN")?;
        let offset = a["byteOffset"].as_u64().unwrap_or(0) as usize;
        let stride = view["byteStride"].as_u64().unwrap_or(size) as usize;
        require(stride >= size as usize, "invalid clip stride")?;
        let count = a["count"].as_u64().ok_or("accessor count")? as usize;
        let mut bytes = vec![];
        for i in 0..count {
            let start = i
                .checked_mul(stride)
                .and_then(|n| n.checked_add(offset))
                .ok_or("accessor overflow")?;
            bytes.extend_from_slice(
                data.get(
                    start
                        ..start
                            .checked_add(size as usize)
                            .ok_or("accessor overflow")?,
                )
                .ok_or("accessor outside view")?,
            );
        }
        Ok(json!([a["type"], count, STANDARD.encode(bytes)]))
    };
    let mut out = vec![];
    for c in a["channels"].as_array().ok_or("channels")? {
        let s = &a["samplers"][c["sampler"].as_u64().ok_or("sampler index")? as usize];
        out.push(
            json!([
                paths
                    .get(&(c["target"]["node"].as_u64().ok_or("channel node")? as usize))
                    .ok_or("unowned channel node")?,
                c["target"]["path"],
                s["interpolation"].as_str().unwrap_or("LINEAR"),
                values(&s["input"])?,
                values(&s["output"])?
            ])
            .to_string(),
        );
    }
    out.sort();
    Ok(out)
}
fn sounds(animation: &mut Value, source: &Value) -> Result<(), String> {
    let duration = animation["extras"]["duration"]
        .as_f64()
        .ok_or("animation duration")?;
    let donor: Vec<_> = source["m_Events"]
        .as_array()
        .ok_or("source events")?
        .iter()
        .filter(|e| e["functionName"] == "sound")
        .collect();
    require(!donor.is_empty(), "donor sound events missing")?;
    let events = animation["extras"]["nonTrs"]["events"]
        .as_array_mut()
        .ok_or("native events")?;
    let mut replacement: Vec<_> = events
        .iter()
        .filter(|e| e["functionName"] != "sound")
        .cloned()
        .collect();
    for event in donor {
        let time = event["time"].as_f64().ok_or("sound time")?;
        require(
            time.is_finite() && time >= 0.0 && time <= duration,
            "sound outside clip",
        )?;
        replacement.push(json!({"time":event["time"],"functionName":"sound","stringParameter":event["data"],"floatParameter":0.0,"intParameter":0,"messageOptions":event["messageOptions"],"objectParameter":null,"objectParameterProvenance":{"presence":"missing","interpretation":"missing"}}));
    }
    replacement.sort_by(|a, b| {
        a["time"]
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&b["time"].as_f64().unwrap_or(0.0))
    });
    *events = replacement;
    Ok(())
}

pub(super) fn run(
    target: &Path,
    work: &Path,
    academy: &Path,
    primary: &Path,
    navigation: Option<&Path>,
    apply: bool,
) -> Result<(), String> {
    let table_before =
        fs::read(p::relative(target, "data/tables/table-set.json")?).map_err(|e| e.to_string())?;
    let table: Value = serde_json::from_slice(&table_before).map_err(|e| e.to_string())?;
    let matches: Vec<_> = table["tables"]
        .as_array()
        .ok_or("tables")?
        .iter()
        .filter(|t| t["name"] == "native_asset_routes")
        .flat_map(|t| t["value"]["m_pAudioData"].as_array().into_iter().flatten())
        .filter(|r| r["trueName"] == "Pirate_Melee1" && r["category"] == "sfx")
        .collect();
    require(
        matches.len() == 1,
        "expected unique native Pirate SFX route",
    )?;
    let pirate_route = matches[0]["path"]
        .as_str()
        .ok_or("Pirate SFX path")?
        .to_owned();
    require(
        pirate_route.starts_with("audio/sfx/"),
        "Pirate SFX ownership differs",
    )?;
    let sources = [
        (
            "nano_flapjack",
            44,
            2742116923i64,
            [354693194i64, 2820527001, 3615652454, 708455227],
            "d58f2940ac87e786cd7b340af6616187ebd685e91b4624a29e6a51ddf66d76ca",
        ),
        (
            "nano_chowder",
            46,
            2187236750i64,
            [1511668751, 1934724603, 1982085174, 671541331],
            "fbf9c254aeb9539c07cab1c22f5f358d8d7746aa0faf80724517c7b4be38034f",
        ),
        (
            "nano_zaksaturday",
            49,
            3868737284i64,
            [3621751189, 282365582, 3158146062, 1667824429],
            "e6fe25af51adc13168ea260557af9392b25fabdb10eabdf8af6e272c6420e89d",
        ),
    ];
    let mut routes: Vec<String> = sources
        .iter()
        .map(|(m, ..)| format!("characters/nanos/{m}/{m}.glb"))
        .collect();
    routes.extend([
        "characters/nanos/nano_jackolantern/nano_jackolantern.glb".into(),
        pirate_route.clone(),
        "data/tables/table-set.json".into(),
    ]);
    let expected = p::snapshot(target, &routes)?;
    require(
        expected["data/tables/table-set.json"].as_deref()
            == Some(p::digest(&table_before).as_str()),
        "native table changed before recovery",
    )?;
    let mut outputs = vec![];
    for (model, number, owner, selection, sha) in sources {
        let bundle = format!("Nano_{number:03}.resourceFile");
        let asset = format!("CustomAssetBundle-Nano_{number:03}");
        let evidence = recover(
            academy,
            work,
            "alternate",
            &bundle,
            &asset,
            "Animation",
            owner,
            &format!("{model}-owner"),
            sha,
        )?;
        for id in selection {
            require(
                evidence["pointers"]
                    .as_array()
                    .ok_or("owner pointers")?
                    .iter()
                    .any(|p| {
                        p["fieldPath"]
                            .as_str()
                            .is_some_and(|s| s.starts_with("/m_Animations/"))
                            && p["status"] == "resolved"
                            && p["target"]["serializedAsset"] == asset
                            && p["target"]["type"] == "AnimationClip"
                            && p["target"]["pathId"] == id
                    }),
                "clip not owned by selected Animation",
            )?;
        }
        let route = format!("characters/nanos/{model}/{model}.glb");
        let path = p::relative(target, &route)?;
        let mut raw = fs::read(&path).map_err(|e| e.to_string())?;
        if model == "nano_chowder" {
            let source = work.join("chowder-source.json");
            let candidate = work.join("chowder-candidate");
            super::cli::run_cli(vec![
                "export-logical-model-source".into(),
                academy.join(&bundle).display().to_string(),
                "nano/nano_chowder.kfm".into(),
                source.display().to_string(),
                navigation.unwrap_or(academy).display().to_string(),
            ])?;
            ffone_asset_pipeline::cli::run([
                "publish-logical-model".into(),
                source.into_os_string(),
                "nano".into(),
                candidate.clone().into_os_string(),
            ])?;
            let donor = candidate.join("models/nano/nano_chowder.glb");
            let donor_bytes = fs::read(&donor).map_err(|e| e.to_string())?;
            let (d, db) = glb::decode(&donor_bytes, true)?;
            let (n, nb) = glb::decode(&raw, true)?;
            let old_paths = paths(&n)?;
            let new_paths = paths(&d)?;
            let inverse: BTreeMap<_, _> = old_paths.iter().map(|(i, p)| (p, i)).collect();
            require(
                inverse.len() == new_paths.len(),
                "Chowder hierarchy differs",
            )?;
            for (i, path) in new_paths {
                let j = **inverse.get(&path).ok_or("Chowder rest hierarchy differs")?;
                for key in ["translation", "rotation", "scale", "matrix"] {
                    require(
                        d["nodes"][i][key] == n["nodes"][j][key],
                        "Chowder rest pose differs",
                    )?;
                }
            }
            let mut missing = vec![];
            for name in ["skill1", "skill2", "skill3"] {
                let old: Vec<_> = n["animations"]
                    .as_array()
                    .ok_or("animations")?
                    .iter()
                    .filter(|a| a["name"] == name)
                    .collect();
                let new: Vec<_> = d["animations"]
                    .as_array()
                    .ok_or("donor animations")?
                    .iter()
                    .filter(|a| a["name"] == name)
                    .collect();
                require(new.len() == 1 && old.len() <= 1, "ambiguous Chowder clip")?;
                if let Some(old) = old.first() {
                    require(
                        curves(&n, &nb, old)? == curves(&d, &db, new[0])?,
                        "existing Chowder curves differ",
                    )?;
                } else {
                    missing.push(name.to_string());
                }
            }
            if !missing.is_empty() {
                let output = work.join("chowder-appended.glb");
                let mut args = vec![
                    path.display().to_string(),
                    donor.display().to_string(),
                    output.display().to_string(),
                ];
                args.extend(missing);
                ffone_asset_pipeline::model_animation_append::run(args)?;
                raw = fs::read(output).map_err(|e| e.to_string())?;
            }
        }
        let (mut doc, suffix) = glb::decode(&raw, true)?;
        for (name, id) in ["call", "skill1", "skill2", "skill3"]
            .into_iter()
            .zip(selection)
        {
            let evidence = recover(
                academy,
                work,
                "alternate",
                &bundle,
                &asset,
                "AnimationClip",
                id,
                &format!("{model}-{name}"),
                sha,
            )?;
            require(evidence["object"]["name"] == name, "clip name differs")?;
            let animations = doc["animations"].as_array_mut().ok_or("animations")?;
            require(
                animations.iter().filter(|a| a["name"] == name).count() == 1,
                "native clip ambiguous",
            )?;
            sounds(
                animations.iter_mut().find(|a| a["name"] == name).unwrap(),
                &evidence["object"]["value"],
            )?;
        }
        outputs.push((route, glb::encode(&raw, &doc, &suffix, true)?));
    }
    let route = "characters/nanos/nano_jackolantern/nano_jackolantern.glb";
    let raw = fs::read(p::relative(target, route)?).map_err(|e| e.to_string())?;
    let (mut doc, suffix) = glb::decode(&raw, true)?;
    for a in doc["animations"].as_array_mut().ok_or("Jack animations")? {
        if let Some(events) = a
            .pointer_mut("/extras/nonTrs/events")
            .and_then(Value::as_array_mut)
        {
            for e in events {
                if e["functionName"] == "sound" {
                    let name = e["stringParameter"].as_str().ok_or("sound key")?;
                    e["stringParameter"] =
                        json!(name.replace("Jack O'Lantern_", "Jack_O_Lantern_"));
                }
            }
        }
    }
    outputs.push((route.into(), glb::encode(&raw, &doc, &suffix, true)?));
    let sha = "782cda4c1c811a4d46139d441821b55fe386cc87156629f4387fddb39994ef2f";
    let evidence = recover(
        primary,
        work,
        "primary",
        "DongResources_08_06.resourceFile",
        "CustomAssetBundle-a3962f73ba4214b50b6d88e38442b1c2",
        "AudioClip",
        179,
        "pirate-melee1",
        sha,
    )?;
    let audio = STANDARD
        .decode(
            evidence
                .pointer("/object/value/audio data/base64")
                .and_then(Value::as_str)
                .ok_or("audio payload")?,
        )
        .map_err(|e| e.to_string())?;
    require(
        audio == fs::read(p::relative(target, &pirate_route)?).map_err(|e| e.to_string())?,
        "Pirate native SFX differs from raw payload",
    )?;
    let route = "data/tables/table-set.json";
    let raw = fs::read(p::relative(target, route)?).map_err(|e| e.to_string())?;
    let mut doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let table = doc["tables"]
        .as_array_mut()
        .ok_or("tables")?
        .iter_mut()
        .find(|t| t["name"] == "native_asset_routes")
        .ok_or("native routes")?;
    let rows = table["value"]["m_pAudioData"]
        .as_array_mut()
        .ok_or("audio rows")?;
    let removed: Vec<_> = rows
        .iter()
        .filter(|r| r["logicalKey"] == "voice/pirate/melee1")
        .collect();
    require(
        removed.is_empty() || (removed.len() == 1 && removed[0]["trueName"] == "Pirate_Melee1"),
        "unexpected Pirate voice row",
    )?;
    rows.retain(|r| r["logicalKey"] != "voice/pirate/melee1");
    outputs.push((route.into(), super::native_json_patch::rewrite(&raw, &doc)?));
    for (_, number, _, _, sha) in sources {
        require(
            p::digest(
                &fs::read(academy.join(format!("Nano_{number:03}.resourceFile")))
                    .map_err(|e| e.to_string())?,
            ) == sha,
            "Academy source changed during recovery",
        )?;
    }
    require(
        p::digest(
            &fs::read(primary.join("DongResources_08_06.resourceFile"))
                .map_err(|e| e.to_string())?,
        ) == sha,
        "primary source changed during recovery",
    )?;
    p::install_checked(target, work, &outputs, apply, &expected)?;
    Ok(())
}

#[cfg(test)]
mod tests;
