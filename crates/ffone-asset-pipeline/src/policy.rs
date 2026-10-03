use std::path::Path;

use crate::{PipelineError, Result};

const LEGACY_EXTENSIONS: &[&str] = &[
    ".unity3d",
    ".resourcefile",
    ".assets",
    ".ffclient",
    ".assetbundle",
    ".bundle",
    ".ress",
    ".nif",
    ".kfm",
    ".kf",
    ".cg",
    ".obj",
];

const LEGACY_TEXT_TOKENS: &[(&str, &str)] = &[
    ("assetbundle://", "Unity AssetBundle locator"),
    ("archive:/", "archive locator"),
    ("unityfs", "UnityFS signature/token"),
    ("unityraw", "UnityRaw signature/token"),
    ("unityweb", "UnityWeb signature/token"),
    ("gamebryo file format", "Gamebryo signature/token"),
    ("netimmerse file format", "NetImmerse signature/token"),
];

pub(crate) fn reject_legacy_path(path: &str) -> Result<()> {
    let lower = path.to_ascii_lowercase();
    for component in lower.split('/') {
        if component == "assetbundle"
            || component == "assetbundles"
            || component == "globalgamemanagers"
            || component == "maindata"
            || component.starts_with("cab-")
        {
            return Err(PipelineError::ForbiddenLegacy {
                path: path.to_owned(),
                reason: "legacy container component",
            });
        }
        if let Some(extension) = LEGACY_EXTENSIONS
            .iter()
            .find(|extension| component.ends_with(**extension))
        {
            return Err(PipelineError::ForbiddenLegacy {
                path: path.to_owned(),
                reason: legacy_extension_reason(extension),
            });
        }
    }
    Ok(())
}

pub(crate) fn reject_legacy_text(path: &str, bytes: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(bytes).map_err(|_| PipelineError::UnsupportedAsset {
        path: path.to_owned(),
        reason: "text asset is not valid UTF-8".to_owned(),
    })?;
    let lower = text.to_ascii_lowercase();
    if let Some((_, reason)) = LEGACY_TEXT_TOKENS
        .iter()
        .find(|(token, _)| lower.contains(token))
    {
        return Err(PipelineError::ForbiddenLegacy {
            path: path.to_owned(),
            reason,
        });
    }
    Ok(())
}

pub(crate) fn extension(path: &str) -> Option<&str> {
    Path::new(path).extension().and_then(|value| value.to_str())
}

fn legacy_extension_reason(extension: &str) -> &'static str {
    match extension {
        ".unity3d" | ".assetbundle" | ".bundle" => "Unity container extension",
        ".resourcefile" | ".assets" | ".ress" => "Unity serialized-data extension",
        ".ffclient" => "legacy client-project extension",
        ".nif" | ".kfm" | ".kf" => "Gamebryo/NetImmerse extension",
        ".cg" => "legacy Cg shader extension",
        ".obj" => "intermediate OBJ extension",
        _ => "legacy extension",
    }
}
