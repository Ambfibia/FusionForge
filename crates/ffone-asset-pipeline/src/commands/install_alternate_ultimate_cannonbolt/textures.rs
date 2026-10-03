use super::*;

pub(super) fn validate_texture_report(spec: &SetSpec, report: &Value) -> Result<(), String> {
    if report["name"] != spec.texture_true_name
        || report.pointer("/source/pathId").and_then(Value::as_i64) != Some(spec.texture_path_id)
        || report["width"] != 256
        || report["height"] != 256
        || report["textureFormatName"] != "DXT1"
        || report["mipCount"] != 9
    {
        return Err(format!(
            "exact alternate Texture2D report changed for {}",
            spec.texture_true_name
        ));
    }
    Ok(())
}

pub(super) fn add_runtime_texture_contract(
    runtime: &mut Value,
    alias: &str,
    spec: &SetSpec,
    report: &Value,
    texture_artifact: &ResourceSetArtifact,
    base_png: &[u8],
) -> Result<(), String> {
    let textures = runtime["textures"]
        .as_array_mut()
        .ok_or_else(|| "runtime texture catalog has no textures".to_owned())?;
    if textures.iter().any(|entry| entry["trueName"] == alias) {
        return Err(format!("runtime texture alias already exists: {alias}"));
    }
    let sampler = &report["sampler"];
    let import = &report["importSettings"];
    textures.push(json!({
        "nativeAsset": texture_artifact,
        "nativePngSha256": sha256_hex(base_png),
        "publishedMipPolicy": "baseLevelOnly",
        "sampler": {
            "anisotropyLevel": sampler.pointer("/aniso/value").and_then(Value::as_u64).unwrap_or(1),
            "legacyFilterMode": sampler.pointer("/filterMode/value").and_then(Value::as_i64).unwrap_or(1),
            "legacyWrapMode": sampler.pointer("/wrapMode/value").and_then(Value::as_i64).unwrap_or(0),
            "magFilter": "linear",
            "minFilter": "linear",
            "mipMapBias": sampler.pointer("/mipBias/value").and_then(Value::as_f64).unwrap_or(0.0),
            "name": alias,
            "wrapS": "repeat",
            "wrapT": "repeat"
        },
        "source": {
            "asset": format!("{}/{}", spec.texture_resource, report.pointer("/source/asset").and_then(Value::as_str).unwrap_or("")),
            "completeImageSize": import["m_CompleteImageSize"],
            "containerRoute": format!("texture/{}.dds", spec.texture_true_name),
            "height": report["height"],
            "imageCount": import["m_ImageCount"],
            "mipMap": import["m_MipMap"],
            "pathId": spec.texture_path_id,
            "sourceChainSha256": report.pointer("/sourcePayload/sha256").and_then(Value::as_str).unwrap_or(""),
            "sourceMipCount": report["mipCount"],
            "textureDimension": import["m_TextureDimension"],
            "textureFormat": report["textureFormat"],
            "textureFormatName": report["textureFormatName"],
            "width": report["width"]
        },
        "trueName": alias,
        "usageColorSpace": "srgb",
        "usageColorSpaceSource": "ActorSkinCombiner runtime assignment to ShaderLab _MainTex; approved alternate Ultimate Cannonbolt donor preserves the same sRGB slot interpretation; exact source mip metadata is retained while the native runtime publishes the decoded base level"
    }));
    Ok(())
}

pub(super) fn update_texture_audit(
    path: &Path,
    runtime: &Value,
    extension_sets: &[Value],
) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let mut audit = read_json_value(path)?;
    let resolved = SETS
        .iter()
        .flat_map(|spec| spec.aliases.iter().copied())
        .collect::<BTreeSet<_>>();
    for key in [
        "missingTrueNames",
        "extensionMissingTrueNames",
        "missingSourceMetadata",
    ] {
        if let Some(values) = audit[key].as_array_mut() {
            values.retain(|value| value.as_str().is_none_or(|name| !resolved.contains(name)));
        }
    }
    if let Some(items) = audit["extensionMissingItems"].as_array_mut() {
        items.retain(|item| {
            item["textureTrueName"]
                .as_str()
                .is_none_or(|name| !resolved.contains(name))
        });
    }
    audit["exactRuntimeContracts"] =
        json!(runtime["textures"].as_array().map(Vec::len).unwrap_or(0));
    audit["alternateDonorExtensions"] = json!({
        "sourceAlias": SOURCE_ALIAS,
        "sourceBuild": SOURCE_BUILD,
        "sets": extension_sets,
        "evidence": "target/ffone-audits/player-equipment-alternate-extension-audit.json"
    });
    write_replace(path, &pretty_json(&audit)?)
}
