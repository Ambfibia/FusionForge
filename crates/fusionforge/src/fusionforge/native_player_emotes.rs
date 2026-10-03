//! Rebuild selected custom clips from immutable source, directly into existing rigs.
use super::{
    direct_input, native_publication as publication, native_texture_sharing as glb,
    unity::UnityValue,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    ops::Range,
    path::{Path, PathBuf},
};

fn full(value: &UnityValue) -> Value {
    match value {
        UnityValue::Array(v) => Value::Array(v.iter().map(full).collect()),
        UnityValue::Object(v) => {
            Value::Object(v.iter().map(|(k, v)| (k.clone(), full(v))).collect())
        }
        UnityValue::Pair(a, b) => json!([full(a), full(b)]),
        _ => value.to_json_sample(),
    }
}

fn data_range(doc: &Value, bytes: &[u8], index: usize) -> Result<Range<usize>, String> {
    let a = &doc["accessors"][index];
    let v = &doc["bufferViews"][a["bufferView"].as_u64().ok_or("accessor view")? as usize];
    let width = match a["type"].as_str() {
        Some("SCALAR") => 1,
        Some("VEC3") => 3,
        Some("VEC4") => 4,
        _ => return Err("unsupported animation accessor".into()),
    };
    if a["componentType"] != 5126
        || !a["sparse"].is_null()
        || !v["byteStride"].is_null()
        || v["buffer"] != 0
    {
        return Err("unsupported animation storage".into());
    }
    let start = 28
        + u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize
        + v["byteOffset"].as_u64().unwrap_or(0) as usize
        + a["byteOffset"].as_u64().unwrap_or(0) as usize;
    let len = a["count"].as_u64().ok_or("accessor count")? as usize * width * 4;
    let end = start.checked_add(len).ok_or("accessor overflow")?;
    if end > bytes.len() {
        return Err("truncated animation data".into());
    }
    Ok(start..end)
}

fn approximately_equal(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len()
        && a.chunks_exact(4).zip(b.chunks_exact(4)).all(|(a, b)| {
            let a = f32::from_le_bytes(a.try_into().unwrap());
            let b = f32::from_le_bytes(b.try_into().unwrap());
            a.is_finite() && b.is_finite() && (a - b).abs() <= 1e-5 * (1.0 + b.abs())
        })
}

fn replace_payloads(original: &[u8], before: &[u8], after: &[u8]) -> Result<Vec<u8>, String> {
    let (target, _) = glb::decode(original, true)?;
    let (raw, _) = glb::decode(before, true)?;
    let (fixed, _) = glb::decode(after, true)?;
    let mut result = original.to_vec();
    let mut touched = BTreeSet::new();
    for (raw_animation, fixed_animation) in raw["animations"]
        .as_array()
        .ok_or("source animations")?
        .iter()
        .zip(
            fixed["animations"]
                .as_array()
                .ok_or("retargeted animations")?,
        )
    {
        let name = raw_animation["name"].as_str().ok_or("clip name")?;
        let matches: Vec<_> = target["animations"]
            .as_array()
            .ok_or("native animations")?
            .iter()
            .filter(|a| a["name"] == name)
            .collect();
        let [animation] = matches.as_slice() else {
            return Err(format!("missing/ambiguous native {name}"));
        };
        for (source_channel, fixed_channel) in raw_animation["channels"]
            .as_array()
            .ok_or("source channels")?
            .iter()
            .zip(
                fixed_animation["channels"]
                    .as_array()
                    .ok_or("fixed channels")?,
            )
        {
            let matches: Vec<_> = animation["channels"]
                .as_array()
                .ok_or("native channels")?
                .iter()
                .filter(|c| c["target"] == source_channel["target"])
                .collect();
            let [channel] = matches.as_slice() else {
                return Err(format!("unmatched channel in {name}"));
            };
            let node = channel["target"]["node"].as_u64().ok_or("node")? as usize;
            if target["nodes"][node]["name"] != raw["nodes"][node]["name"] {
                return Err("rig node identity differs".into());
            }
            let sampler =
                &animation["samplers"][channel["sampler"].as_u64().ok_or("sampler")? as usize];
            let raw_sampler = &raw_animation["samplers"]
                [source_channel["sampler"].as_u64().ok_or("source sampler")? as usize];
            let fixed_sampler = &fixed_animation["samplers"]
                [fixed_channel["sampler"].as_u64().ok_or("fixed sampler")? as usize];
            if sampler["interpolation"] != raw_sampler["interpolation"] {
                return Err(format!("local interpolation edit: {name}"));
            }
            let range = |doc: &Value, bytes: &[u8], sampler: &Value, field: &str| {
                data_range(
                    doc,
                    bytes,
                    sampler[field].as_u64().ok_or("accessor")? as usize,
                )
            };
            if original[range(&target, original, sampler, "input")?]
                != before[range(&raw, before, raw_sampler, "input")?]
            {
                return Err(format!("local timing edit: {name}"));
            }
            let output = sampler["output"].as_u64().ok_or("output")? as usize;
            if !touched.insert(output) {
                return Err("shared selected output accessor".into());
            }
            let dst = data_range(&target, original, output)?;
            let src = range(&raw, before, raw_sampler, "output")?;
            let corrected = range(&fixed, after, fixed_sampler, "output")?;
            let old = &original[dst.clone()];
            if !approximately_equal(old, &before[src])
                && !approximately_equal(old, &after[corrected.clone()])
            {
                return Err(format!(
                    "local curve edit preserved: {name} {:?}",
                    channel["target"]
                ));
            }
            if dst.len() != corrected.len() {
                return Err("retarget changed key count".into());
            }
            if !target["accessors"][output]["min"].is_null()
                || !target["accessors"][output]["max"].is_null()
            {
                return Err("bounded output accessor requires metadata rebuild".into());
            }
            result[dst].copy_from_slice(&after[corrected]);
        }
    }
    // A curve may never share an output with an unrelated animation.
    for output in touched {
        let uses = target["animations"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|a| a["samplers"].as_array().unwrap())
            .filter(|s| s["output"] == output)
            .count();
        if uses != 1 {
            return Err("animation output shared outside selected clips".into());
        }
    }
    gltf::Gltf::from_slice(&result).map_err(|e| e.to_string())?;
    Ok(result)
}

pub(super) fn run(args: &[String]) -> Result<(), String> {
    if args.len() < 2
        || args[2..]
            .iter()
            .any(|s| !["--check", "--replace-existing"].contains(&s.as_str()))
    {
        return Err(
            "convert-player-emotes <FFR-bundle> <native-root> [--check] [--replace-existing]"
                .into(),
        );
    }
    let source = Path::new(&args[0]);
    let root = Path::new(&args[1]);
    let source_bytes = fs::read(source).map_err(|e| e.to_string())?;
    if publication::digest(&source_bytes)
        != "2c5961ddf656034acdad8b4c1a1a10e8ac156eaad2a1484edb9bb45e0205a091"
    {
        return Err("unrecognized FFR source; native files preserved".into());
    }
    let assets = direct_input::read_assets(source)?;
    let [asset] = assets.as_slice() else {
        return Err("ambiguous source asset".into());
    };
    let ids = [
        644, 624, 641, 629, 607, 604, 610, 646, 637, 585, 648, 639, 623,
    ];
    let mut objects = Vec::new();
    for id in ids {
        let info = asset.objects.get(&id).ok_or("missing source clip")?;
        let value = asset.read_object(0, info)?;
        objects.push(json!({"pathId":id,"type":asset.object_type_name(info),"name":super::unity::object_name(&value),"value":full(&value)}));
    }
    let contract_path = root.join(ffone_asset_pipeline::PLAYER_SHARED_RIG_CONTRACT_PATH);
    let contract = fs::read(&contract_path).map_err(|e| e.to_string())?;
    let path = PathBuf::from(ffone_asset_pipeline::MALE_SHARED_SKELETON_GLB_PATH);
    let original = fs::read(root.join(&path)).map_err(|e| e.to_string())?;
    let (before, after) =
        ffone_asset_pipeline::rebuild_male_emote_payloads(&contract, Value::Array(objects))
            .map_err(|e| e.to_string())?;
    let output = replace_payloads(&original, &before, &after)?;
    if fs::read(&contract_path).map_err(|e| e.to_string())? != contract
        || fs::read(root.join(&path)).map_err(|e| e.to_string())? != original
    {
        return Err("native rig changed during conversion".into());
    }
    let changed = original != output;
    let files = [(path, output)];
    let replace = args.iter().any(|s| s == "--replace-existing");
    if args.iter().any(|s| s == "--check") {
        ffone_asset_pipeline::direct_output::check_with_permission(root, &files, replace)?;
    } else {
        ffone_asset_pipeline::direct_output::install_with_permission(root, &files, replace)?;
    }
    println!("male custom emotes: 13 clips validated; changed={changed}");
    Ok(())
}
