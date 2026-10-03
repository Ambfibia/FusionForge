use super::*;

/// Parse the unique matching texture property from the exact ShaderLab
/// `Properties` block. The accepted set is intentionally closed: these are the
/// only defaults whose Unity semantics are represented by the native runtime.
pub(crate) fn exact_shader_texture_defaults(
    script: &str,
) -> Result<Vec<ShaderLabTextureDefaultProperty>, String> {
    let masked = mask_shaderlab(script)?;
    let (properties_open, properties_close) = exact_properties_block(script, &masked)?;
    let mut defaults = Vec::new();
    for line in direct_lines(script, &masked, properties_open + 1, properties_close) {
        let masked_line = mask_shaderlab(line)?;
        let Some((name_start, name_end)) = first_identifier(&masked_line) else {
            continue;
        };
        let Some(open) = masked_line[name_end..]
            .iter()
            .position(|byte| *byte == b'(')
            .map(|offset| name_end + offset)
        else {
            continue;
        };
        let mut depth = 0_usize;
        let mut close = None;
        let mut type_separator = None;
        for (index, byte) in masked_line.iter().enumerate().skip(open + 1) {
            match byte {
                b'(' => depth += 1,
                b')' if depth == 0 => {
                    close = Some(index);
                    break;
                }
                b')' => depth -= 1,
                b',' if depth == 0 => type_separator = Some(index),
                _ => {}
            }
        }
        let Some(close) = close else {
            return Err(format!(
                "ShaderLab property {:?} has an unterminated declaration",
                &line[name_start..name_end]
            ));
        };
        let Some(type_separator) = type_separator else {
            continue;
        };
        if line[type_separator + 1..close].trim() != "2D" {
            continue;
        }
        let name = line[name_start..name_end].to_owned();
        let value = exact_shader_property_default(script, &masked, &name)?;
        let value = match value.as_str() {
            "\"white\" {}" => ShaderLabTextureDefault::BuiltinWhite,
            "\"black\" {}" => ShaderLabTextureDefault::BuiltinBlack,
            "\"gray\" {}" => ShaderLabTextureDefault::BuiltinGray,
            "\"bump\" {}" => ShaderLabTextureDefault::BuiltinBump,
            "\"red\" {}" => ShaderLabTextureDefault::BuiltinRed,
            "\"\" {}" => ShaderLabTextureDefault::Blank,
            _ => {
                return Err(format!(
                    "unsupported or non-exact ShaderLab texture default for {name:?}: {value:?}"
                ));
            }
        };
        if defaults
            .iter()
            .any(|property: &ShaderLabTextureDefaultProperty| property.slot == name)
        {
            return Err(format!(
                "ShaderLab texture property default {name:?} occurs more than once"
            ));
        }
        defaults.push(ShaderLabTextureDefaultProperty { slot: name, value });
    }
    Ok(defaults)
}
