use super::*;

pub(super) fn parse_blend_factor(token: &str) -> Result<MaterialBlendFactor, String> {
    match token.to_ascii_lowercase().as_str() {
        "zero" => Ok(MaterialBlendFactor::Zero),
        "one" => Ok(MaterialBlendFactor::One),
        "srccolor" => Ok(MaterialBlendFactor::SourceColor),
        "oneminussrccolor" => Ok(MaterialBlendFactor::OneMinusSourceColor),
        "dstcolor" => Ok(MaterialBlendFactor::DestinationColor),
        "oneminusdstcolor" => Ok(MaterialBlendFactor::OneMinusDestinationColor),
        "srcalpha" => Ok(MaterialBlendFactor::SourceAlpha),
        "oneminussrcalpha" => Ok(MaterialBlendFactor::OneMinusSourceAlpha),
        "dstalpha" => Ok(MaterialBlendFactor::DestinationAlpha),
        "oneminusdstalpha" => Ok(MaterialBlendFactor::OneMinusDestinationAlpha),
        "srcalphasaturate" => Ok(MaterialBlendFactor::SourceAlphaSaturate),
        _ => Err(format!("unsupported ShaderLab blend factor {token:?}")),
    }
}

pub(super) fn parse_blend_operation(token: &str) -> Result<MaterialBlendOperation, String> {
    match token.to_ascii_lowercase().as_str() {
        "add" => Ok(MaterialBlendOperation::Add),
        "sub" => Ok(MaterialBlendOperation::Subtract),
        "revsub" => Ok(MaterialBlendOperation::ReverseSubtract),
        "min" => Ok(MaterialBlendOperation::Minimum),
        "max" => Ok(MaterialBlendOperation::Maximum),
        _ => Err(format!("unsupported ShaderLab BlendOp {token:?}")),
    }
}

pub(crate) fn exact_declared_shader_name(script: &str) -> Result<String, String> {
    let masked = mask_shaderlab(script)?;
    declared_shader_name_from_masked(script, &masked)
}

pub(super) fn declared_shader_name_from_masked(script: &str, masked: &[u8]) -> Result<String, String> {
    let (keyword_start, keyword_end) =
        first_identifier(masked).ok_or_else(|| "ShaderLab script is empty".to_owned())?;
    if !script[keyword_start..keyword_end].eq_ignore_ascii_case("Shader") {
        return Err("ShaderLab script does not begin with a Shader declaration".to_owned());
    }
    let (declared_name, declaration_end) =
        parse_string_after(script, keyword_end, "Shader declaration")?;
    if find_keyword(masked, "Shader", declaration_end, script.len()).is_some() {
        return Err(format!(
            "shader {declared_name:?} script contains a second Shader declaration"
        ));
    }
    Ok(declared_name)
}

pub(super) fn validate_declared_shader_name(
    expected_name: &str,
    script: &str,
    masked: &[u8],
) -> Result<(), String> {
    let declared_name = declared_shader_name_from_masked(script, masked)?;
    if declared_name != expected_name {
        return Err(format!(
            "shader argument {expected_name:?} contradicts declared ShaderLab name {declared_name:?}"
        ));
    }
    Ok(())
}
