//! Full-chain native texture sharing. Native payloads only; no legacy identity inference.
use super::native_publication as publication;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

type Files = BTreeMap<String, Vec<u8>>;

fn files(root: &Path, parent: &Path, out: &mut Vec<String>) -> Result<(), String> {
    for entry in fs::read_dir(parent).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let route = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let path = publication::relative(root, &route)?;
        if path.is_dir() {
            files(root, &path, out)?;
        } else if path.is_file() {
            out.push(route);
        }
    }
    Ok(())
}

pub(super) fn decode(bytes: &[u8], glb: bool) -> Result<(Value, Vec<u8>), String> {
    if !glb {
        return Ok((
            serde_json::from_slice(bytes.strip_prefix(&[239, 187, 191]).unwrap_or(bytes))
                .map_err(|e| e.to_string())?,
            vec![],
        ));
    }
    let number = |offset: usize| -> Result<usize, String> {
        Ok(u32::from_le_bytes(
            bytes
                .get(offset..offset + 4)
                .ok_or("truncated GLB")?
                .try_into()
                .unwrap(),
        ) as usize)
    };
    if bytes.get(..4) != Some(b"glTF")
        || number(4)? != 2
        || number(8)? != bytes.len()
        || number(16)? != 0x4e4f534a
    {
        return Err("invalid GLB header".into());
    }
    let end = 20usize.checked_add(number(12)?).ok_or("GLB overflow")?;
    let doc = serde_json::from_slice(bytes.get(20..end).ok_or("truncated GLB JSON")?)
        .map_err(|e| e.to_string())?;
    let mut cursor = end;
    while cursor < bytes.len() {
        let n = number(cursor)?;
        if n % 4 != 0 {
            return Err("unaligned GLB chunk".into());
        }
        cursor = cursor
            .checked_add(8)
            .and_then(|x| x.checked_add(n))
            .ok_or("GLB overflow")?;
        if cursor > bytes.len() {
            return Err("truncated GLB chunk".into());
        }
    }
    Ok((doc, bytes[end..].to_vec()))
}

pub(super) fn encode(
    original: &[u8],
    doc: &Value,
    suffix: &[u8],
    glb: bool,
) -> Result<Vec<u8>, String> {
    if !glb {
        return super::native_json_patch::rewrite(original, doc);
    }
    let (old, _) = decode(original, true)?;
    if old == *doc {
        return Ok(original.to_vec());
    }
    let mut body = serde_json::to_vec(doc).map_err(|e| e.to_string())?;
    body.resize(body.len().next_multiple_of(4), b' ');
    let total = u32::try_from(20 + body.len() + suffix.len()).map_err(|_| "GLB too large")?;
    let mut out = b"glTF".to_vec();
    for n in [2, total, body.len() as u32, 0x4e4f534a] {
        out.extend(n.to_le_bytes());
    }
    out.extend(body);
    out.extend(suffix);
    Ok(out)
}

pub(super) fn visit(value: &Value, action: &mut impl FnMut(&Value)) {
    action(value);
    match value {
        Value::Object(o) => {
            for v in o.values() {
                visit(v, action)
            }
        }
        Value::Array(a) => {
            for v in a {
                visit(v, action)
            }
        }
        _ => (),
    }
}
pub(super) fn mutate(value: &mut Value, action: &mut impl FnMut(&mut Value)) {
    action(value);
    match value {
        Value::Object(o) => {
            for v in o.values_mut() {
                mutate(v, action)
            }
        }
        Value::Array(a) => {
            for v in a {
                mutate(v, action)
            }
        }
        _ => (),
    }
}
pub(super) fn normalized(path: &str) -> Option<String> {
    if path.starts_with('/') || path.contains(['\\', ':']) {
        return None;
    }
    let mut parts = vec![];
    for p in path.split('/') {
        match p {
            "" | "." => (),
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(p),
        }
    }
    Some(parts.join("/"))
}
fn resolve(owner: &str, reference: &str, paths: &BTreeSet<String>) -> Option<String> {
    let parent = owner.rsplit_once('/').map(|x| x.0).unwrap_or("");
    let joined = normalized(&format!("{parent}/{reference}"));
    joined
        .filter(|p| paths.contains(p))
        .or_else(|| normalized(reference).filter(|p| paths.contains(p)))
}
pub(super) fn relative_uri(owner: &str, destination: &str) -> String {
    let mut a: Vec<_> = owner.split('/').collect();
    a.pop();
    let b: Vec<_> = destination.split('/').collect();
    let common = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    std::iter::repeat_n("..", a.len() - common)
        .chain(b[common..].iter().copied())
        .collect::<Vec<_>>()
        .join("/")
}

#[derive(Default)]
struct Plan {
    additions: Files,
    redirects: BTreeMap<String, String>,
    groups: usize,
}

fn plan(root: &Path, routes: &[String], code: &str) -> Result<Plan, String> {
    let paths: BTreeSet<_> = routes.iter().cloned().collect();
    let mut png = BTreeMap::new();
    for p in routes.iter().filter(|p| p.ends_with(".png")) {
        png.insert(
            p.clone(),
            publication::digest(
                &fs::read(publication::relative(root, p)?).map_err(|e| e.to_string())?,
            ),
        );
    }
    let mut chains: BTreeMap<String, BTreeSet<Vec<String>>> = BTreeMap::new();
    let mut interpretations: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for p in routes.iter().filter(|p| p.ends_with(".glb")) {
        let raw = fs::read(publication::relative(root, p)?).map_err(|e| e.to_string())?;
        let (doc, _) = decode(&raw, true)?;
        visit(&doc, &mut |v| {
            let (Some(levels), Some(sampler), Some(_)) = (
                v["mipLevels"].as_array(),
                v.pointer("/sampler/descriptor").and_then(Value::as_object),
                v["uri"].as_str(),
            ) else {
                return;
            };
            if levels.is_empty() || v["colorSpace"].is_null() {
                return;
            }
            let Some(refs): Option<Vec<_>> = levels
                .iter()
                .map(|l| {
                    l["uri"]
                        .as_str()
                        .and_then(|r| resolve(p, r, &paths))
                        .filter(|r| png.contains_key(r))
                })
                .collect()
            else {
                return;
            };
            let mut sampler = sampler.clone();
            sampler.remove("name");
            let interpretation = json!([v["colorSpace"], sampler]).to_string();
            // Include every declared dimension and all bytes in the chain.
            let key = json!([
                interpretation,
                levels
                    .iter()
                    .zip(&refs)
                    .map(|(l, r)| json!([l["width"], l["height"], png[r]]))
                    .collect::<Vec<_>>()
            ])
            .to_string();
            for r in &refs {
                interpretations
                    .entry(r.clone())
                    .or_default()
                    .insert(interpretation.clone());
            }
            chains.entry(key).or_default().insert(refs);
        });
    }
    let mut plan = Plan::default();
    let mut used = paths.clone();
    for members in chains.values() {
        let members: Vec<_> = members
            .iter()
            .filter(|refs| {
                refs.iter().all(|p| {
                    [
                        "characters/npcs/",
                        "characters/mobs/",
                        "characters/fusions/",
                        "characters/nanos/",
                        "map/shared/effects/",
                    ]
                    .iter()
                    .any(|prefix| p.starts_with(prefix))
                        && interpretations[p].len() == 1
                })
            })
            .collect();
        if members.len() < 2
            || members
                .iter()
                .flat_map(|r| r.iter())
                .any(|p| plan.redirects.contains_key(p) || code.contains(p.as_str()))
        {
            continue;
        }
        let stem = Path::new(&members[0][0])
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        // Match the accepted domain relocation policy; character artwork does
        // not become an effect merely because multiple models share it.
        let prefix = if members
            .iter()
            .flat_map(|m| m.iter())
            .all(|p| p.contains("/effects/"))
        {
            "effects/shared/textures"
        } else if members
            .iter()
            .flat_map(|m| m.iter())
            .all(|p| p.starts_with("characters/npcs/"))
        {
            "characters/npcs/shared/textures"
        } else if members
            .iter()
            .flat_map(|m| m.iter())
            .all(|p| p.starts_with("characters/mobs/"))
        {
            "characters/mobs/shared/textures"
        } else {
            "characters/shared/textures"
        };
        let mut base = format!("{prefix}/{stem}.png");
        let mut variant = 1;
        while used.contains(&base)
            || (1..members[0].len()).any(|i| {
                used.contains(&format!(
                    "{}.mips/mip-{i:02}.png",
                    base.trim_end_matches(".png")
                ))
            })
        {
            variant += 1;
            base = format!("{prefix}/{stem}_variant_{variant:02}.png");
        }
        for (i, src) in members[0].iter().enumerate() {
            let dest = if i == 0 {
                base.clone()
            } else {
                format!("{}.mips/mip-{i:02}.png", base.trim_end_matches(".png"))
            };
            let bytes = fs::read(publication::relative(root, src)?).map_err(|e| e.to_string())?;
            for refs in &members {
                if fs::read(publication::relative(root, &refs[i])?).map_err(|e| e.to_string())?
                    != bytes
                {
                    return Err("hash collision or concurrent texture edit".into());
                }
                plan.redirects.insert(refs[i].clone(), dest.clone());
            }
            used.insert(dest.clone());
            plan.additions.insert(dest, bytes);
        }
        plan.groups += 1;
    }
    let mut terrain: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for (p, sha) in &png {
        if p.starts_with("map/tiles/") && p.contains("/terrain/details/density/") {
            terrain
                .entry((p.split("/terrain/").next().unwrap().into(), sha.clone()))
                .or_default()
                .push(p.clone());
        }
    }
    for members in terrain
        .values()
        .filter(|m| m.len() > 1 && !m.iter().any(|p| code.contains(p)))
    {
        let canonical = &members[0];
        let bytes = fs::read(publication::relative(root, canonical)?).map_err(|e| e.to_string())?;
        for p in &members[1..] {
            if fs::read(publication::relative(root, p)?).map_err(|e| e.to_string())? != bytes {
                return Err("density content differs".into());
            }
            plan.redirects.insert(p.clone(), canonical.clone());
        }
    }
    Ok(plan)
}

fn rewrite(root: &Path, routes: &[String], plan: &Plan) -> Result<Files, String> {
    let paths: BTreeSet<_> = routes
        .iter()
        .cloned()
        .chain(plan.additions.keys().cloned())
        .collect();
    let mut changed = Files::new();
    let mut hashes: BTreeMap<String, String> = BTreeMap::new();
    let mut sizes = BTreeMap::new();
    for iteration in 0..20 {
        let mut more = false;
        for p in routes
            .iter()
            .filter(|p| p.ends_with(".glb") || p.ends_with(".json"))
        {
            let original = fs::read(publication::relative(root, p)?).map_err(|e| e.to_string())?;
            let current = changed.get(p).unwrap_or(&original);
            // Most documents cannot contain any affected URI or digest. Avoid
            // repeatedly parsing their complete material/animation metadata.
            // Escaped JSON falls back to the full parser to retain exact semantics.
            let body = if p.ends_with(".glb") && current.len() >= 20 {
                let length = u32::from_le_bytes(current[12..16].try_into().unwrap()) as usize;
                current
                    .get(20..20usize.checked_add(length).ok_or("GLB JSON overflow")?)
                    .ok_or("truncated GLB JSON")?
            } else {
                current.as_slice()
            };
            if let Ok(text) = std::str::from_utf8(body) {
                if !text.contains('\\')
                    && !plan
                        .redirects
                        .keys()
                        .chain(sizes.keys())
                        .any(|path| text.contains(path.rsplit('/').next().unwrap_or(path)))
                    && !hashes.keys().any(|hash| text.contains(hash))
                {
                    continue;
                }
            }
            let (mut doc, suffix) = decode(current, p.ends_with(".glb"))?;
            let mut dirty = false;
            mutate(&mut doc, &mut |v| {
                if let Some(s) = v.as_str() {
                    if let Some(old) = resolve(p, s, &paths) {
                        if let Some(new) = plan.redirects.get(&old) {
                            *v = json!(if s == old {
                                new.clone()
                            } else {
                                relative_uri(p, new)
                            });
                            dirty = true;
                            return;
                        }
                    }
                    let (prefix, h) = s
                        .strip_prefix("blake3:")
                        .map(|h| ("blake3:", h))
                        .unwrap_or(("", s));
                    if let Some(new) = hashes.get(h) {
                        if new != h {
                            *v = json!(format!("{prefix}{new}"));
                            dirty = true;
                        }
                    }
                }
                if let Some(o) = v.as_object_mut() {
                    let size = o
                        .values()
                        .filter_map(Value::as_str)
                        .filter_map(|s| resolve(p, s, &paths))
                        .find_map(|r| sizes.get(&r).copied());
                    if let Some(size) = size {
                        for key in ["bytes", "byteLength", "fileBytes"] {
                            if o.contains_key(key) && o[key] != json!(size) {
                                o.insert(key.into(), json!(size));
                                dirty = true;
                            }
                        }
                    }
                    for key in ["files", "textures"] {
                        if let Some(a) = o.get_mut(key).and_then(Value::as_array_mut) {
                            if a.iter().all(|x| {
                                x.as_object().is_some_and(|x| {
                                    x.contains_key("path")
                                        && x.keys().all(|k| {
                                            ["path", "blake3", "bytes", "sha256", "byteLength"]
                                                .contains(&k.as_str())
                                        })
                                })
                            }) {
                                let mut seen = BTreeSet::new();
                                a.retain(|x| {
                                    let keep = seen.insert(x.to_string());
                                    dirty |= !keep;
                                    keep
                                });
                            }
                        }
                    }
                }
            });
            if dirty {
                let bytes = encode(current, &doc, &suffix, p.ends_with(".glb"))?;
                if bytes != *current {
                    for (old, new) in [
                        (publication::digest(current), publication::digest(&bytes)),
                        (
                            blake3::hash(current).to_hex().to_string(),
                            blake3::hash(&bytes).to_hex().to_string(),
                        ),
                    ] {
                        hashes.insert(old, new);
                    }
                    sizes.insert(p.clone(), bytes.len());
                    changed.insert(p.clone(), bytes);
                    more = true;
                }
            }
        }
        if !more {
            return Ok(changed);
        }
        let keys: Vec<_> = hashes.keys().cloned().collect();
        for key in keys {
            let mut h = hashes[&key].clone();
            let mut seen = BTreeSet::new();
            while let Some(next) = hashes.get(&h) {
                if next == &h {
                    break;
                }
                if !seen.insert(h.clone()) {
                    return Err("cyclic dependent hashes".into());
                }
                h = next.clone();
            }
            hashes.insert(key, h);
        }
        if iteration == 19 {
            return Err("native hash graph did not converge".into());
        }
    }
    unreachable!()
}

pub(super) fn run(root: &Path, work: &Path, apply: bool) -> Result<(), String> {
    let mut routes = vec![];
    files(root, root, &mut routes)?;
    routes.retain(|p| {
        [".png", ".json", ".glb"]
            .iter()
            .any(|suffix| p.ends_with(suffix))
    });
    routes.sort();
    let expected = publication::snapshot(root, &routes)?;
    eprintln!(
        "Texture sharing: checked {} native input files",
        routes.len()
    );
    let mut code = String::new();
    let crate_root = root
        .parent()
        .and_then(Path::parent)
        .ok_or("native assets root")?
        .join("crates");
    if crate_root.exists() {
        let mut source = vec![];
        files(&crate_root, &crate_root, &mut source)?;
        for p in source.iter().filter(|p| p.ends_with(".rs")) {
            code.push_str(
                &fs::read_to_string(publication::relative(&crate_root, p)?)
                    .map_err(|e| e.to_string())?,
            );
            code.push('\n');
        }
    }
    let plan = plan(root, &routes, &code)?;
    eprintln!(
        "Texture sharing: {} compatible chains, {} candidate paths",
        plan.groups,
        plan.redirects.len()
    );
    let changed = if plan.redirects.is_empty() {
        Files::new()
    } else {
        rewrite(root, &routes, &plan)?
    };
    let outputs: Vec<_> = plan
        .additions
        .iter()
        .chain(changed.iter())
        .map(|(p, b)| (p.clone(), b.clone()))
        .collect();
    let mut expected = expected;
    for p in plan.additions.keys() {
        expected.insert(p.clone(), None);
    }
    publication::install_checked(root, work, &outputs, false, &expected)?;
    let deletions: Vec<_> = plan
        .redirects
        .keys()
        .map(|p| json!({"path":p,"sha256":expected[p]}))
        .collect();
    publication::write(
        &work.join("plan.json"),
        &publication::encode(
            &json!({"schema":"fusionforge.native-texture-sharing.v2","chainGroups":plan.groups,"redirects":plan.redirects,"deletions":deletions,"applied":false}),
        )?,
    )?;
    if apply {
        let mut current = vec![];
        files(root, root, &mut current)?;
        current.retain(|p| {
            [".png", ".json", ".glb"]
                .iter()
                .any(|suffix| p.ends_with(suffix))
        });
        current.sort();
        if current != routes {
            return Err("native file inventory changed during sharing".into());
        }
        // Move old immutable payloads into the case first. A failed install
        // restores them; the publication transaction rolls back its own writes.
        if publication::snapshot(root, &expected.keys().cloned().collect::<Vec<_>>())? != expected {
            return Err("native inputs changed before sharing".into());
        }
        let mut moved = vec![];
        let result = (|| {
            for p in plan.redirects.keys() {
                let from = publication::relative(root, p)?;
                let backup = publication::relative(&work.join("removed"), p)?;
                fs::create_dir_all(backup.parent().unwrap()).map_err(|e| e.to_string())?;
                fs::rename(&from, &backup).map_err(|e| e.to_string())?;
                moved.push((from, backup));
                expected.insert(p.clone(), None);
            }
            publication::install_checked(root, work, &outputs, true, &expected)?;
            Ok::<_, String>(())
        })();
        if let Err(e) = result {
            let failures: Vec<_> = moved
                .iter()
                .rev()
                .filter_map(|(to, from)| fs::rename(from, to).err().map(|e| e.to_string()))
                .collect();
            return Err(format!("{e}; texture rollback failures: {failures:?}"));
        }
        let mut report = publication::read_json(&work.join("plan.json"))?;
        report["applied"] = json!(true);
        publication::write(&work.join("plan.json"), &publication::encode(&report)?)?;
    }
    println!(
        "shared chains: {}; replaced PNG paths: {}; changed documents: {}; applied: {apply}",
        plan.groups,
        plan.redirects.len(),
        changed.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests;
