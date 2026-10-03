use super::native_publication::{digest, encode, install_checked, read_json, relative, snapshot};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn table_value<'a>(document: &'a mut Value, key: &str) -> Result<&'a mut Value, String> {
    let tables = document["tables"]
        .as_array_mut()
        .ok_or("missing native table-set")?;
    let mut matches = tables.iter_mut().filter(|t| t["value"].get(key).is_some());
    let first = matches
        .next()
        .ok_or_else(|| format!("missing table {key}"))?;
    ensure(matches.next().is_none(), "ambiguous table-set section")?;
    Ok(&mut first["value"])
}

fn retire_nano_alias(table: &mut Value) -> Result<(), String> {
    let rows = table["m_pNanoData"]
        .as_array_mut()
        .ok_or("missing Nano rows")?;
    ensure(
        rows.len() >= 71,
        "Nano roster must already include accepted additions",
    )?;
    ensure(
        rows[41]["m_iNanoNumber"] == 41 && rows[41]["m_iMesh"] == 40,
        "Nano 41 ownership differs",
    )?;
    ensure(
        rows[66]["m_iNanoNumber"] == 66 && rows[66]["m_iMesh"] == 52,
        "Van Kleiss ownership differs",
    )?;
    ensure(
        rows[52] == rows[0]
            || (rows[52]["m_iNanoNumber"] == 52
                && rows[52]["m_iMesh"] == 52
                && rows[52]["m_iNanoName"] == 51),
        "unrecognized alias 52",
    )?;
    rows[52] = rows[0].clone();
    ensure(
        rows.iter().all(|r| r["m_iNanoNumber"] != 52),
        "Nano 52 remains outside reserved slot",
    )
}

fn sync_nano_tuning(native: &mut Value, server: &mut Value) -> Result<(), String> {
    let value = table_value(native, "m_pNanoTable")?;
    ensure(
        value["m_pNanoTable"]["m_pNanoData"] == server["m_pNanoTable"]["m_pNanoData"],
        "Nano roster differs",
    )?;
    let tunes = value["m_pNanoTable"]["m_pNanoTuneData"]
        .as_array()
        .ok_or("native tunes")?;
    let old = server["m_pNanoTable"]["m_pNanoTuneData"]
        .as_array()
        .ok_or("server tunes")?;
    let skills = value["m_pSkillTable"]["m_pSkillData"]
        .as_array()
        .ok_or("native skills")?;
    let old_skills = server["m_pSkillTable"]["m_pSkillData"]
        .as_array()
        .ok_or("server skills")?;
    ensure(
        tunes.len() == old.len() && skills.starts_with(old_skills),
        "tuning/skill roster changed",
    )?;
    for (index, (new, old)) in tunes.iter().zip(old).enumerate() {
        let mut expected = old.clone();
        expected["m_iTuneNumber"] = json!(index);
        if index == 237 && old["m_iSkillID"] == 0 {
            expected["m_iSkillID"] = json!(286);
        }
        ensure(*new == expected, "unexpected tuning payload change")?;
    }
    for nano in value["m_pNanoTable"]["m_pNanoData"]
        .as_array()
        .ok_or("Nano rows")?
    {
        if nano["m_iNanoNumber"].as_i64().ok_or("Nano ID")? <= 0 {
            continue;
        }
        for index in nano["m_iTune"].as_array().ok_or("Nano tunes")? {
            let tune = tunes
                .get(index.as_u64().ok_or("tune index")? as usize)
                .ok_or("tune out of bounds")?;
            let skill = tune["m_iSkillID"].as_u64().ok_or("skill ID")?;
            ensure(
                skill > 0
                    && skills
                        .get(skill as usize)
                        .is_some_and(|s| s["m_iSkillNumber"] == skill),
                "invalid active Nano skill",
            )?;
        }
    }
    server["m_pNanoTable"]["m_pNanoTuneData"] = json!(tunes);
    server["m_pSkillTable"]["m_pSkillData"] = json!(skills);
    Ok(())
}

fn unstable_icon(value: &mut Value) -> Result<(), String> {
    let table = &mut value["m_pSkillTable"];
    ensure(
        table["m_pSkillIconData"][68] == json!({"m_iIconNumber":67,"m_iIconType":2}),
        "skill icon row 68 differs",
    )?;
    let skill = &mut table["m_pSkillData"][122];
    ensure(
        skill["m_iSkillNumber"] == 122 && (skill["m_iIcon"] == 1 || skill["m_iIcon"] == 68),
        "unexpected Unstable skill",
    )?;
    skill["m_iIcon"] = json!(68);
    Ok(())
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or("truncated GLB")?
            .try_into()
            .unwrap(),
    ))
}

fn fred_skin(bytes: &[u8]) -> Result<(Vec<u8>, usize), String> {
    ensure(
        bytes.starts_with(b"glTF")
            && u32_at(bytes, 4)? == 2
            && u32_at(bytes, 8)? as usize == bytes.len(),
        "not a complete GLB 2.0",
    )?;
    let n = u32_at(bytes, 12)? as usize;
    ensure(u32_at(bytes, 16)? == 0x4e4f534a, "missing JSON chunk")?;
    let document: Value = serde_json::from_slice(bytes.get(20..20 + n).ok_or("truncated JSON")?)
        .map_err(|e| e.to_string())?;
    ensure(
        document["nodes"][0]["name"] == "npc_fredfredburger2",
        "wrong Fred model",
    )?;
    ensure(
        document["meshes"].as_array().is_some_and(|a| a.len() == 1),
        "expected one Fred mesh",
    )?;
    ensure(u32_at(bytes, 24 + n)? == 0x004e4942, "missing BIN chunk")?;
    let bin_start = 28 + n;
    let bin_end = bin_start
        .checked_add(u32_at(bytes, 20 + n)? as usize)
        .ok_or("GLB size overflow")?;
    ensure(bin_end == bytes.len(), "unexpected GLB chunks")?;
    let mut result = bytes.to_vec();
    let mut seen = BTreeSet::new();
    let mut changed = 0;
    for primitive in document["meshes"][0]["primitives"]
        .as_array()
        .ok_or("missing primitives")?
    {
        let index = primitive["attributes"]["WEIGHTS_0"]
            .as_u64()
            .ok_or("weights absent")? as usize;
        let accessor = &document["accessors"][index];
        ensure(
            accessor["componentType"] == 5126
                && accessor["type"] == "VEC4"
                && accessor.get("sparse").is_none(),
            "unsupported Fred weights",
        )?;
        let view = &document["bufferViews"][accessor["bufferView"]
            .as_u64()
            .ok_or("weight buffer view absent")?
            as usize];
        ensure(view["buffer"] == 0, "external weight buffer")?;
        let view_start = bin_start
            .checked_add(view["byteOffset"].as_u64().unwrap_or(0) as usize)
            .ok_or("weight view overflow")?;
        let view_end = view_start
            .checked_add(
                view["byteLength"]
                    .as_u64()
                    .ok_or("weight view length missing")? as usize,
            )
            .ok_or("weight view overflow")?;
        ensure(view_end <= bin_end, "weight view outside BIN")?;
        let start = view_start
            .checked_add(accessor["byteOffset"].as_u64().unwrap_or(0) as usize)
            .ok_or("weight accessor overflow")?;
        let stride = view["byteStride"].as_u64().unwrap_or(16) as usize;
        ensure(stride >= 16, "short weight stride")?;
        for vertex in 0..accessor["count"].as_u64().ok_or("weight count absent")? as usize {
            let offset = vertex
                .checked_mul(stride)
                .and_then(|v| start.checked_add(v))
                .ok_or("weight offset overflow")?;
            ensure(
                offset.checked_add(16).is_some_and(|end| end <= view_end),
                "weight view outside BIN",
            )?;
            if !seen.insert(offset) {
                continue;
            }
            let w = (0..4)
                .map(|i| {
                    u32_at(bytes, offset + i * 4)
                        .map(f32::from_bits)
                        .map(f64::from)
                })
                .collect::<Result<Vec<_>, _>>()?;
            ensure(
                w.iter().all(|v| v.is_finite() && *v >= 0.0)
                    && (w.iter().sum::<f64>() - 1.0).abs() < 1e-5,
                "invalid skin weights",
            )?;
            let total = w[0] + w[1];
            ensure(total > 0.0, "no two-bone weight")?;
            if w[2] != 0.0 || w[3] != 0.0 {
                for (i, value) in [w[0] / total, w[1] / total, 0.0, 0.0]
                    .into_iter()
                    .enumerate()
                {
                    result[offset + i * 4..offset + i * 4 + 4]
                        .copy_from_slice(&(value as f32).to_le_bytes());
                }
                changed += 1;
            }
        }
    }
    Ok((result, changed))
}

fn vehicle_routes(root: &Path) -> Result<(Value, Value), String> {
    let mut table_set = read_json(&root.join("data/tables/table-set.json"))?;
    let table = &table_value(&mut table_set, "m_pVehicleItemTable")?["m_pVehicleItemTable"];
    let models = read_json(&root.join("characters/player/items/catalog.json"))?;
    let textures = read_json(&root.join("data/character_creation/runtime_textures.json"))?;
    let old = read_json(&root.join("data/character_creation/avatar_items.json"))?;
    let models = models["models"].as_array().ok_or("models absent")?;
    let textures = textures["textures"].as_array().ok_or("textures absent")?;
    let old = old["items"].as_array().ok_or("avatar items absent")?;
    let mut routes = Vec::new();
    let mut reserved = Vec::new();
    let mut unresolved = Vec::new();
    for item in table["m_pItemData"]
        .as_array()
        .ok_or("vehicle table absent")?
    {
        let id = item["m_iItemNumber"].as_u64().ok_or("vehicle id absent")?;
        let mesh = item["m_iMesh"].as_u64().ok_or("vehicle mesh id absent")? as usize;
        if id == 0 || mesh == 0 {
            reserved.push(id);
            continue;
        }
        ensure(
            matches!(item["m_iEquipType"].as_u64(), Some(1..=3)),
            "unknown vehicle equip type",
        )?;
        let mesh = &table["m_pItemMeshData"][mesh];
        let prior = old
            .iter()
            .filter(|v| v["category"] == "vehicle" && v["itemNumber"] == id)
            .collect::<Vec<_>>();
        ensure(prior.len() <= 1, "duplicate prior vehicle route")?;
        let (model, texture) = if let Some(prior) = prior.first() {
            let v = &prior["male"];
            ensure(
                v["models"].as_array().is_some_and(|a| a.len() == 1)
                    && v["primaryTexture"]["candidates"]
                        .as_array()
                        .is_some_and(|a| a.len() == 1),
                "ambiguous prior vehicle",
            )?;
            (
                v["models"][0]["nativeAsset"]["path"]
                    .as_str()
                    .ok_or("vehicle path missing")?,
                v["primaryTexture"]["candidates"][0]["path"]
                    .as_str()
                    .ok_or("vehicle texture missing")?,
            )
        } else {
            let name = mesh["m_pstrMMeshModelString"]
                .as_str()
                .ok_or("vehicle model name absent")?;
            let texture = mesh["m_pstrMTextureString"]
                .as_str()
                .ok_or("vehicle texture name absent")?;
            let matching = models
                .iter()
                .filter(|m| {
                    m["category"] == "vehicle"
                        && m["trueName"]
                            .as_str()
                            .is_some_and(|s| s.eq_ignore_ascii_case(name))
                })
                .collect::<Vec<_>>();
            let images = textures
                .iter()
                .filter(|t| {
                    t["trueName"]
                        .as_str()
                        .is_some_and(|s| s.eq_ignore_ascii_case(texture))
                })
                .filter_map(|t| t["nativeAsset"]["path"].as_str())
                .collect::<BTreeSet<_>>();
            if matching.len() != 1 || images.len() != 1 {
                unresolved.push(json!({"itemId":id,"modelName":name,"textureName":texture,"models":matching.len(),"textures":images.len()}));
                continue;
            }
            (
                matching[0]["model"]["path"]
                    .as_str()
                    .ok_or("native vehicle model absent")?,
                *images.iter().next().unwrap(),
            )
        };
        ensure(
            relative(root, model)?.is_file() && relative(root, texture)?.is_file(),
            "missing vehicle native payload",
        )?;
        routes.push(json!({"itemId":id,"model":model,"texture":texture}));
    }
    let report = json!({"vehicles":routes.len(),"reservedWithoutModel":reserved,"unresolvedTableExtensions":unresolved});
    Ok((
        json!({"schema":"ffone.personal-vehicle-catalog.v1","vehicles":routes}),
        report,
    ))
}

fn validate_locales(en: &Value, ru: &Value) -> Result<(), String> {
    let en = en["entries"]
        .as_object()
        .ok_or("English localization entries")?;
    let ru = ru["entries"]
        .as_object()
        .ok_or("Russian localization entries")?;
    ensure(en.keys().eq(ru.keys()), "EN/RU localization keys differ")?;
    fn placeholders(text: &str) -> Vec<&str> {
        let mut values = Vec::new();
        let mut start = None;
        for (index, c) in text.char_indices() {
            if c == '{' {
                start = Some(index);
            } else if c == '}' {
                if let Some(begin) = start.take() {
                    if index > begin + 1 {
                        values.push(&text[begin..=index]);
                    }
                }
            }
        }
        values
    }
    for (key, english) in en {
        let mut a = placeholders(english.as_str().ok_or("English label must be text")?);
        let mut b = placeholders(ru[key].as_str().ok_or("Russian label must be text")?);
        a.sort();
        b.sort();
        ensure(a == b, &format!("placeholder mismatch at {key}"))?;
    }
    Ok(())
}

pub(super) fn run(args: &[String]) -> Result<(), String> {
    let operation = args.first().ok_or("repair-native requires an operation")?;
    let mut flags = BTreeMap::new();
    let mut apply = false;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--apply" {
            ensure(!apply, "duplicate --apply")?;
            apply = true;
            i += 1;
            continue;
        }
        let key = &args[i];
        ensure(
            [
                "--target-root",
                "--work",
                "--server-xdt",
                "--input",
                "--output",
                "--expected-input-sha256",
                "--source-root",
                "--primary-root",
                "--navigation-root",
                "--clips",
                "--semantic-catalog",
                "--recipe",
            ]
            .contains(&key.as_str()),
            "unknown repair option",
        )?;
        let value = args.get(i + 1).ok_or("missing repair option value")?;
        ensure(
            flags.insert(key.as_str(), value.as_str()).is_none(),
            "duplicate repair option",
        )?;
        i += 2;
    }
    let get = |name| {
        flags
            .get(name)
            .copied()
            .ok_or_else(|| format!("{name} required"))
    };
    let target = Path::new(get("--target-root")?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let work_path = super::workspace::resolve_destination(Path::new(get("--work")?))?;
    ensure(
        !work_path.starts_with(&target) && !target.starts_with(&work_path),
        "work overlaps target",
    )?;
    let work = super::workspace::fresh_case_dir(&work_path)?;
    if operation == "player-emote-events" {
        return super::native_emote_events::run(&target, &work, Path::new(get("--source-root")?), apply);
    }
    if operation == "player-damage-animation" {
        return super::native_hnpc_clips::run(
            &target, &work, Path::new(get("--source-root")?), Some("woundupper"),
            Some("characters/player/shared/damage_animations.json"), apply,
        );
    }
    if operation == "deduplicate-textures" {
        return super::native_texture_sharing::run(&target, &work, apply);
    }
    if operation == "nano-voice-events" {
        return super::native_nano_voice::run(&target, &work, Path::new(get("--source-root")?), Path::new(get("--primary-root")?), flags.get("--navigation-root").map(Path::new), apply);
    }
    if operation == "banker" {
        return super::native_banker::run(&target, &work, Path::new(get("--source-root")?), flags.get("--navigation-root").map(Path::new), apply);
    }
    if operation == "hnpc-clips" {
        return super::native_hnpc_clips::run(&target, &work, Path::new(get("--source-root")?), flags.get("--clips").copied(), flags.get("--semantic-catalog").copied(), apply);
    }
    if operation == "audio-paths" {
        return super::native_audio_paths::run(&target, &work, Path::new(get("--recipe")?), apply);
    }
    let mut observed = Vec::new();
    let routes: Vec<String> = match operation.as_str() {
        "area51-vortex" => vec!["map/tiles/map_12_10/behaviour.json".into()],
        "candy-cove" => [
            "map/tiles/map_08_06/behaviour.json",
            "map/tiles/map_08_06/scene.json",
            "map/tiles/map_08_06/objects.json",
            "map/tiles/map_08_06/tile.json",
            "map/catalog.json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        "nano-cel-shading" => ["nano_mordecai", "nano_titan"].into_iter()
            .map(|n| format!("characters/nanos/{n}/{n}.glb")).collect(),
        "darwin-head-normals" => vec!["characters/nanos/nano_darwin/nano_darwin.glb".into()],
        "fred-skin" => vec![get("--output")?.into()],
        "vehicle-routes" => [
            "characters/player/shared/vehicles.json",
            "data/tables/table-set.json",
            "characters/player/items/catalog.json",
            "data/character_creation/runtime_textures.json",
            "data/character_creation/avatar_items.json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        "retire-nano-alias"
        | "unstable-power-icon"
        | "sync-nano-tuning"
        | "nano-identities"
        | "unstable-powers"
        | "nano-tuning" => {
            let server = Path::new(get("--server-xdt")?)
                .canonicalize()
                .map_err(|e| e.to_string())?;
            let bytes = fs::read(&server).map_err(|e| e.to_string())?;
            observed.push((server, Some(digest(&bytes))));
            let mut routes = vec!["assets/game/data/tables/table-set.json".into()];
            if matches!(
                operation.as_str(),
                "nano-identities" | "unstable-powers" | "nano-tuning"
            ) {
                routes.extend([
                    "assets/game/localization/en.json".into(),
                    "assets/game/localization/ru.json".into(),
                ]);
            }
            if operation == "unstable-power-icon" {
                routes.extend([
                    "assets/game/icons/skills/skillicon_67.png".into(),
                    "assets/game/ui/en/gameplay/nano/icons/skill/skillicon_67.png".into(),
                ]);
            }
            routes
        }
        _ => return Err(format!("unknown native repair {operation}")),
    };
    for (route, sha) in snapshot(&target, &routes)? {
        observed.push((relative(&target, &route)?, sha));
    }
    let mut outputs = Vec::new();
    let mut root = target.clone();
    match operation.as_str() {
        "area51-vortex" => {
            let route = "map/tiles/map_12_10/behaviour.json";
            let original = fs::read(relative(&target, route)?).map_err(|e| e.to_string())?;
            let mut document: Value =
                serde_json::from_slice(&original).map_err(|e| e.to_string())?;
            super::native_world_repairs::vortex(&mut document)?;
            outputs.push((
                route.into(),
                super::native_json_patch::rewrite(&original, &document)?,
            ));
        }
        "candy-cove" => outputs.extend(super::native_world_repairs::candy(&target)?),
        "nano-cel-shading" => outputs.extend(super::native_cel_shading::outputs(&target)?),
        "darwin-head-normals" => outputs.extend(super::native_darwin_normals::outputs(&target)?),
        "fred-skin" => {
            let raw = fs::read(get("--input")?).map_err(|e| e.to_string())?;
            ensure(
                digest(&raw) == get("--expected-input-sha256")?,
                "Fred input hash differs",
            )?;
            let (after, changed) = fred_skin(&raw)?;
            ensure(
                fred_skin(&after)?.0 == after,
                "Fred repair must be idempotent",
            )?;
            println!("Changed {changed} vertex weight records; other bytes preserved");
            outputs.push((get("--output")?.into(), after));
        }
        "vehicle-routes" => {
            let (catalog, report) = vehicle_routes(&target)?;
            super::native_publication::write(
                &work.join("routes-publication.json"),
                &encode(&report)?,
            )?;
            let path = "characters/player/shared/vehicles.json";
            let bytes = if relative(&target, path)?.exists() {
                super::native_json_patch::rewrite(
                    &fs::read(relative(&target, path)?).map_err(|e| e.to_string())?,
                    &catalog,
                )?
            } else {
                encode(&catalog)?
            };
            outputs.push((path.into(), bytes));
        }
        "retire-nano-alias"
        | "unstable-power-icon"
        | "sync-nano-tuning"
        | "nano-identities"
        | "unstable-powers"
        | "nano-tuning" => {
            let server_path = Path::new(get("--server-xdt")?)
                .canonicalize()
                .map_err(|e| e.to_string())?;
            let native_path = target.join("assets/game/data/tables/table-set.json");
            let mut native = read_json(&native_path)?;
            let mut server = read_json(&server_path)?;
            if matches!(
                operation.as_str(),
                "nano-identities" | "unstable-powers" | "nano-tuning"
            ) {
                ensure(
                    table_value(&mut native, "m_pNanoTable")?["m_pNanoTable"]
                        == server["m_pNanoTable"],
                    "synchronize native/server Nano tables first",
                )?;
                if operation == "nano-identities" {
                    let value = table_value(&mut native, "m_pNanoTable")?;
                    let quests = value["m_pMissionTable"]["m_pMissionData"]
                        .as_array()
                        .ok_or("mission table missing")?
                        .iter()
                        .filter(|m| m["m_iSTNanoID"] == 41)
                        .map(|m| m["m_iHTaskID"].clone())
                        .collect::<Vec<_>>();
                    ensure(
                        quests == vec![json!(5213), json!(5214), json!(5215)],
                        "Nano 41 quest ownership differs",
                    )?;
                    let models = &table_value(&mut native, "m_pCharacterModelData")?
                        ["m_pCharacterModelData"];
                    for name in [
                        "nano_flapjack",
                        "nano_johnnybravo",
                        "nano_holonano",
                        "nano_coop",
                        "nano_ben",
                        "nano_ghostfreak",
                        "nano_upgrade",
                    ] {
                        let matches = models
                            .as_array()
                            .ok_or("character routes")?
                            .iter()
                            .filter(|m| m["id"] == format!("nano/{name}"))
                            .collect::<Vec<_>>();
                        ensure(matches.len() == 1, "Nano model owner missing or ambiguous")?;
                        ensure(
                            relative(
                                &target.join("assets/game"),
                                matches[0]["glb"].as_str().ok_or("Nano model route")?,
                            )?
                            .is_file(),
                            "required Nano model missing",
                        )?;
                    }
                }
                if operation == "unstable-powers" {
                    for icon in [
                        table_value(&mut native, "m_pSkillTable")?["m_pSkillTable"]["m_pSkillData"]
                            [122]["m_iIcon"]
                            .clone(),
                        server["m_pSkillTable"]["m_pSkillData"][122]["m_iIcon"].clone(),
                    ] {
                        ensure(icon == 1 || icon == 68, "Unstable icon ownership differs")?;
                    }
                }
                let sources = Path::new(get("--source-root")?)
                    .canonicalize()
                    .map_err(|e| e.to_string())?;
                let recipes = crate::repository_root().join("recipes/native/cli");
                let client_paths = [
                    ("data/tables/table-set.json".into(), native_path.clone()),
                    (
                        "localization/en.json".into(),
                        target.join("assets/game/localization/en.json"),
                    ),
                    (
                        "localization/ru.json".into(),
                        target.join("assets/game/localization/ru.json"),
                    ),
                ];
                outputs.extend(super::native_publication::patch_files(
                    &recipes.join(format!("{operation}-client.json")),
                    &sources,
                    &client_paths,
                )?);
                outputs.extend(super::native_publication::patch_files(
                    &recipes.join(format!("{operation}-server.json")),
                    &sources,
                    &[("xdt.json".into(), server_path.clone())],
                )?);
                let bytes_for = |path: &Path| {
                    outputs
                        .iter()
                        .find(|(p, _)| Path::new(p) == path)
                        .map(|(_, b)| b.as_slice())
                        .ok_or("missing staged table")
                };
                let mut staged_native: Value =
                    serde_json::from_slice(bytes_for(&native_path)?).map_err(|e| e.to_string())?;
                let staged_server: Value =
                    serde_json::from_slice(bytes_for(&server_path)?).map_err(|e| e.to_string())?;
                ensure(
                    table_value(&mut staged_native, "m_pNanoTable")?["m_pNanoTable"]
                        == staged_server["m_pNanoTable"],
                    "staged native/server Nano tables differ",
                )?;
                let en: Value = serde_json::from_slice(bytes_for(&client_paths[1].1)?)
                    .map_err(|e| e.to_string())?;
                let ru: Value = serde_json::from_slice(bytes_for(&client_paths[2].1)?)
                    .map_err(|e| e.to_string())?;
                validate_locales(&en, &ru)?;
            } else if operation == "sync-nano-tuning" {
                sync_nano_tuning(&mut native, &mut server)?;
            } else if operation == "retire-nano-alias" {
                let native_table = &mut table_value(&mut native, "m_pNanoTable")?["m_pNanoTable"];
                ensure(
                    *native_table == server["m_pNanoTable"],
                    "synchronize native/server Nano tables first",
                )?;
                retire_nano_alias(native_table)?;
                retire_nano_alias(&mut server["m_pNanoTable"])?;
            } else {
                unstable_icon(table_value(&mut native, "m_pSkillTable")?)?;
                unstable_icon(&mut server)?;
                let png = fs::read(target.join("assets/game/icons/skills/skillicon_67.png"))
                    .map_err(|e| e.to_string())?;
                ensure(
                    digest(&png)
                        == "470d030f1dc4c6e6099db1b3b01b95504565b312c57dc0d8daa3afb5a270793c",
                    "Unstable icon differs",
                )?;
                let dest =
                    target.join("assets/game/ui/en/gameplay/nano/icons/skill/skillicon_67.png");
                ensure(
                    !dest.exists() || fs::read(&dest).map_err(|e| e.to_string())? == png,
                    "existing Unstable icon differs",
                )?;
                outputs.push((dest.to_string_lossy().into_owned(), png));
            }
            if !matches!(
                operation.as_str(),
                "nano-identities" | "unstable-powers" | "nano-tuning"
            ) {
                if operation != "sync-nano-tuning" {
                    outputs.push((
                        native_path.to_string_lossy().into_owned(),
                        super::native_json_patch::rewrite(
                            &fs::read(&native_path).map_err(|e| e.to_string())?,
                            &native,
                        )?,
                    ));
                }
                outputs.push((
                    server_path.to_string_lossy().into_owned(),
                    super::native_json_patch::rewrite(
                        &fs::read(&server_path).map_err(|e| e.to_string())?,
                        &server,
                    )?,
                ));
            }
            while !server_path.starts_with(&root) {
                root = root
                    .parent()
                    .ok_or("client and server have no common filesystem root")?
                    .to_path_buf();
            }
            for (path, _) in &mut outputs {
                *path = Path::new(path)
                    .strip_prefix(&root)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
            }
        }
        _ => return Err(format!("unknown native repair {operation}")),
    }
    let expected = observed
        .into_iter()
        .map(|(path, sha)| {
            path.strip_prefix(&root)
                .map(|p| (p.to_string_lossy().replace('\\', "/"), sha))
                .map_err(|e| e.to_string())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let report = install_checked(&root, &work, &outputs, apply, &expected)?;
    println!("{report}");
    Ok(())
}

#[cfg(test)]
mod tests;
