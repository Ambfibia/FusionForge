//! Bounded direct conversion of primary shared-rig clips, preserving existing assets.
use super::{direct_input, native_publication as publication, native_texture_sharing as glb, unity::UnityValue};
use serde_json::{Value, json};
use std::{fs, path::{Path, PathBuf}};

fn full(value: &UnityValue) -> Value {
    match value {
        UnityValue::Array(v) => Value::Array(v.iter().map(full).collect()),
        UnityValue::Object(v) => Value::Object(v.iter().map(|(k,v)| (k.clone(), full(v))).collect()),
        UnityValue::Pair(a,b) => json!([full(a),full(b)]),
        UnityValue::Bytes(v) => json!({"bytes":v.len(),"base64":base64::Engine::encode(&base64::engine::general_purpose::STANDARD,v)}),
        _ => value.to_json_sample(),
    }
}

pub(super) fn run(args: &[String]) -> Result<(), String> {
    run_clip(args, "convert-player-launcher", "luncher", [34376,34577], "launcher_animations.json", "clamp")
}

pub(super) fn run_walk(args: &[String]) -> Result<(), String> {
    run_clip(args, "convert-player-walk", "walk", [34461,34550], "locomotion_animations.json", "loop")
}

fn run_clip(args: &[String], command: &str, name: &str, ids: [i64; 2], catalog: &str, playback: &str) -> Result<(), String> {
    if args.len() < 2 || args[2..].iter().any(|s| !["--check", "--replace-existing"].contains(&s.as_str())) {
        return Err(format!("{command} <CharacterSelection.resourceFile> <native-root> [--check] [--replace-existing]"));
    }
    let source = Path::new(&args[0]);
    let root = Path::new(&args[1]);
    if publication::digest(&fs::read(source).map_err(|e|e.to_string())?) != "96a3362fdfff0da0107bd1aea5666431b417380c9855c6f09fceebc8d372ae93" {
        return Err("unrecognized primary source; native assets preserved".into());
    }
    let assets = direct_input::read_assets(source)?;
    let [asset] = assets.as_slice() else { return Err("ambiguous source asset".into()); };
    let contract_path = root.join(ffone_asset_pipeline::PLAYER_SHARED_RIG_CONTRACT_PATH);
    let contract_bytes = fs::read(&contract_path).map_err(|e|e.to_string())?;
    let contract: ffone_asset_pipeline::PlayerSharedRigContract = serde_json::from_slice(&contract_bytes).map_err(|e|e.to_string())?;
    let mut files = Vec::new();
    let mut snapshots = Vec::new();
    let mut genders = Vec::new();
    for gender in &contract.genders {
        let owner = asset.objects.get(&gender.animation_component_path_id).ok_or("missing Animation owner")?;
        let owner = full(&asset.read_object(0, owner)?);
        println!("{:?} Animation wrap={}",gender.gender,owner["m_WrapMode"]);
        let mut selected = Vec::new();
        // Only this exact primary pair is admitted; ownership chooses the gender.
        for id in ids {
            if !owner["m_Animations"].as_array().ok_or("owner clips")?.iter().any(|p|p["fileId"] == 0 && p["pathId"] == id) { continue; }
            let info = asset.objects.get(&id).ok_or("missing source clip")?;
            let value = asset.read_object(0,info)?;
            if asset.object_type_name(info) != "AnimationClip" || super::unity::object_name(&value) != name { return Err("source clip identity changed".into()); }
            let value = full(&value);
            println!("{:?}: {name} #{id}, settings={:?}", gender.gender, value.as_object().unwrap().iter().filter(|(k,_)|k.to_ascii_lowercase().contains("wrap") || k.to_ascii_lowercase().contains("clipsettings")).collect::<Vec<_>>());
            selected.push(json!({"pathId":id,"type":"AnimationClip","name":name,"value":value}));
        }
        if selected.len() != 1 { return Err(format!("ambiguous gender {name} clip")); }
        let donor = ffone_asset_pipeline::encode_player_rig_clip_additions(&contract_bytes,gender.gender,json!(selected)).map_err(|e|e.to_string())?;
        let (donor_doc,_) = glb::decode(&donor,true)?;
        let route = PathBuf::from(&gender.skeleton_glb);
        let original = fs::read(root.join(&route)).map_err(|e|e.to_string())?;
        let (doc,_) = glb::decode(&original,true)?;
        let animations = doc["animations"].as_array().ok_or("native animations")?;
        let existing: Vec<_> = animations.iter().enumerate().filter(|(_,a)|a["name"] == name).collect();
        let (index, output) = match existing.as_slice() {
            [] => (animations.len(), ffone_asset_pipeline::model_animation_append::append_bytes(&original,&donor,&[name.to_owned()])?),
            [(index, _)] => (*index, original.clone()),
            _ => return Err(format!("ambiguous native {name}")),
        };
        ffone_asset_pipeline::model_animation_append::selected_clips_match(&output,&donor,&[name.to_owned()])?;
        let clip = &donor_doc["animations"][0];
        let duration = clip["samplers"].as_array().ok_or("clip samplers")?.iter().filter_map(|s| {
            donor_doc["accessors"][s["input"].as_u64()? as usize]["max"][0].as_f64()
        }).fold(0.0_f64,f64::max);
        if duration <= 0.0 { return Err(format!("invalid {name} duration")); }
        genders.push(json!({"gender":gender.gender,"clips":[{"name":name,"gltfAnimationIndex":index,"channelCount":clip["channels"].as_array().ok_or("channels")?.len(),"durationSeconds":duration,"playback":playback}]}));
        snapshots.push((route.clone(),original));
        files.push((route,output));
    }
    files.push((PathBuf::from(format!("characters/player/shared/{catalog}")),serde_json::to_vec_pretty(&json!({"schema":"ffone.player-animation-catalog.v1","genders":genders})).map_err(|e|e.to_string())?));
    if fs::read(&contract_path).map_err(|e|e.to_string())? != contract_bytes || snapshots.iter().any(|(p,b)|fs::read(root.join(p)).ok().as_ref()!=Some(b)) { return Err("native rig changed during conversion".into()); }
    let replace = args.iter().any(|s|s == "--replace-existing");
    if args.iter().any(|s|s == "--check") { ffone_asset_pipeline::direct_output::check_with_permission(root,&files,replace)?; }
    else { ffone_asset_pipeline::direct_output::install_with_permission(root,&files,replace)?; }
    println!("primary {name}: two genders, existing clips and render assets preserved");
    Ok(())
}
