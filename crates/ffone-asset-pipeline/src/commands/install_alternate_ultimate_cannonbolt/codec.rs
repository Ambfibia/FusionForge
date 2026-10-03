use super::*;

pub(super) fn decode_exact_mips(report: &Value) -> Result<Vec<Vec<u8>>, String> {
    let levels = report["mipLevels"]
        .as_array()
        .ok_or_else(|| "exact texture report has no mipLevels".to_owned())?;
    let mut decoded = Vec::with_capacity(levels.len());
    for (index, level) in levels.iter().enumerate() {
        if level["level"].as_u64() != Some(index as u64) {
            return Err(format!("non-contiguous exact mip level {index}"));
        }
        let payload = &level["payload"];
        let encoded = payload["dataUrl"]
            .as_str()
            .and_then(|value| value.strip_prefix("data:image/png;base64,"))
            .ok_or_else(|| format!("mip level {index} has no PNG data URL"))?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|error| format!("cannot decode mip level {index}: {error}"))?;
        if payload["byteLength"].as_u64() != Some(bytes.len() as u64)
            || payload["sha256"].as_str() != Some(sha256_hex(&bytes).as_str())
        {
            return Err(format!("mip level {index} acceptance identity changed"));
        }
        decoded.push(bytes);
    }
    Ok(decoded)
}
