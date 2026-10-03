use super::*;

pub(super) fn validate_retrobution_fusion_eye_report(material_publish: &Value) -> Result<()> {
    const EYE_SHADER: &str = "normal_blendOneOneTest_cullOff";
    const EYE_SHADER_SHA256: &str =
        "cb4b27442bbd396742347760f7610d17d767e2d4fa226ec8cd3ec863cdebe0ed";
    const EYE_PNG_SHA256: &str = "b895006b292f14784a902625761cf75b642c2103d3d845605f09bc47851de214";

    let eyes = material_publish
        .get("materials")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|material| string(material, "name") == Some("spwaneye"))
        .collect::<Vec<_>>();
    if eyes.is_empty() {
        return Ok(());
    }
    let [eye] = eyes.as_slice() else {
        return invalid("Retrobution Fusion publish report duplicates spwaneye material");
    };
    if string(eye, "serializedShaderName") != Some(EYE_SHADER)
        || string(eye, "declaredShaderName") != Some(EYE_SHADER)
        || string(eye, "legacyShaderName") != Some(EYE_SHADER)
        || string(eye, "shaderSha256") != Some(EYE_SHADER_SHA256)
        || eye.get("effectiveRenderQueue").and_then(Value::as_i64) != Some(3000)
        || eye.get("renderPassCount").and_then(Value::as_u64) != Some(1)
    {
        return invalid(
            "Retrobution spwaneye material/shader report is not exact primary evidence",
        );
    }

    let textures = material_publish
        .get("textures")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|texture| string(texture, "sourceName") == Some("spwaneye.dds"))
        .collect::<Vec<_>>();
    let [texture] = textures.as_slice() else {
        return invalid("Retrobution Fusion publish report requires one spwaneye.dds texture");
    };
    if texture.get("width").and_then(Value::as_u64) != Some(128)
        || texture.get("height").and_then(Value::as_u64) != Some(512)
        || texture.get("sourceMipCount").and_then(Value::as_u64) != Some(1)
        || string(texture, "publishedPolicy") != Some("baseLevelOnly")
        || string(texture, "sha256") != Some(EYE_PNG_SHA256)
        || texture.get("byteLength").and_then(Value::as_u64) != Some(17_046)
    {
        return invalid("Retrobution spwaneye report is not the accepted 128x512 primary texture");
    }
    let levels = texture
        .get("mipLevels")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_error("Retrobution spwaneye report has no mip levels"))?;
    let [level] = levels.as_slice() else {
        return invalid("Retrobution spwaneye report must contain one source mip");
    };
    if level.get("level").and_then(Value::as_u64) != Some(0)
        || level.get("width").and_then(Value::as_u64) != Some(128)
        || level.get("height").and_then(Value::as_u64) != Some(512)
        || level.get("pngByteLength").and_then(Value::as_u64) != Some(17_046)
        || string(level, "pngSha256") != Some(EYE_PNG_SHA256)
    {
        return invalid("Retrobution spwaneye mip report is incomplete or contradictory");
    }
    Ok(())
}

pub(super) fn validate_relative(relative: &str) -> Result<()> {
    if relative.is_empty()
        || relative.contains('\\')
        || Path::new(relative).is_absolute()
        || Path::new(relative)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("unsafe native relative path {relative:?}"));
    }
    Ok(())
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::LogicalModelPublish(format!(
        "logical character install failed: {}",
        message.into()
    ))
}
