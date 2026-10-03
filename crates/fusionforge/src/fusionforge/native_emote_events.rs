//! Recover native emote event code from exact, owner-validated primary clips.
use super::native_publication as p;
use serde_json::{Value, json};
use std::{fs, path::Path};

const CLIPS: &[(&str, i64, i64)] = &[
    ("Cry", 34471, 34494),
    ("Angry", 34304, 34161),
    ("Shocked", 34383, 34629),
    ("Hello", 34598, 34192),
    ("Thank", 34362, 34476),
    ("Dance1", 34566, 34283),
    ("Kiss", 34427, 34602),
    ("Agree", 34319, 34201),
    ("Laugh", 34353, 34377),
    ("No", 34410, 34250),
    ("Flex", 34558, 34218),
    ("Tease", 34623, 34547),
    ("Ok", 34423, 34431),
    ("Applaud", 34217, 34149),
    ("Cheer", 34405, 34515),
    ("Dance2", 34285, 34430),
    ("Dance3", 34225, 34536),
    ("Dance4", 34635, 34634),
    ("Dance5", 34316, 34628),
    ("Goodbye", 34310, 34575),
    ("Beach1", 34211, 34573),
    ("Beach2", 34402, 34563),
    ("Beach3", 34433, 34552),
];

fn events(value: &Value) -> Result<(f64, Vec<(f64, String)>), String> {
    let rows = value.as_array().ok_or("missing clip events")?;
    let mut end = None;
    let mut sounds = vec![];
    for row in rows {
        let name = row["functionName"].as_str().ok_or("event function")?;
        if !matches!(name, "end" | "sound") {
            continue;
        }
        let time = row["time"].as_f64().ok_or("event time")?;
        if !time.is_finite() || time < 0.0 {
            return Err("invalid event time".into());
        }
        if name == "end" {
            if end.replace(time).is_some() {
                return Err("duplicate end event".into());
            }
        } else {
            sounds.push((time, row["data"].as_str().ok_or("sound data")?.to_owned()));
        }
    }
    Ok((end.ok_or("missing end event")?, sounds))
}

pub(super) fn run(target: &Path, work: &Path, source: &Path, apply: bool) -> Result<(), String> {
    const ROUTE: &str = "crates/ffone-client/src/player_emote/events.rs";
    const BUNDLE: &str = "CharacterSelection.resourceFile";
    const ASSET: &str = "CustomAssetBundle-ce09c4c9be8a046ca92e0044f22d1b99";
    const SHA: &str = "96a3362fdfff0da0107bd1aea5666431b417380c9855c6f09fceebc8d372ae93";
    let expected = p::snapshot(target, &[ROUTE.into()])?;
    let mut requests = vec![(234214, "Animation"), (234274, "Animation")];
    for &(_, m, f) in CLIPS {
        requests.extend([(m, "AnimationClip"), (f, "AnimationClip")]);
    }
    let evidence = super::cli::scoped_evidence_batch(source, BUNDLE, ASSET, SHA, &requests)?;
    p::write(
        &work.join("events-evidence.json"),
        &p::encode(&json!(evidence))?,
    )?;
    let mut text = String::from(
        "// Native player emote timing and semantic sound events.\n// Maintained by FusionForge repair-native player-emote-events.\nuse super::{PlayerEmoteEvents, PlayerRigGender, TutorialPlayerClip};\npub(super) const fn events(gender: PlayerRigGender, clip: TutorialPlayerClip) -> Option<PlayerEmoteEvents> {\n    match (gender, clip) {\n",
    );
    let mut contracts = vec![];
    for (index, &(clip, male, female)) in CLIPS.iter().enumerate() {
        for (gender_index, (gender, id)) in
            [("Male", male), ("Female", female)].into_iter().enumerate()
        {
            let owner = &evidence[gender_index];
            if !owner["pointers"]
                .as_array()
                .ok_or("Animation pointers")?
                .iter()
                .any(|ptr| {
                    ptr["target"]["pathId"] == id
                        && ptr["target"]["serializedAsset"] == ASSET
                        && ptr["target"]["type"] == "AnimationClip"
                })
            {
                return Err(format!("{gender} does not own {clip}"));
            }
            let obj = &evidence[2 + index * 2 + gender_index]["object"];
            if obj["pathId"] != id || obj["name"] != clip.to_lowercase() {
                return Err(format!("unexpected scoped clip {clip}"));
            }
            let (end, sounds) = events(&obj["value"]["m_Events"])?;
            // Debug string formatting emits valid Rust escapes, including arbitrary sound text.
            let sound_text = sounds
                .iter()
                .map(|(t, s)| format!("({t:?}, {s:?})"))
                .collect::<Vec<_>>()
                .join(", ");
            text.push_str(&format!("        (PlayerRigGender::{gender}, TutorialPlayerClip::{clip}) => Some(PlayerEmoteEvents {{ end: {end:?}, sounds: &[{sound_text}] }}),\n"));
            contracts.push(json!({"gender":gender,"clip":clip,"end":end,"sounds":sounds}));
        }
    }
    text.push_str("        _ => None,\n    }\n}\n");
    let staged = work.join("events.rs");
    p::write(&staged, text.as_bytes())?;
    let status = std::process::Command::new("rustfmt")
        .args(["--edition", "2024"])
        .arg(&staged)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("rustfmt rejected generated event code".into());
    }
    p::write(
        &work.join("event-contracts.json"),
        &p::encode(&json!(contracts))?,
    )?;
    let bytes = fs::read(staged).map_err(|e| e.to_string())?;
    p::install_checked(target, work, &[(ROUTE.into(), bytes)], apply, &expected)?;
    Ok(())
}

#[cfg(test)]
mod tests;
