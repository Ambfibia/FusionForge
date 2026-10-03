use super::*;

pub(super) fn parse_direct_state(
    shader_name: &str,
    scope_name: &str,
    script: &str,
    masked: &[u8],
    start: usize,
    end: usize,
    allow_redundant_zwrite: bool,
    allow_compound_particle_fixed_state: bool,
) -> Result<RawState, String> {
    let mut state = RawState::default();
    for statement in direct_lines(script, masked, start, end) {
        let statement = statement
            .split_once("//")
            .map_or(statement, |(before, _)| before)
            .trim();
        if statement.is_empty() {
            continue;
        }
        // Unity's legacy particle program emits four fixed-function commands
        // on one physical line. Keep this admission scoped to the exact,
        // hash-bound Bubbie particle family; the ordinary parser intentionally
        // continues to reject compound render-state syntax.
        if allow_compound_particle_fixed_state
            && matches!(
                statement,
                "Cull Off Lighting Off ZWrite Off Fog { Color (0,0,0,0) }"
                    | "Cull Back Lighting Off ZWrite Off Fog { Color (0,0,0,0) }"
            )
        {
            set_once(
                &mut state.cull,
                if statement.starts_with("Cull Back") {
                    MaterialCullMode::Back
                } else {
                    MaterialCullMode::Off
                },
                "Cull",
                &format!("shader {shader_name:?} {scope_name}"),
            )?;
            set_once(
                &mut state.z_write,
                false,
                "ZWrite",
                &format!("shader {shader_name:?} {scope_name}"),
            )?;
            continue;
        }
        let command_end = statement
            .find(|character: char| character.is_whitespace() || character == '{')
            .unwrap_or(statement.len());
        let command = statement[..command_end].to_ascii_lowercase();
        let tokens = statement.split_whitespace().collect::<Vec<_>>();
        let context = || format!("shader {shader_name:?} {scope_name}");

        match command.as_str() {
            "tags" => {
                if let Some(queue) = parse_queue_tag(statement)? {
                    set_once(&mut state.queue, queue, "Queue tag", &context())?;
                }
            }
            "name" => {
                let name = parse_quoted_value(statement, "Name")?;
                set_once(&mut state.name, name, "pass Name", &context())?;
            }
            "blend" => {
                let blend = if tokens.len() == 2 && tokens[1].eq_ignore_ascii_case("Off") {
                    RawBlend::Disabled
                } else if tokens.len() == 3 && !statement.contains(',') {
                    RawBlend::Enabled {
                        source: parse_blend_factor(tokens[1])?,
                        destination: parse_blend_factor(tokens[2])?,
                    }
                } else {
                    return Err(format!(
                        "{} has unsupported Blend syntax {statement:?}",
                        context()
                    ));
                };
                set_once(&mut state.blend, blend, "Blend", &context())?;
            }
            "blendop" => {
                if tokens.len() != 2 || statement.contains(',') {
                    return Err(format!(
                        "{} has unsupported BlendOp syntax {statement:?}",
                        context()
                    ));
                }
                let operation = parse_blend_operation(tokens[1])?;
                set_once(&mut state.blend_operation, operation, "BlendOp", &context())?;
            }
            "cull" => {
                require_token_count(&tokens, 2, &context(), statement)?;
                let cull = match tokens[1].to_ascii_lowercase().as_str() {
                    "off" => MaterialCullMode::Off,
                    "front" => MaterialCullMode::Front,
                    "back" => MaterialCullMode::Back,
                    _ => return Err(format!("invalid Cull command {statement:?}")),
                };
                set_once(&mut state.cull, cull, "Cull", &context())?;
            }
            "zwrite" => {
                require_token_count(&tokens, 2, &context(), statement)?;
                let enabled = parse_on_off(tokens[1], "ZWrite")?;
                if !(allow_redundant_zwrite && state.z_write == Some(enabled)) {
                    set_once(&mut state.z_write, enabled, "ZWrite", &context())?;
                }
            }
            "ztest" => {
                require_token_count(&tokens, 2, &context(), statement)?;
                let compare = parse_compare(tokens[1])?;
                set_once(&mut state.z_test, compare, "ZTest", &context())?;
            }
            "alphatest" => {
                let alpha_test = if tokens.len() == 2 && tokens[1].eq_ignore_ascii_case("Off") {
                    RawAlphaTest::Disabled
                } else if tokens.len() == 3 {
                    RawAlphaTest::Enabled {
                        compare: parse_compare(tokens[1])?,
                        reference: parse_alpha_reference(tokens[2])?,
                    }
                } else {
                    return Err(format!(
                        "{} has unsupported AlphaTest syntax {statement:?}",
                        context()
                    ));
                };
                set_once(&mut state.alpha_test, alpha_test, "AlphaTest", &context())?;
            }
            "colormask" => {
                require_token_count(&tokens, 2, &context(), statement)?;
                let mask = parse_color_mask(tokens[1])?;
                set_once(&mut state.color_mask, mask, "ColorMask", &context())?;
            }
            _ => {}
        }
    }
    Ok(state)
}

pub(super) fn parse_queue_tag(statement: &str) -> Result<Option<String>, String> {
    let compact = statement
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    if compact == "tags{queue=transparent}" {
        return Ok(Some("transparent".to_owned()));
    }
    let marker = "\"queue\"=\"";
    let Some(start) = compact.find(marker) else {
        return Ok(None);
    };
    if compact[start + marker.len()..].contains(marker) {
        return Err(format!(
            "Tags statement contains more than one Queue entry: {statement:?}"
        ));
    }
    let value_start = start + marker.len();
    let value_end = compact[value_start..]
        .find('"')
        .map(|offset| value_start + offset)
        .ok_or_else(|| format!("unterminated Queue tag in {statement:?}"))?;
    Ok(Some(compact[value_start..value_end].to_owned()))
}

pub(super) fn parse_quoted_value(statement: &str, command: &str) -> Result<String, String> {
    let mut rest = statement[command.len()..].trim();
    if !rest.starts_with('"') {
        return Err(format!(
            "{command} requires one quoted value: {statement:?}"
        ));
    }
    rest = &rest[1..];
    let end = rest
        .find('"')
        .ok_or_else(|| format!("unterminated quoted value in {statement:?}"))?;
    if !rest[end + 1..].trim().is_empty() {
        return Err(format!("unexpected text after {command}: {statement:?}"));
    }
    Ok(rest[..end].to_owned())
}

pub(super) fn parse_compare(token: &str) -> Result<MaterialCompareFunction, String> {
    match token.to_ascii_lowercase().as_str() {
        "off" | "disabled" => Ok(MaterialCompareFunction::Disabled),
        "never" => Ok(MaterialCompareFunction::Never),
        "less" => Ok(MaterialCompareFunction::Less),
        "equal" => Ok(MaterialCompareFunction::Equal),
        "lequal" => Ok(MaterialCompareFunction::LessEqual),
        "greater" => Ok(MaterialCompareFunction::Greater),
        "notequal" => Ok(MaterialCompareFunction::NotEqual),
        "gequal" => Ok(MaterialCompareFunction::GreaterEqual),
        "always" => Ok(MaterialCompareFunction::Always),
        _ => Err(format!("unsupported ShaderLab compare function {token:?}")),
    }
}

pub(super) fn parse_alpha_reference(token: &str) -> Result<RawAlphaReference, String> {
    if let Some(property) = token
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        if property.is_empty() || !property.starts_with('_') {
            return Err(format!("invalid ShaderLab alpha property {token:?}"));
        }
        Ok(RawAlphaReference::FloatProperty(property.to_owned()))
    } else {
        let value = token
            .parse::<f64>()
            .map_err(|_| format!("invalid ShaderLab alpha literal {token:?}"))?;
        if !value.is_finite() {
            return Err(format!("non-finite ShaderLab alpha literal {token:?}"));
        }
        Ok(RawAlphaReference::Literal(value))
    }
}

pub(super) fn parse_color_mask(token: &str) -> Result<u8, String> {
    if token == "0" {
        return Ok(0);
    }
    let mut mask = 0_u8;
    for channel in token.chars() {
        let bit = match channel.to_ascii_uppercase() {
            'R' => 0b0001,
            'G' => 0b0010,
            'B' => 0b0100,
            'A' => 0b1000,
            _ => return Err(format!("invalid ShaderLab ColorMask {token:?}")),
        };
        if mask & bit != 0 {
            return Err(format!(
                "duplicate channel in ShaderLab ColorMask {token:?}"
            ));
        }
        mask |= bit;
    }
    if mask == 0 {
        Err("empty ShaderLab ColorMask".to_owned())
    } else {
        Ok(mask)
    }
}

pub(super) fn parse_on_off(token: &str, field: &str) -> Result<bool, String> {
    match token.to_ascii_lowercase().as_str() {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(format!("{field} expects On or Off, got {token:?}")),
    }
}

pub(super) fn parse_string_after(
    script: &str,
    mut index: usize,
    context: &str,
) -> Result<(String, usize), String> {
    let bytes = script.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if bytes.get(index) != Some(&b'"') {
        return Err(format!("{context} is missing its quoted name"));
    }
    index += 1;
    let mut value = String::new();
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            value.push(byte as char);
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return Ok((value, index + 1));
        } else {
            value.push(byte as char);
        }
        index += 1;
    }
    Err(format!("{context} has an unterminated quoted name"))
}

pub(super) fn find_keyword(masked: &[u8], keyword: &str, start: usize, end: usize) -> Option<usize> {
    let keyword = keyword.as_bytes();
    let mut index = start;
    while index + keyword.len() <= end {
        if (index == start || !is_identifier_continue(masked[index - 1]))
            && masked[index..index + keyword.len()].eq_ignore_ascii_case(keyword)
            && (index + keyword.len() == end
                || !is_identifier_continue(masked[index + keyword.len()]))
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

pub(super) fn find_block(
    script: &str,
    masked: &[u8],
    keyword: &str,
    start: usize,
    end: usize,
) -> Result<Block, String> {
    let keyword_start = find_keyword(masked, keyword, start, end)
        .ok_or_else(|| format!("ShaderLab source has no {keyword} block"))?;
    block_from_keyword(script, masked, keyword, keyword_start, end)
}
