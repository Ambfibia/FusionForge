//! Native collision geometry from scoped Unity evidence. No source identities enter the GLB.
use super::native_publication::digest;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

fn packed(value: &Value) -> Result<Vec<f64>, String> {
    let count = value["m_NumItems"].as_u64().ok_or("packed count")? as usize;
    let bits = value["m_BitSize"].as_u64().ok_or("packed bits")? as usize;
    if count == 0 || !(1..=32).contains(&bits) {
        return Err("invalid packed stream".into());
    }
    let data = STANDARD
        .decode(value["m_Data"]["base64"].as_str().ok_or("packed bytes")?)
        .map_err(|e| e.to_string())?;
    if count.checked_mul(bits).ok_or("packed overflow")? > data.len() * 8 {
        return Err("truncated packed stream".into());
    }
    let start = value["m_Start"].as_f64().ok_or("packed start")?;
    let scale = value["m_Range"].as_f64().ok_or("packed range")? / ((1u64 << bits) - 1) as f64;
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        let mut n = 0u32;
        for bit in 0..bits {
            let offset = index * bits + bit;
            n |= u32::from((data[offset / 8] >> (offset % 8)) & 1) << bit;
        }
        values.push(f64::from(n) * scale + start);
    }
    Ok(values)
}

pub(super) fn convert(
    mesh: &Value,
    collider: &Value,
    name: &str,
    reverse: bool,
) -> Result<(Vec<u8>, Value), String> {
    for (e, kind) in [(mesh, "Mesh"), (collider, "MeshCollider")] {
        if e["schema"] != "fusionforge.object-evidence.v1"
            || e["triage"]["unresolvedPointerCount"] != 0
            || e["object"]["type"] != kind
        {
            return Err(format!("invalid scoped {kind} evidence"));
        }
    }
    if mesh["source"] != collider["source"]
        || mesh["object"]["serializedAsset"] != collider["object"]["serializedAsset"]
    {
        return Err("collider and mesh must share raw and serialized asset ownership".into());
    }
    let col = &collider["object"]["value"];
    if col["m_IsTrigger"] != false
        || col["m_Convex"] != false
        || col["m_Mesh"]["fileId"] != 0
        || col["m_Mesh"]["pathId"] != mesh["object"]["pathId"]
    {
        return Err("only exact local non-trigger, non-convex colliders are supported".into());
    }
    let obj = &mesh["object"]["value"];
    let count = obj["m_CollisionVertexCount"]
        .as_u64()
        .ok_or("collision count")? as usize;
    let values = packed(&obj["m_CompressedMesh"]["m_Vertices"])?;
    if count == 0 || values.len() % 3 != 0 || count > values.len() / 3 {
        return Err("invalid collision vertex count".into());
    }
    let positions = values
        .chunks_exact(3)
        .take(count)
        .map(|p| [(-p[0]) as f32, p[1] as f32, p[2] as f32])
        .collect::<Vec<_>>();
    if positions.iter().flatten().any(|v| !v.is_finite()) {
        return Err("non-finite collision position".into());
    }
    let mut indices = obj["m_CollisionTriangles"]
        .as_array()
        .ok_or("collision triangles")?
        .iter()
        .map(|i| {
            i.as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .ok_or("invalid index".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if indices.is_empty() || indices.len() % 3 != 0 || indices.iter().any(|i| *i as usize >= count)
    {
        return Err("invalid collision triangles".into());
    }
    if reverse {
        for triangle in indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    let mut binary = Vec::new();
    for p in &positions {
        for v in p {
            binary.extend_from_slice(&v.to_le_bytes());
        }
    }
    let mut geometry = binary.clone();
    for i in &indices {
        geometry.extend_from_slice(&i.to_le_bytes());
    }
    let max = *indices.iter().max().unwrap();
    let min = *indices.iter().min().unwrap();
    let short = max <= u16::MAX.into();
    for i in &indices {
        if short {
            binary.extend_from_slice(&(*i as u16).to_le_bytes());
        } else {
            binary.extend_from_slice(&i.to_le_bytes());
        }
    }
    binary.resize(binary.len().next_multiple_of(4), 0);
    let lo = (0..3)
        .map(|axis| {
            f64::from(
                positions
                    .iter()
                    .map(|p| p[axis])
                    .fold(f32::INFINITY, f32::min),
            )
        })
        .collect::<Vec<_>>();
    let hi = (0..3)
        .map(|axis| {
            f64::from(
                positions
                    .iter()
                    .map(|p| p[axis])
                    .fold(f32::NEG_INFINITY, f32::max),
            )
        })
        .collect::<Vec<_>>();
    let name = serde_json::to_string(name).map_err(|e| e.to_string())?;
    // Stable published field order is part of the existing accepted GLB bytes.
    let text = format!(
        r#"{{"asset":{{"version":"2.0","generator":"FusionForge character collision publisher","extras":{{"coordinateContract":"ffone.native-coordinate-contract.v1","purpose":"exact non-rendering character collision"}}}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"name":{name},"mesh":0}}],"meshes":[{{"name":{name},"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"mode":4}}]}}],"buffers":[{{"byteLength":{length}}}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{pos_len},"target":34962}},{{"buffer":0,"byteOffset":{pos_len},"byteLength":{idx_len},"target":34963}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":{count},"type":"VEC3","min":{lo},"max":{hi}}},{{"bufferView":1,"componentType":{component},"count":{idx_count},"type":"SCALAR","min":[{min}],"max":[{max}]}}]}}"#,
        length = binary.len(),
        pos_len = count * 12,
        idx_len = indices.len() * if short { 2 } else { 4 },
        component = if short { 5123 } else { 5125 },
        idx_count = indices.len(),
        lo = serde_json::to_string(&lo).unwrap(),
        hi = serde_json::to_string(&hi).unwrap()
    );
    let mut text = text.into_bytes();
    text.resize(text.len().next_multiple_of(4), b' ');
    let length = u32::try_from(28 + text.len() + binary.len()).map_err(|_| "GLB too large")?;
    let mut glb = b"glTF".to_vec();
    for v in [2, length, text.len() as u32, 0x4e4f534a] {
        glb.extend_from_slice(&v.to_le_bytes());
    }
    glb.extend(text);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004e4942u32.to_le_bytes());
    glb.extend(binary);
    let report = json!({"vertexCount":count,"indexCount":indices.len(),"geometrySha256":digest(&geometry),"sha256":digest(&glb),"reverseWinding":reverse});
    Ok((glb, report))
}

#[cfg(test)]
mod tests;
