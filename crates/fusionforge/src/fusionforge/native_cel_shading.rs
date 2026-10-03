//! Owner-requested cel shading for the accepted native Mordecai/Titan additions.
use serde_json::{Value, json};
use std::{fs, path::Path};

fn decode(bytes: &[u8]) -> Result<(Value, &[u8]), String> {
    let word = |offset| {
        bytes
            .get(offset..offset + 4)
            .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
    };
    if bytes.get(..4) != Some(b"glTF")
        || word(4) != Some(2)
        || word(8) != Some(bytes.len() as u32)
        || word(16) != Some(0x4e4f534a)
    {
        return Err("invalid GLB 2".into());
    }
    let end = 20usize
        .checked_add(word(12).ok_or("missing JSON length")? as usize)
        .ok_or("overflow")?;
    let document = serde_json::from_slice(bytes.get(20..end).ok_or("truncated GLB")?)
        .map_err(|e| e.to_string())?;
    Ok((document, bytes.get(end..).ok_or("truncated chunks")?))
}
fn encode(document: &Value, tail: &[u8]) -> Result<Vec<u8>, String> {
    let mut json = serde_json::to_vec(document).map_err(|e| e.to_string())?;
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let mut out = b"glTF".to_vec();
    out.extend(2u32.to_le_bytes());
    out.extend(((20 + json.len() + tail.len()) as u32).to_le_bytes());
    out.extend((json.len() as u32).to_le_bytes());
    out.extend(0x4e4f534au32.to_le_bytes());
    out.extend(json);
    out.extend(tail);
    Ok(out)
}
fn transform(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let (mut doc, tail) = decode(bytes)?;
    for material in doc["materials"].as_array_mut().ok_or("missing materials")? {
        let native = &mut material["extras"]["ffone"];
        if !matches!(
            native["legacyShaderName"].as_str(),
            Some(
                "normal"
                    | "normal_blendSrcalphaInvsrcalpha"
                    | "normal_blendSrcalphaInvsrcalpha_zwriteOff"
            )
        ) {
            return Err("cel extension requires a validated native normal surface".into());
        }
        native["nativeSurfaceStyle"] = json!("cel");
    }
    let result = encode(&doc, tail)?;
    if decode(&result)?.1 != tail {
        return Err("geometry/animation BIN changed".into());
    }
    Ok(result)
}
pub(super) fn outputs(root: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    ["nano_mordecai", "nano_titan"]
        .into_iter()
        .map(|name| {
            let route = format!("characters/nanos/{name}/{name}.glb");
            let bytes = fs::read(root.join(&route)).map_err(|e| e.to_string())?;
            let after = transform(&bytes)?;
            if transform(&after)? != after {
                return Err("cel repair is not idempotent".into());
            }
            Ok((route, after))
        })
        .collect()
}
