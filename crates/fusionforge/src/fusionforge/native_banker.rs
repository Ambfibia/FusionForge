//! Source-backed banker model recovery and current HNPC route publication.
use super::{native_nano_voice::recover, native_publication as p, native_texture_sharing as g};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

fn ensure(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
// Compatibility is restricted to these source-pinned accepted banker models.
// The general decoder continues reconstructing correct unit normals.
fn accepted_normals(mesh: &Value, current: &Value) -> Result<Value, String> {
    fn unpack(v: &Value) -> Result<Vec<u64>, String> {
        let bits = v["m_BitSize"].as_u64().ok_or("packed normal bits")? as usize;
        ensure((1..=32).contains(&bits), "packed normal width")?;
        let count = v["m_NumItems"].as_u64().ok_or("packed normal count")? as usize;
        let bytes = STANDARD
            .decode(v["m_Data"]["base64"].as_str().ok_or("packed normal data")?)
            .map_err(|e| e.to_string())?;
        ensure(
            count
                .checked_mul(bits)
                .is_some_and(|n| n <= bytes.len() * 8),
            "truncated packed normals",
        )?;
        Ok((0..count)
            .map(|i| {
                (0..bits).fold(0, |v, b| {
                    v | (((bytes[(i * bits + b) / 8] >> ((i * bits + b) % 8)) & 1) as u64) << b
                })
            })
            .collect())
    }
    let packed = &mesh["m_CompressedMesh"]["m_Normals"];
    let xy = unpack(packed)?;
    let signs = unpack(&mesh["m_CompressedMesh"]["m_NormalSigns"])?;
    ensure(xy.len() == 2 * signs.len(), "packed banker normal counts")?;
    let range = packed["m_Range"].as_f64().ok_or("normal range")?
        / ((1u64 << packed["m_BitSize"].as_u64().unwrap()) - 1) as f64;
    let start = packed["m_Start"].as_f64().ok_or("normal start")?;
    let mut old = vec![];
    let mut corrected = vec![];
    for (i, sign) in signs.iter().enumerate() {
        let x = xy[i * 2] as f64 * range + start;
        let y = xy[i * 2 + 1] as f64 * range + start;
        let squared = 1.0 - x * x - y * y;
        let sign = if *sign == 0 { -1.0 } else { 1.0 };
        old.extend([-x, y, squared * sign]);
        if squared >= 0.0 {
            corrected.extend([-x, y, squared.sqrt() * sign]);
        } else {
            let length = (x * x + y * y).sqrt();
            corrected.extend([-x / length, y / length, 0.0]);
        }
    }
    let actual = current.as_array().ok_or("native normal array")?;
    ensure(
        actual.len() == corrected.len()
            && actual
                .iter()
                .zip(corrected)
                .all(|(a, b)| a.as_f64().is_some_and(|a| (a - b).abs() < 1e-12)),
        "banker normal coordinate contract changed",
    )?;
    Ok(json!(old))
}
fn bindings(doc: &Value) -> Vec<Value> {
    let mut out = vec![];
    g::visit(doc, &mut |v| {
        if v["mipLevels"].as_array().is_some_and(|x| !x.is_empty())
            && v["sampler"].is_object()
            && v["uri"].is_string()
        {
            out.push(v.clone());
        }
    });
    out
}
fn signature(b: &Value) -> Result<Value, String> {
    let mut sampler = b["sampler"]["descriptor"]
        .as_object()
        .ok_or("missing native sampler descriptor")?
        .clone();
    sampler.remove("name");
    Ok(json!([
        b["colorSpace"],
        sampler,
        b["mipLevels"]
            .as_array()
            .ok_or("native mip levels")?
            .iter()
            .map(|m| json!([m["width"], m["height"], m["pngSha256"]]))
            .collect::<Vec<_>>()
    ]))
}

fn shared_model(
    root: &Path,
    candidate: &Path,
    donor_path: &str,
    output: &str,
    observed: &mut BTreeMap<String, Option<String>>,
) -> Result<Vec<u8>, String> {
    let donor_bytes = fs::read(p::relative(root, donor_path)?).map_err(|e| e.to_string())?;
    observed.insert(donor_path.into(), Some(p::digest(&donor_bytes)));
    let donors = bindings(&g::decode(&donor_bytes, true)?.0);
    let raw = fs::read(candidate).map_err(|e| e.to_string())?;
    let (mut model, suffix) = g::decode(&raw, true)?;
    let mut redirects = BTreeMap::new();
    for binding in bindings(&model) {
        let signature = signature(&binding)?;
        let donor = donors
            .iter()
            .find(|d| self::signature(d).ok().as_ref() == Some(&signature))
            .ok_or("banker has no exact shared texture contract")?;
        for (level, shared) in binding["mipLevels"]
            .as_array()
            .unwrap()
            .iter()
            .zip(donor["mipLevels"].as_array().unwrap())
        {
            let uri = level["uri"].as_str().ok_or("candidate texture URI")?;
            let parent = donor_path.rsplit_once('/').ok_or("donor parent")?.0;
            let route = g::normalized(&format!(
                "{parent}/{}",
                shared["uri"].as_str().ok_or("donor texture URI")?
            ))
            .ok_or("donor texture path escape")?;
            ensure(
                route.starts_with("characters/player/rendering/textures/"),
                "unexpected banker shared texture domain",
            )?;
            let bytes = fs::read(p::relative(root, &route)?).map_err(|e| e.to_string())?;
            // Candidate PNGs are local outputs of the exact model exporter.
            ensure(
                !uri.contains(['\\', ':']) && !uri.starts_with('/'),
                "candidate URI invalid",
            )?;
            let texture = candidate
                .parent()
                .unwrap()
                .join(uri)
                .canonicalize()
                .map_err(|e| e.to_string())?;
            ensure(
                texture.starts_with(
                    candidate
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .canonicalize()
                        .map_err(|e| e.to_string())?,
                ),
                "candidate texture escapes staging",
            )?;
            ensure(
                fs::read(texture).map_err(|e| e.to_string())? == bytes,
                "banker complete mip bytes differ",
            )?;
            observed.insert(route.clone(), Some(p::digest(&bytes)));
            redirects.insert(uri.to_string(), g::relative_uri(output, &route));
        }
    }
    ensure(
        !redirects.is_empty(),
        "banker model has no texture bindings",
    )?;
    g::mutate(&mut model, &mut |v| {
        if let Some(new) = v.as_str().and_then(|s| redirects.get(s)) {
            *v = json!(new);
        }
    });
    if let Ok(existing) = fs::read(p::relative(root, output)?) {
        let (old, bin) = g::decode(&existing, true)?;
        if old == model && bin == suffix {
            return Ok(existing);
        }
        return Err(format!(
            "banker model has an unrecognized native preimage: {output}"
        ));
    }
    g::encode(&raw, &model, &suffix, true)
}

pub(super) fn run(
    root: &Path,
    work: &Path,
    source: &Path,
    navigation: Option<&Path>,
    apply: bool,
) -> Result<(), String> {
    let pins = [
        (
            "TableData.resourceFile",
            "bd8767c2d316f60a4b38de320794701b2391d7b40412020bdc1d17e88c7b7212",
        ),
        (
            "CharacterSelection.resourceFile",
            "96a3362fdfff0da0107bd1aea5666431b417380c9855c6f09fceebc8d372ae93",
        ),
        (
            "CharTexture.resourceFile",
            "b22d64844a25dc6010d2b1bfbcf5120d57097d6e5a4602401ba7d85f17545586",
        ),
        (
            "Retro_shared.resourceFile",
            "2ca4147c3705fdd3a0fc0ec44e088da6e87279e978e73b19701b0d29babbfc33",
        ),
    ];
    for (file, sha) in pins {
        ensure(
            p::digest(&fs::read(p::relative(source, file)?).map_err(|e| e.to_string())?) == sha,
            "banker raw source changed",
        )?;
    }
    let evidence = recover(
        source,
        work,
        "primary",
        pins[0].0,
        "CustomAssetBundle-1dca92eecee4742d985b799d8226666d",
        "MonoBehaviour",
        6,
        "hnpc-source",
        pins[0].1,
    )?;
    let rows = evidence["object"]["value"]["TableElement"]
        .as_array()
        .ok_or("HNPC rows")?;
    let catalog_route = "data/hnpc/catalog.json";
    let before = fs::read(p::relative(root, catalog_route)?).map_err(|e| e.to_string())?;
    let mut catalog: Value = serde_json::from_slice(&before).map_err(|e| e.to_string())?;
    let donor = "characters/player/items/shirt/shirt_secretagentcoat/models/f_shirt_secretagentcoat/model.glb";
    let mut routes = vec![catalog_route.to_string(), donor.to_string()];
    for kind in ["shirt", "pants", "shoes"] {
        routes.push(format!(
            "characters/player/hnpc/banker/f_{kind}_secretagent.glb"
        ));
    }
    for name in [
        "shoes_agentsix",
        "female_banker_jacket",
        "female_banker_trousers",
    ] {
        routes.push(format!("characters/hnpc/textures/{name}.png"));
    }
    let mut expected = p::snapshot(root, &routes)?;
    ensure(
        expected[catalog_route].as_deref() == Some(p::digest(&before).as_str()),
        "HNPC catalog changed before conversion",
    )?;
    let mut outputs = vec![];
    let mut parts = BTreeMap::new();
    for (kind, slot, source_key) in [
        ("shirt", 2, "strShirt"),
        ("pants", 1, "strPants"),
        ("shoes", 0, "strShoes"),
    ] {
        let name = format!("f_{kind}_secretagent");
        let source_json = work.join(format!("{name}.source.json"));
        let candidate = work.join(format!("{name}-candidate"));
        super::cli::run_cli(vec![
            "export-logical-model-source".into(),
            source
                .join("CharacterSelection.resourceFile")
                .display()
                .to_string(),
            format!("wear/{name}.nif"),
            source_json.display().to_string(),
            navigation.unwrap_or(source).display().to_string(),
        ])?;
        let mut model_source = p::read_json(&source_json)?;
        for mesh in model_source["meshes"]
            .as_array_mut()
            .ok_or("banker source meshes")?
        {
            let id = mesh["pathId"].as_i64().ok_or("banker mesh id")?;
            let asset = mesh["sourceAsset"]
                .as_str()
                .ok_or("banker mesh asset")?
                .to_string();
            ensure(
                asset == "CustomAssetBundle-ce09c4c9be8a046ca92e0044f22d1b99",
                "banker mesh source differs",
            )?;
            let evidence = recover(
                source,
                work,
                "primary",
                "CharacterSelection.resourceFile",
                &asset,
                "Mesh",
                id,
                &format!("banker-mesh-{id}"),
                pins[1].1,
            )?;
            mesh["normals"] = accepted_normals(&evidence["object"]["value"], &mesh["normals"])?;
        }
        p::write(&source_json, &p::encode(&model_source)?)?;
        ffone_asset_pipeline::publish_logical_model(
            &ffone_asset_pipeline::LogicalModelPublishOptions::new(
                &source_json,
                "player",
                &candidate,
            ),
        )
        .map_err(|e| e.to_string())?;
        let route = format!("characters/player/hnpc/banker/{name}.glb");
        let bytes = shared_model(
            root,
            &candidate.join(format!("models/player/{name}.glb")),
            donor,
            &route,
            &mut expected,
        )?;
        parts.insert(
            kind,
            (
                name.clone(),
                route.clone(),
                bytes.len(),
                blake3::hash(&bytes).to_hex().to_string(),
                slot,
                source_key,
            ),
        );
        outputs.push((route, bytes));
    }
    for (bundle, asset, id, name, sha) in [
        (
            "CharTexture.resourceFile",
            "CustomAssetBundle-aa120043d3c634fe9adfb5cbe08e6970",
            2337,
            "shoes_agentsix",
            pins[2].1,
        ),
        (
            "Retro_shared.resourceFile",
            "CustomAssetBundle-Retro_shared",
            1966,
            "female_banker_jacket",
            pins[3].1,
        ),
        (
            "Retro_shared.resourceFile",
            "CustomAssetBundle-Retro_shared",
            1967,
            "female_banker_trousers",
            pins[3].1,
        ),
    ] {
        // Exact texture export is validated against its scoped source identity.
        let texture_json = work.join(format!("{name}.json"));
        super::cli::run_cli(vec![
            "export-exact-texture".into(),
            source.join(bundle).display().to_string(),
            id.to_string(),
            texture_json.display().to_string(),
        ])?;
        let texture = p::read_json(&texture_json)?;
        ensure(
            texture["id"] == format!("{asset}:{id}")
                && texture["mipCount"] == 1
                && texture["width"] == 256
                && texture["height"] == 256,
            "banker texture identity or mip contract differs",
        )?;
        ensure(
            texture["importSettings"]["m_TextureSettings"]
                == json!({"m_Aniso":1,"m_FilterMode":1,"m_MipBias":0.0,"m_WrapMode":0}),
            "banker sampler differs",
        )?;
        let level = &texture["mipLevels"][0]["payload"];
        let mut bytes = STANDARD
            .decode(
                level["dataUrl"]
                    .as_str()
                    .ok_or("PNG payload")?
                    .split_once(',')
                    .ok_or("PNG data URL")?
                    .1,
            )
            .map_err(|e| e.to_string())?;
        ensure(
            p::digest(&bytes) == level["sha256"].as_str().ok_or("PNG digest")?,
            "PNG digest differs",
        )?;
        let route = format!("characters/hnpc/textures/{name}.png");
        // Preserve an accepted encoding only when decoded dimensions and every
        // RGBA pixel match. A missing file is independently regenerated from raw.
        if let Ok(old) = fs::read(p::relative(root, &route)?) {
            let a = image::load_from_memory(&old)
                .map_err(|e| e.to_string())?
                .into_rgba8();
            let b = image::load_from_memory(&bytes)
                .map_err(|e| e.to_string())?
                .into_rgba8();
            if a == b {
                bytes = old;
            } else {
                return Err(format!(
                    "banker texture has different existing artwork: {route}"
                ));
            }
        }
        let entry = json!({"trueName":name,"path":route,"bytes":bytes.len(),"sha256":p::digest(&bytes),"width":256,"height":256,"sampler":{"name":name,"minFilter":"linear","magFilter":"linear","wrapS":"repeat","wrapT":"repeat","legacyFilterMode":1,"legacyWrapMode":0,"anisotropyLevel":1,"mipMapBias":0.0}});
        let textures = catalog["textures"].as_array_mut().ok_or("HNPC textures")?;
        textures.retain(|t| t["trueName"] != name);
        textures.push(entry);
        textures.sort_by(|a, b| a["trueName"].as_str().cmp(&b["trueName"].as_str()));
        outputs.push((route, bytes));
        ensure(
            p::digest(&fs::read(source.join(bundle)).map_err(|e| e.to_string())?) == sha,
            "texture source changed",
        )?;
    }
    for index in [144usize, 145, 176, 177, 178] {
        let row = &mut catalog["appearances"][index];
        ensure(
            row["gender"] == "female",
            "banker appearance gender differs",
        )?;
        for (kind, (name, route, len, hash, slot, key)) in &parts {
            let src = rows.get(index).ok_or("source appearance absent")?;
            if src[format!("{key}Mesh")] != format!("{name}.nif") {
                ensure(index != 176 && index != 177, "banker source mesh differs")?;
                continue;
            }
            let mut texture = Path::new(
                src[format!("{key}Texture")]
                    .as_str()
                    .ok_or("source texture")?,
            )
            .file_stem()
            .ok_or("texture stem")?
            .to_string_lossy()
            .to_string();
            texture = match texture.as_str() {
                "shirt_secretagentcoat" => "female_banker_jacket".into(),
                "pants_secretagentpants" => "female_banker_trousers".into(),
                _ => texture,
            };
            let parts = row["parts"].as_array_mut().ok_or("appearance parts")?;
            ensure(
                parts.iter().filter(|p| p["kind"] == *kind).count() == 1,
                "banker slot ambiguous",
            )?;
            let part = parts.iter_mut().find(|p| p["kind"] == *kind).unwrap();
            let object = part.as_object_mut().ok_or("appearance part")?;
            for (k,v) in json!({"exactRoute":format!("wear/{name}.nif"),"trueName":name,"sourceRoute":route,"resourceSet":"hnpc-banker","nativeAsset":{"path":route,"bytes":len,"blake3":hash},"textures":[texture],"actorSkinCombinerClothesIndex":slot}).as_object().unwrap(){object.insert(k.clone(),v.clone());}
        }
    }
    outputs.push((
        catalog_route.into(),
        super::native_json_patch::rewrite(&before, &catalog)?,
    ));
    for (file, sha) in pins {
        ensure(
            p::digest(&fs::read(source.join(file)).map_err(|e| e.to_string())?) == sha,
            "banker raw source changed during conversion",
        )?;
    }
    p::install_checked(root, work, &outputs, apply, &expected)?;
    Ok(())
}

#[cfg(test)]
mod tests;
