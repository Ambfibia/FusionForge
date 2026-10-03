//! Repair flat shading seams on the accepted Darwin head without welding geometry.
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};
fn word(b: &[u8], o: usize) -> Result<usize, String> {
    Ok(u32::from_le_bytes(b.get(o..o + 4).ok_or("truncated GLB")?.try_into().unwrap()) as usize)
}
fn attr(doc: &Value, i: usize, component: u64, kind: &str) -> Result<(usize, usize), String> {
    let a = &doc["accessors"][i];
    let v = &doc["bufferViews"][a["bufferView"].as_u64().ok_or("missing view")? as usize];
    if a["componentType"] != component
        || a["type"] != kind
        || v.get("byteStride").is_some()
        || a.get("sparse").is_some()
    {
        return Err("unexpected Darwin accessor layout".into());
    }
    Ok((
        v["byteOffset"].as_u64().unwrap_or(0) as usize
            + a["byteOffset"].as_u64().unwrap_or(0) as usize,
        a["count"].as_u64().ok_or("count")? as usize,
    ))
}
fn vec3(b: &[u8], o: usize) -> Result<[f64; 3], String> {
    let mut v = [0.; 3];
    for k in 0..3 {
        v[k] = f32::from_le_bytes(
            b.get(o + 4 * k..o + 4 * k + 4)
                .ok_or("truncated vector")?
                .try_into()
                .unwrap(),
        ) as f64;
    }
    Ok(v)
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let len = dot(v, v).sqrt();
    (len.is_finite() && len > 1e-15).then(|| v.map(|x| x / len))
}
fn repair(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.get(..4) != Some(b"glTF") || word(bytes, 4)? != 2 || word(bytes, 8)? != bytes.len() {
        return Err("invalid GLB".into());
    }
    let n = word(bytes, 12)?;
    let doc: Value =
        serde_json::from_slice(bytes.get(20..20 + n).ok_or("JSON")?).map_err(|e| e.to_string())?;
    if doc["nodes"][0]["name"] != "nano_darwin" || word(bytes, 24 + n)? != 0x004e4942 {
        return Err("wrong model/BIN".into());
    }
    let bin = bytes.get(28 + n..).ok_or("BIN")?;
    let mut out = bytes.to_vec();
    let prim = &doc["meshes"][0]["primitives"][0];
    let attrs = &prim["attributes"];
    let index = |name: &str| {
        attrs[name]
            .as_u64()
            .map(|v| v as usize)
            .ok_or("missing attribute".to_owned())
    };
    let (po, count) = attr(&doc, index("POSITION")?, 5126, "VEC3")?;
    let (no, nc) = attr(&doc, index("NORMAL")?, 5126, "VEC3")?;
    let (jo, jc) = attr(&doc, index("JOINTS_0")?, 5123, "VEC4")?;
    let (wo, wc) = attr(&doc, index("WEIGHTS_0")?, 5126, "VEC4")?;
    if count != nc || count != jc || count != wc {
        return Err("attribute count mismatch".into());
    }
    let joints = doc["skins"][0]["joints"].as_array().ok_or("skin")?;
    let head = joints
        .iter()
        .position(|id| doc["nodes"][id.as_u64().unwrap() as usize]["name"] == "Bone_Head")
        .ok_or("head bone")? as u16;
    let mut groups: BTreeMap<Vec<u8>, Vec<usize>> = BTreeMap::new();
    let mut positions = Vec::new();
    for i in 0..count {
        positions.push(vec3(bin, po + 12 * i)?);
        let skin = bin.get(jo + 8 * i..jo + 8 * i + 8).ok_or("joints")?;
        let weights = bin.get(wo + 16 * i..wo + 16 * i + 16).ok_or("weights")?;
        let influence = (0..4)
            .filter(|k| u16::from_le_bytes(skin[2 * k..2 * k + 2].try_into().unwrap()) == head)
            .map(|k| f32::from_le_bytes(weights[4 * k..4 * k + 4].try_into().unwrap()))
            .sum::<f32>();
        if influence < 0.999 {
            continue;
        }
        let mut key = bin[po + 12 * i..po + 12 * i + 12].to_vec();
        key.extend(skin);
        key.extend(weights);
        groups.entry(key).or_default().push(i);
    }
    let ia = prim["indices"].as_u64().ok_or("indices")? as usize;
    let component = doc["accessors"][ia]["componentType"]
        .as_u64()
        .ok_or("index component")?;
    let (io, ic) = attr(&doc, ia, component, "SCALAR")?;
    let size = match component {
        5123 => 2,
        5125 => 4,
        _ => return Err("index format".into()),
    };
    if ic % 3 != 0 {
        return Err("triangles".into());
    }
    let mut areas = vec![[0.; 3]; count];
    for t in 0..ic / 3 {
        let mut ids = [0; 3];
        for k in 0..3 {
            let o = io + (3 * t + k) * size;
            ids[k] = if size == 2 {
                u16::from_le_bytes(bin.get(o..o + 2).ok_or("indices")?.try_into().unwrap()) as usize
            } else {
                word(bin, o)?
            };
            if ids[k] >= count {
                return Err("index bounds".into());
            }
        }
        let a = sub(positions[ids[1]], positions[ids[0]]);
        let b = sub(positions[ids[2]], positions[ids[0]]);
        let area = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        for id in ids {
            for k in 0..3 {
                areas[id][k] += area[k];
            }
        }
    }
    let mut changed = 0;
    for ids in groups.values() {
        for &id in ids {
            let Some(reference) = unit(areas[id]) else {
                continue;
            };
            let mut sum = [0.; 3];
            for &other in ids {
                if unit(areas[other]).is_some_and(|v| dot(reference, v) >= 0.5) {
                    for k in 0..3 {
                        sum[k] += areas[other][k];
                    }
                }
            }
            let Some(normal) = unit(sum) else {
                return Err("degenerate head normal".into());
            };
            for k in 0..3 {
                let offset = 28 + n + no + 12 * id + 4 * k;
                let value = (normal[k] as f32).to_le_bytes();
                if out[offset..offset + 4] != value {
                    changed += 1;
                }
                out[offset..offset + 4].copy_from_slice(&value);
            }
        }
    }
    if groups.len() < 100 {
        return Err("head ownership incomplete".into());
    }
    println!(
        "Darwin head: {} exact position/skin groups; {changed} normal components changed; all other GLB bytes retained",
        groups.len()
    );
    Ok(out)
}
pub(super) fn outputs(root: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let route = "characters/nanos/nano_darwin/nano_darwin.glb";
    let before = fs::read(root.join(route)).map_err(|e| e.to_string())?;
    let after = repair(&before)?;
    if repair(&after)? != after {
        return Err("normal repair is not idempotent".into());
    }
    Ok(vec![(route.into(), after)])
}
