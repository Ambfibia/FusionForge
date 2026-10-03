use super::*;

pub(super) fn direct_child_blocks(
    script: &str,
    masked: &[u8],
    keyword: &str,
    start: usize,
    end: usize,
) -> Result<Vec<Block>, String> {
    let mut blocks = Vec::new();
    let mut depth = 0_usize;
    let mut index = start;
    while index < end {
        match masked[index] {
            b'{' => {
                depth += 1;
                index += 1;
            }
            b'}' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| "unbalanced ShaderLab braces".to_owned())?;
                index += 1;
            }
            byte if depth == 0 && is_identifier_start(byte) => {
                let word_start = index;
                index += 1;
                while index < end && is_identifier_continue(masked[index]) {
                    index += 1;
                }
                if script[word_start..index].eq_ignore_ascii_case(keyword) {
                    let block = block_from_keyword(script, masked, keyword, word_start, end)?;
                    index = block.close + 1;
                    blocks.push(block);
                }
            }
            _ => index += 1,
        }
    }
    if depth != 0 {
        return Err("unbalanced ShaderLab child-block braces".to_owned());
    }
    Ok(blocks)
}

pub(super) fn direct_lines<'a>(script: &'a str, masked: &[u8], start: usize, end: usize) -> Vec<&'a str> {
    let mut lines = Vec::new();
    let mut depth = 0_usize;
    let mut line_start = start;
    while line_start < end {
        let line_end = masked[line_start..end]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(end, |offset| line_start + offset);
        if depth == 0 {
            lines.push(&script[line_start..line_end]);
        }
        for byte in &masked[line_start..line_end] {
            match byte {
                b'{' => depth += 1,
                b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        line_start = (line_end + 1).min(end);
    }
    lines
}

pub(super) fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

pub(super) fn is_identifier_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
