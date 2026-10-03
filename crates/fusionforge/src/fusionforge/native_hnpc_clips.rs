//! Recover missing player/HNPC clips from their exact primary Animation owner.
use super::{native_nano_voice::recover, native_publication as p, native_texture_sharing as g};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};
const DEFAULT: &str = "stand2,stand3,stand4,rifleguard,swordguard,talk,talkexclamation,talkquestion,scratch,foldedarms,report,usestanding,observe,rocketstand4,rocketstand3,stun";
fn names(value: &str) -> Result<BTreeSet<String>, String> {
    let mut out = BTreeSet::new();
    for name in value.split(',') {
        if name.is_empty()
            || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            || !out.insert(name.to_string())
        {
            return Err("clip names must be unique comma-separated identifiers".into());
        }
    }
    Ok(out)
}
fn playback(name: &str) -> &'static str {
    if ["jumpstart", "jumpend", "jumplandrun"]
        .iter()
        .any(|s| name.ends_with(s))
        || [
            "stun",
            "woundupper",
            "stickdash",
            "rifledash",
            "rifletumbling",
            "rocketsomersault",
            "stickdodgeupper",
            "rifledodgeupper",
        ]
        .contains(&name)
    {
        "clamp"
    } else {
        "loop"
    }
}

pub(super) fn run(
    target: &Path,
    work: &Path,
    source: &Path,
    requested: Option<&str>,
    semantic: Option<&str>,
    apply: bool,
) -> Result<(), String> {
    let requested = names(requested.unwrap_or(DEFAULT))?;
    let sha = "96a3362fdfff0da0107bd1aea5666431b417380c9855c6f09fceebc8d372ae93";
    let asset = "CustomAssetBundle-ce09c4c9be8a046ca92e0044f22d1b99";
    let contract_route = "characters/player/shared/player_rig_contract.json";
    let contract_bytes =
        fs::read(p::relative(target, contract_route)?).map_err(|e| e.to_string())?;
    let contract: Value = serde_json::from_slice(&contract_bytes).map_err(|e| e.to_string())?;
    let genders = contract["genders"].as_array().ok_or("rig genders")?;
    if genders.len() != 2
        || genders
            .iter()
            .map(|g| g["gender"].as_str().unwrap_or(""))
            .collect::<BTreeSet<_>>()
            != BTreeSet::from(["male", "female"])
    {
        return Err("expected unique male/female rigs".into());
    }
    let mut routes = vec![contract_route.to_string()];
    for gender in genders {
        routes.push(gender["skeletonGlb"].as_str().ok_or("rig GLB")?.into());
    }
    if let Some(path) = semantic {
        routes.push(path.into());
    }
    let expected = p::snapshot(target, &routes)?;
    if expected[contract_route].as_deref() != Some(p::digest(&contract_bytes).as_str()) {
        return Err("rig contract changed before conversion".into());
    }
    let mut selected = vec![];
    let mut names_by_id = None;
    for gender in genders {
        let label = gender["gender"].as_str().unwrap();
        let route = gender["skeletonGlb"].as_str().unwrap();
        let raw = fs::read(p::relative(target, route)?).map_err(|e| e.to_string())?;
        let (doc, _) = g::decode(&raw, true)?;
        let animations = doc["animations"].as_array().ok_or("rig animations")?;
        let existing: BTreeSet<_> = animations
            .iter()
            .map(|a| a["name"].as_str().unwrap_or("").to_string())
            .collect();
        if existing.len() != animations.len() {
            return Err("ambiguous existing animation names".into());
        }
        let missing: BTreeSet<_> = requested.difference(&existing).cloned().collect();
        if !missing.is_empty() && names_by_id.is_none() {
            names_by_id = Some(super::cli::animation_clip_names(
                &source.join("CharacterSelection.resourceFile"),
                asset,
            )?);
        }
        let owner = recover(
            source,
            work,
            "primary",
            "CharacterSelection.resourceFile",
            asset,
            "Animation",
            gender["animationComponentPathId"]
                .as_i64()
                .ok_or("rig Animation owner")?,
            &format!("{label}-owner"),
            sha,
        )?;
        let mut objects = vec![];
        let mut found = BTreeSet::new();
        for ptr in owner["pointers"].as_array().ok_or("owner pointers")? {
            let target = &ptr["target"];
            let Some(name) = target["pathId"]
                .as_i64()
                .and_then(|id| names_by_id.as_ref().and_then(|m| m.get(&id)))
                .map(String::as_str)
            else {
                continue;
            };
            if !ptr["fieldPath"]
                .as_str()
                .is_some_and(|s| s.starts_with("/m_Animations/"))
                || !missing.contains(name)
            {
                continue;
            }
            if ptr["status"] != "resolved"
                || target["serializedAsset"] != asset
                || target["type"] != "AnimationClip"
                || !found.insert(name.to_string())
            {
                return Err("ambiguous or external HNPC clip owner".into());
            }
            let evidence = recover(
                source,
                work,
                "primary",
                "CharacterSelection.resourceFile",
                asset,
                "AnimationClip",
                target["pathId"].as_i64().ok_or("clip path ID")?,
                &format!("{label}-{name}"),
                sha,
            )?;
            if evidence["object"]["name"] != name {
                return Err("HNPC clip name changed".into());
            }
            objects.push(evidence["object"].clone());
        }
        if found != missing {
            return Err(format!(
                "missing scoped {label} clips: {:?}",
                missing.difference(&found).collect::<Vec<_>>()
            ));
        }
        objects.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        p::write(&work.join(format!("{label}.json")), &p::encode(&objects)?)?;
        selected.push((label.to_string(), route.to_string(), raw, missing));
    }
    if selected.iter().any(|(_, _, _, m)| !m.is_empty()) {
        ffone_asset_pipeline::export_player_rig_clip_additions(
            target,
            work,
            &work.join("additions"),
        )
        .map_err(|e| e.to_string())?;
    }
    let mut outputs = vec![];
    let mut semantic_genders = vec![];
    for (label, route, original, missing) in selected {
        let bytes = if missing.is_empty() {
            original.clone()
        } else {
            let addition = work.join(format!("additions/{label}.glb"));
            let added = fs::read(&addition).map_err(|e| e.to_string())?;
            let old = g::decode(&original, true)?.0;
            let new = g::decode(&added, true)?.0;
            if old["nodes"] != new["nodes"] || old["scenes"] != new["scenes"] {
                return Err("HNPC native skeleton/rest pose differs".into());
            }
            let original_path = work.join(format!("{label}-before.glb"));
            p::write(&original_path, &original)?;
            let output = work.join(format!("{label}-complete.glb"));
            let mut args = vec![
                original_path.display().to_string(),
                addition.display().to_string(),
                output.display().to_string(),
            ];
            args.extend(missing);
            ffone_asset_pipeline::model_animation_append::run(args)?;
            fs::read(output).map_err(|e| e.to_string())?
        };
        let doc = g::decode(&bytes, true)?.0;
        let mut clips = vec![];
        for (index, a) in doc["animations"]
            .as_array()
            .ok_or("merged animations")?
            .iter()
            .enumerate()
        {
            let name = a["name"].as_str().ok_or("clip name")?;
            if !requested.contains(name) {
                continue;
            }
            let mut duration = 0.0f64;
            for s in a["samplers"].as_array().ok_or("clip samplers")? {
                let time = doc["accessors"][s["input"].as_u64().ok_or("input accessor")? as usize]
                    ["max"][0]
                    .as_f64()
                    .ok_or("clip duration")?;
                if !time.is_finite() || time < 0.0 {
                    return Err("invalid clip duration".into());
                }
                duration = duration.max(time);
            }
            clips.push(json!({"name":name,"gltfAnimationIndex":index,"channelCount":a["channels"].as_array().ok_or("channels")?.len(),"durationSeconds":duration,"playback":playback(name)}));
        }
        semantic_genders.push(json!({"gender":label,"clips":clips}));
        outputs.push((route, bytes));
    }
    if let Some(route) = semantic {
        let value =
            json!({"schema":"ffone.player-animation-catalog.v1","genders":semantic_genders});
        let path = p::relative(target, route)?;
        let bytes = match fs::read(path) {
            Ok(before) => super::native_json_patch::rewrite(&before, &value)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => p::encode(&value)?,
            Err(e) => return Err(e.to_string()),
        };
        outputs.push((route.into(), bytes));
    }
    if p::digest(
        &fs::read(source.join("CharacterSelection.resourceFile")).map_err(|e| e.to_string())?,
    ) != sha
    {
        return Err("primary rig source changed during conversion".into());
    }
    p::install_checked(target, work, &outputs, apply, &expected)?;
    Ok(())
}

#[cfg(test)]
mod tests;
