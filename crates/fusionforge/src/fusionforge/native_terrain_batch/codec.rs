use super::*;

pub(super) fn exact_byte_payload(value: Option<&UnityValue>) -> Option<Vec<u8>> {
    match value? {
        UnityValue::Bytes(bytes) => Some(bytes.clone()),
        UnityValue::Array(values) => values
            .iter()
            .map(|value| value.as_i64().and_then(|value| u8::try_from(value).ok()))
            .collect(),
        _ => None,
    }
}

pub(super) fn encode_gray8_png(width: u32, height: u32, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = GrayImage::from_raw(width, height, bytes.to_vec())
        .ok_or_else(|| format!("could not construct {width}x{height} Gray8 image"))?;
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageLuma8(image)
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|err| format!("could not encode gameplay attributes PNG: {err}"))?;
    Ok(cursor.into_inner())
}

pub(super) fn publication_payload_role(relative: &str) -> &'static str {
    match relative {
        "terrain.json" => "terrainDescriptor",
        "heightmap.png" => "heightmap",
        "scene-instance.json" => "scenePlacement",
        "manifest.json" => "terrainExportManifest",
        "environment/environment.json" => "terrainEnvironment",
        value if value.starts_with("environment/source/") && value.ends_with(".raw.bin") => {
            "terrainEnvironmentSourceRaw"
        }
        value if value.starts_with("environment/source/") && value.ends_with(".parsed.json") => {
            "terrainEnvironmentSourceParsed"
        }
        "gameplay/attributes.png" => "gameplayAttributes",
        "gameplay/attributes.bin" => "gameplayAttributesRaw",
        "gameplay/attributes.raw.json" => "gameplayAttributesProvenance",
        value if value.starts_with("weights/") => "splatWeights",
        value if value.starts_with("layers/") => "terrainLayer",
        value if value.starts_with("components/") && value.ends_with(".bin") => "ownerComponentRaw",
        value if value.starts_with("components/") && value.ends_with(".json") => {
            "ownerComponentParsed"
        }
        _ => "terrainClosure",
    }
}
