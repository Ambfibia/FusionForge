use super::*;

/// Replaces the hard-coded subtitle timeout passed to
/// `cntutorialscript.VoiceOut(string, string, float)` with the duration of the
/// matching tutorial AudioClip. The audio itself is loaded asynchronously by
/// the legacy client, so resolving the duration while building is both safer
/// and deterministic.
pub(crate) fn patch_tutorial_voice_durations(
    root: &Path,
    durations: &BTreeMap<String, f32>,
) -> Result<usize, String> {
    if durations.is_empty() {
        return Ok(0);
    }
    let assembly_path = root.join("Assembly - CSharp.dll");
    if !assembly_path.is_file() {
        return Err(format!(
            "Tutorial script assembly was not found: {}",
            assembly_path.display()
        ));
    }

    let mut context = AssemblyContext::from_path(&assembly_path)?;
    let patched = patch_tutorial_voice_durations_in_context(&mut context, durations)?;
    if patched == 0 {
        return Ok(0);
    }
    write_pe_preserve_layout(&mut context.pe, &assembly_path)?;
    let reparsed = AssemblyContext::from_path(&assembly_path)?;
    let _ = reparsed.metadata.version();
    println!(
        "{}: synchronized {patched} tutorial subtitle timeout(s) with AudioClip duration",
        assembly_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("assembly")
    );
    Ok(patched)
}

pub(super) fn patch_tutorial_voice_durations_in_context(
    context: &mut AssemblyContext,
    durations: &BTreeMap<String, f32>,
) -> Result<usize, String> {
    let voice_out_token = method_def_token(
        context,
        "cntutorialscript::VoiceOut(System.String,System.String,System.Single)",
    )
    .ok_or_else(|| "cntutorialscript::VoiceOut(string,string,float) was not found".to_string())?;
    let normalized = durations
        .iter()
        .filter(|(_, duration)| duration.is_finite() && **duration > 0.0)
        .map(|(name, duration)| (name.to_ascii_lowercase(), *duration))
        .collect::<BTreeMap<_, _>>();
    let mut replacements = Vec::<(u32, f32)>::new();

    for (index, method) in context.metadata.method_defs.iter().enumerate() {
        if method.rva == 0 {
            continue;
        }
        let method_index = index as u32 + 1;
        let type_name = context
            .method_names
            .get(method_index as usize)
            .and_then(|name| name.split_once("::"))
            .map(|(name, _)| name)
            .unwrap_or_default();
        // Iterator methods generated for tutorial event coroutines are nested
        // below cntutorialscript in metadata, while ordinary event methods live
        // directly on the class.
        if !type_name.starts_with("cntutorialscript") {
            continue;
        }
        let body = read_method_body(&context.pe, method.rva)?;
        let instructions = parse_il_instructions(&body.code)?;
        for (call_index, call) in instructions.iter().enumerate() {
            if instruction_call_token(call, &body.code) != Some(voice_out_token) || call_index < 2 {
                continue;
            }
            let timeout = &instructions[call_index - 1];
            if timeout.opcode != IlOpcode::Single(0x22) || timeout.operand_len != 4 {
                continue;
            }
            let search_start = call_index.saturating_sub(12);
            let clip_name = instructions[search_start..call_index - 1]
                .iter()
                .rev()
                .find_map(|instruction| {
                    if instruction.opcode != IlOpcode::Single(0x72) || instruction.operand_len != 4
                    {
                        return None;
                    }
                    let token = read_u32(&body.code, instruction.operand_offset).ok()?;
                    context.metadata.user_strings.get(token & 0x00ff_ffff).ok()
                });
            let Some(clip_name) = clip_name else {
                continue;
            };
            let Some(duration) = normalized.get(&clip_name.to_ascii_lowercase()) else {
                continue;
            };
            replacements.push((body.code_rva + timeout.operand_offset as u32, *duration));
        }
    }

    for (rva, duration) in &replacements {
        context
            .pe
            .write_at_rva(*rva, &duration.to_le_bytes())
            .ok_or_else(|| format!("could not patch tutorial duration at RVA 0x{rva:x}"))?;
    }
    Ok(replacements.len())
}
