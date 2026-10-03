//! Direct, bounded recovery of the three primary corruption projectiles.
use super::unity::{ObjectKey, Pointer, UnityEnvironment, UnityValue};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

fn full(v: &UnityValue) -> Value {
    match v {
        UnityValue::Array(a) => Value::Array(a.iter().map(full).collect()),
        UnityValue::Object(o) => {
            Value::Object(o.iter().map(|(k, v)| (k.clone(), full(v))).collect())
        }
        UnityValue::Pair(a, b) => json!([full(a), full(b)]),
        _ => v.to_json_sample(),
    }
}
fn body(env: &UnityEnvironment, key: ObjectKey) -> Result<Value, String> {
    Ok(full(&env.read_object(key)?))
}
fn pointer(env: &UnityEnvironment, owner: ObjectKey, v: &Value) -> Result<ObjectKey, String> {
    env.resolve_pointer(&Pointer {
        source_asset: owner.asset,
        file_id: v["fileId"].as_i64().ok_or("missing fileId")? as i32,
        path_id: v["pathId"].as_i64().ok_or("missing pathId")?,
    })
}
fn component(
    env: &UnityEnvironment,
    go: ObjectKey,
    kind: &str,
    script: Option<&str>,
) -> Result<(ObjectKey, Value), String> {
    let object = body(env, go)?;
    let mut found = Vec::new();
    for entry in object["m_Component"]
        .as_array()
        .ok_or("missing components")?
    {
        let key = pointer(env, go, &entry[1])?;
        let asset = &env.assets[key.asset];
        if asset.object_type_name(&asset.objects[&key.path_id]) != kind {
            continue;
        }
        let value = body(env, key)?;
        if let Some(expected) = script {
            let script_key = pointer(env, key, &value["m_Script"])?;
            if body(env, script_key)?["m_ClassName"] != expected {
                continue;
            }
        }
        found.push((key, value));
    }
    if found.len() != 1 {
        return Err(format!(
            "missing/ambiguous {kind} {script:?} on {}",
            go.path_id
        ));
    }
    Ok(found.remove(0))
}
fn vector(v: &Value, native: bool) -> Result<Value, String> {
    let x = v["x"].as_f64().ok_or("missing vector x")?;
    let y = v["y"].as_f64().ok_or("missing vector y")?;
    let z = v["z"].as_f64().ok_or("missing vector z")?;
    Ok(json!([if native { -x } else { x }, y, z]))
}
fn curve(v: &Value) -> Result<Value, String> {
    if v["m_PreInfinity"] != 2 || v["m_PostInfinity"] != 2 {
        return Err("unsupported curve infinity".into());
    }
    Ok(Value::Array(
        v["m_Curve"]
            .as_array()
            .ok_or("missing curve")?
            .iter()
            .map(|k| json!([k["time"], k["value"], k["inSlope"], k["outSlope"]]))
            .collect(),
    ))
}
fn property<'a>(v: &'a Value, name: &str) -> Result<&'a Value, String> {
    let found: Vec<_> = v
        .as_array()
        .ok_or("missing material properties")?
        .iter()
        .filter(|row| row[0]["name"] == name)
        .collect();
    if found.len() != 1 {
        return Err(format!("missing/ambiguous material property {name}"));
    }
    Ok(&found[0][1])
}
pub(super) fn convert(
    env: &UnityEnvironment,
    target: &Path,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let asset = env
        .asset_index_by_name("CustomAssetBundle-fa9dbcf4604f64024b06ff1d5e375918")
        .ok_or("primary Effects serialized asset missing")?;
    let mut catalog: Value = serde_json::from_slice(
        &std::fs::read(target.join("effects/npc-skills/catalog.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut projectiles: Value = serde_json::from_slice(
        &std::fs::read(target.join("effects/skill-hits/projectiles.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let bullets = body(
        env,
        ObjectKey {
            asset,
            path_id: 14153,
        },
    )?;
    let mut textures = BTreeMap::<String, Vec<u8>>::new();
    for (style, eid, root) in [(0, 18, 9826), (1, 830, 9324), (2, 831, 11010)] {
        let (_, controller) = component(
            env,
            ObjectKey {
                asset,
                path_id: root,
            },
            "MonoBehaviour",
            Some("EffectEmitterController"),
        )?;
        if controller["conformToScale"] != 0 || controller["nifObject"]["pathId"] != 0 {
            return Err(format!("unsupported corruption effect {eid} mesh/scale"));
        }
        let mut emitters = Vec::new();
        for entry in controller["particles"]
            .as_array()
            .ok_or("missing particles")?
        {
            let go = pointer(
                env,
                ObjectKey {
                    asset,
                    path_id: root,
                },
                &entry["particlePrefab"],
            )?;
            let (_, emitter) =
                component(env, go, "MonoBehaviour", Some("ParticleEmitterController"))?;
            let (_, renderer) = component(env, go, "ParticleRenderer", None)?;
            let (_, animator) = component(env, go, "ParticleAnimator", None)?;
            if animator["damping"].as_f64() != Some(1.) || animator["sizeGrow"].as_f64() != Some(0.)
            {
                return Err("unsupported particle damping/growth".into());
            }
            for key in ["rndForce", "localRotationAxis", "worldRotationAxis"] {
                if vector(&animator[key], false)? != json!([0., 0., 0.]) {
                    return Err(format!("unsupported particle {key}"));
                }
            }
            let materials = renderer["m_Materials"]
                .as_array()
                .ok_or("missing materials")?;
            if materials.len() != 1 {
                return Err("unsupported particle material count".into());
            }
            let material_key = pointer(env, go, &materials[0])?;
            let material = body(env, material_key)?;
            let shader_key = pointer(env, material_key, &material["m_Shader"])?;
            let shader = body(env, shader_key)?;
            let shader = shader["m_Script"].as_str().ok_or("missing shader script")?;
            let blend = if shader.contains("particle_blendOneOne_zwriteOff_cullOff") {
                "OneOne"
            } else if shader.contains("particle_blendSrcalphaOne_zwriteOff_cullOff") {
                "SrcAlphaOne"
            } else {
                return Err("unsupported corruption shader blend".into());
            };
            let tint = property(&material["m_SavedProperties"]["m_Colors"], "_TintColor")?;
            let tex = property(&material["m_SavedProperties"]["m_TexEnvs"], "_MainTex")?;
            if tex["m_Scale"] != json!({"x":1.,"y":1.})
                || tex["m_Offset"] != json!({"x":0.,"y":0.})
                || tex["m_Rotation"].as_f64() != Some(0.)
            {
                return Err("unsupported corruption texture transform".into());
            }
            let texture_key = pointer(env, material_key, &tex["m_Texture"])?;
            let texture = crate::logical_model_material::exact_texture(env, texture_key)?;
            let texture_body = body(env, texture_key)?;
            let texture_name = texture_body["m_Name"].as_str().ok_or("missing texture name")?;
            let texture_name = Path::new(texture_name).file_stem().and_then(|n| n.to_str())
                .ok_or("invalid texture name")?.to_lowercase();
            if !texture_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return Err("unsafe corruption texture name".into());
            }
            let mut paths = Vec::new();
            for mip in texture["mipLevels"]
                .as_array()
                .ok_or("missing texture mips")?
            {
                let level = mip["level"].as_u64().ok_or("missing mip level")?;
                let route = format!(
                    "effects/npc-skills/textures/{texture_name}{}.png",
                    if level == 0 {
                        String::new()
                    } else {
                        format!(".mip{level}")
                    }
                );
                let data = mip
                    .pointer("/payload/dataUrl")
                    .and_then(Value::as_str)
                    .ok_or("missing mip payload")?;
                textures.insert(
                    route.clone(),
                    STANDARD
                        .decode(
                            data.strip_prefix("data:image/png;base64,")
                                .ok_or("expected PNG")?,
                        )
                        .map_err(|e| e.to_string())?,
                );
                paths.push(route);
            }
            if paths.is_empty() {
                return Err("empty corruption texture mip chain".into());
            }
            let colors: Vec<_> = (0..5)
                .map(|n| {
                    let color = animator[format!("colorAnimation[{n}]")]["rgba"]
                        .as_u64()
                        .ok_or("missing particle color")?;
                    Ok(json!([0, 8, 16, 24].map(|shift| ((color >> shift) & 255) as f64 / 255.)))
                })
                .collect::<Result<Vec<_>,String>>()?;
            let keys=entry["scriptKeys"].as_array().ok_or("missing particle keys")?.iter().map(|k|{
                Ok(json!({"time":k["time"],"translation":vector(&k["translate"],true)?,"emit":k["genType"]!=0}))
            }).collect::<Result<Vec<_>,String>>()?;
            let uv = &renderer["UV Animation"];
            emitters.push(json!({"name":emitter["particleName"],"generation":emitter["m_iGenType"],
                "random_position":emitter["randomPosition"],"random_angle":emitter["randomAngle"],"random_velocity":emitter["randomVelocity"],
                "generator_velocity":vector(&emitter["initVelocity"],false)?,"generator_plane":vector(&emitter["plane"],false)?,
                "script_keys":keys,"generations_per_second":emitter["generationsPerSecond"],"number_per_generation":emitter["numberPerGeneration"],
                "lifetime":emitter["lifeTime"],"initial_size":emitter["initialSize"],"force":vector(&animator["force"],true)?,"colors":colors,
                "animate_color":animator["Does Animate Color?"],"uv_tiles":[uv["x Tile"],uv["y Tile"]],"uv_cycles":uv["cycles"],
                "width_curve":curve(&renderer["m_WidthCurve"])?,"height_curve":curve(&renderer["m_HeightCurve"])?,"rotation_curve":curve(&renderer["m_RotationCurve"])?,
                "render_mode":renderer["m_StretchParticles"],"blend_mode":blend,"material_tint":[tint["r"],tint["g"],tint["b"],tint["a"]],"texture":paths}));
        }
        let effects = catalog["effects"]
            .as_array_mut()
            .ok_or("missing native effects")?;
        let plan = json!({"id":eid,"name":format!("corruption-projectile-{style}"),"maximum_timer":controller["maxTimer"],
            "longest_lifetime":controller["longestLifeTime"],"disable_update":controller["disableUpdate"]!=0,"emitters":emitters,"mesh":null});
        if let Some(existing) = effects.iter_mut().find(|r| r["id"] == eid) {
            *existing = plan;
        } else {
            effects.push(plan)
        }
        let b = &bullets["m_pBulletData"][(167 + style) as usize];
        if b["m_iParticleScript"] != eid || b["m_iSuccScript"] != 0 || b["m_iFireScript"] != -1 {
            return Err("corruption bullet identity changed".into());
        }
        let row = json!({"id":167+style,"particle":eid,"impact":0,"scale":b["m_fBulletModelScale"],"impact_scale":b["m_fSuccModelScale"],
            "hide_seconds":b["m_fHideTime"],"duration_seconds":b["m_fMaxTimer"],"source_link":b["m_strFireLink"],"target_link":b["m_strSuccLink"],"impact_sound":null});
        let rows = projectiles["projectiles"]
            .as_array_mut()
            .ok_or("missing native projectiles")?;
        if let Some(existing) = rows.iter().find(|r| r["id"] == 167 + style) {
            if existing != &row {
                return Err("different existing corruption projectile".into());
            }
        } else {
            rows.push(row)
        }
    }
    let mut outputs: Vec<_> = textures.into_iter().collect();
    for (route, value) in [
        ("effects/npc-skills/catalog.json", catalog),
        ("effects/skill-hits/projectiles.json", projectiles),
    ] {
        let mut bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        outputs.push((route.into(), bytes));
    }
    Ok(outputs)
}
