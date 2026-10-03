use super::*;

pub(crate) fn managed_assembly_name(path: &Path) -> Result<Option<String>, String> {
    let context = AssemblyContext::from_path(path)?;
    Ok(context.metadata.assembly().map(|assembly| assembly.name))
}

pub(super) fn binary_reader_unicode_tokens(
    context: &AssemblyContext,
) -> Result<Option<BinaryReaderUnicodeTokens>, String> {
    let read_bytes = member_ref_tokens_by_signature(
        context,
        "System.IO.BinaryReader",
        "ReadBytes",
        "System.Byte[]",
        &["System.Int32"],
    );
    let get_default = member_ref_tokens_by_signature(
        context,
        "System.Text.Encoding",
        "get_Default",
        "System.Text.Encoding",
        &[],
    );
    let get_string = member_ref_tokens_by_signature(
        context,
        "System.Text.Encoding",
        "GetString",
        "System.String",
        &["System.Byte[]"],
    );
    if read_bytes.is_empty() || get_default.is_empty() || get_string.is_empty() {
        return Ok(None);
    }
    let get_unicode = member_ref_tokens_by_signature(
        context,
        "System.Text.Encoding",
        "get_Unicode",
        "System.Text.Encoding",
        &[],
    );
    if get_unicode.is_empty() {
        return Err(
            "Encoding.get_Unicode MemberRef was not found; cannot patch BinaryReader strings"
                .to_string(),
        );
    }

    Ok(Some(BinaryReaderUnicodeTokens {
        read_bytes,
        get_default,
        get_unicode,
        get_string,
    }))
}

pub(super) fn member_ref_tokens_by_signature(
    context: &AssemblyContext,
    owner_full_name: &str,
    member_name: &str,
    return_type: &str,
    params: &[&str],
) -> BTreeSet<u32> {
    context
        .metadata
        .member_refs
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            if context.metadata.strings.get(row.name).ok()? != member_name {
                return None;
            }
            if member_ref_owner_full_name(context, &row.class).as_deref()? != owner_full_name {
                return None;
            }
            let signature = context
                .metadata
                .blobs
                .get(row.signature)
                .ok()
                .and_then(|blob| MethodSig::parse_blob(blob).ok())?;
            if format_type_sig(&context.metadata, &signature.return_type) != return_type {
                return None;
            }
            let signature_params = signature
                .params
                .iter()
                .map(|param| format_type_sig(&context.metadata, param))
                .collect::<Vec<_>>();
            if signature_params.len() != params.len()
                || !signature_params
                    .iter()
                    .map(String::as_str)
                    .zip(params.iter().copied())
                    .all(|(left, right)| left == right)
            {
                return None;
            }
            Some(0x0a00_0000u32 | (index as u32 + 1))
        })
        .collect()
}

pub(super) fn member_ref_owner_full_name(
    context: &AssemblyContext,
    class: &clrmeta::CodedIndex,
) -> Option<String> {
    match class.table? {
        TableId::TypeRef => type_ref_full_name(&context.metadata, class.row),
        TableId::TypeDef => context
            .type_names
            .get(class.row as usize)
            .filter(|value| !value.is_empty())
            .cloned(),
        _ => None,
    }
}

pub(super) fn type_ref_full_name(metadata: &Metadata, index: u32) -> Option<String> {
    let row = metadata.get_type_ref(index)?;
    let name = metadata.strings.get(row.type_name).ok()?;
    let namespace = if row.type_namespace == 0 {
        ""
    } else {
        metadata.strings.get(row.type_namespace).ok()?
    };
    if namespace.is_empty() {
        Some(name.to_string())
    } else {
        Some(format!("{namespace}.{name}"))
    }
}

pub(super) fn binary_reader_readbytes_has_length_operand(
    instructions: &[IlInstruction],
    code: &[u8],
    call_index: usize,
) -> Option<bool> {
    if call_index >= 3
        && instructions[call_index - 2].opcode == IlOpcode::Single(0x18)
        && instructions[call_index - 1].opcode == IlOpcode::Single(0x5a)
        && load_local_index(&instructions[call_index - 3], code).is_some()
    {
        return Some(true);
    }
    load_local_index(instructions.get(call_index.checked_sub(1)?)?, code).map(|_| false)
}

pub(super) fn rewrite_il_code(
    code: &[u8],
    instructions: &[IlInstruction],
    insertions: &BTreeMap<usize, Vec<u8>>,
    replacements: &BTreeMap<usize, u32>,
) -> Result<Vec<u8>, String> {
    let insertion_offsets = insertions
        .iter()
        .map(|(offset, bytes)| (*offset, bytes.len()))
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(
        code.len() + insertion_offsets.iter().map(|(_, len)| *len).sum::<usize>(),
    );

    for instruction in instructions {
        if let Some(bytes) = insertions.get(&instruction.offset) {
            output.extend_from_slice(bytes);
        }
        let expected_offset =
            map_il_offset_after_insertions(instruction.offset, &insertion_offsets);
        if output.len() != expected_offset {
            return Err(format!(
                "IL rewrite offset mismatch at 0x{:x}: expected 0x{expected_offset:x}, got 0x{:x}",
                instruction.offset,
                output.len()
            ));
        }

        let mut bytes = code
            .get(instruction.offset..instruction.offset + instruction.size)
            .ok_or_else(|| {
                format!(
                    "IL instruction at 0x{:x} size {} is out of range",
                    instruction.offset, instruction.size
                )
            })?
            .to_vec();
        if let Some(token) = replacements.get(&instruction.operand_offset) {
            let relative_offset = instruction.operand_offset - instruction.offset;
            bytes
                .get_mut(relative_offset..relative_offset + 4)
                .ok_or_else(|| {
                    format!(
                        "IL token operand at 0x{:x} is out of range",
                        instruction.operand_offset
                    )
                })?
                .copy_from_slice(&token.to_le_bytes());
        }
        adjust_branch_operand(&mut bytes, instruction, code.len(), &insertion_offsets)?;
        output.extend_from_slice(&bytes);
    }

    Ok(output)
}

pub(super) fn adjust_branch_operand(
    bytes: &mut [u8],
    instruction: &IlInstruction,
    code_len: usize,
    insertions: &[(usize, usize)],
) -> Result<(), String> {
    match instruction.opcode {
        IlOpcode::Single(opcode) if is_short_branch_opcode(opcode) => {
            let relative_offset = instruction.operand_offset - instruction.offset;
            let old_delta = bytes.get(relative_offset).copied().ok_or_else(|| {
                format!(
                    "short branch operand at 0x{:x} is out of range",
                    instruction.operand_offset
                )
            })? as i8 as isize;
            let old_target =
                checked_branch_target(instruction.offset, instruction.size, old_delta, code_len)?;
            let new_instruction =
                map_il_offset_after_insertions(instruction.offset, insertions) as isize;
            let new_target = map_il_offset_after_insertions(old_target, insertions) as isize;
            let new_delta = new_target - (new_instruction + instruction.size as isize);
            if !(i8::MIN as isize..=i8::MAX as isize).contains(&new_delta) {
                return Err(format!(
                    "short branch at IL offset 0x{:x} is out of range after patch",
                    instruction.offset
                ));
            }
            bytes[relative_offset] = (new_delta as i8) as u8;
        }
        IlOpcode::Single(opcode) if is_long_branch_opcode(opcode) => {
            let relative_offset = instruction.operand_offset - instruction.offset;
            let old_delta = read_i32(bytes, relative_offset)? as isize;
            let old_target =
                checked_branch_target(instruction.offset, instruction.size, old_delta, code_len)?;
            let new_instruction =
                map_il_offset_after_insertions(instruction.offset, insertions) as isize;
            let new_target = map_il_offset_after_insertions(old_target, insertions) as isize;
            let new_delta = new_target - (new_instruction + instruction.size as isize);
            if !(i32::MIN as isize..=i32::MAX as isize).contains(&new_delta) {
                return Err(format!(
                    "long branch at IL offset 0x{:x} is out of range after patch",
                    instruction.offset
                ));
            }
            bytes[relative_offset..relative_offset + 4]
                .copy_from_slice(&(new_delta as i32).to_le_bytes());
        }
        IlOpcode::Single(0x45) => {
            let relative_offset = instruction.operand_offset - instruction.offset;
            let count = read_u32(bytes, relative_offset)? as usize;
            let old_base = instruction
                .offset
                .checked_add(instruction.size)
                .ok_or_else(|| "switch base offset overflowed".to_string())?;
            let new_base = map_il_offset_after_insertions(old_base, insertions) as isize;
            for case_index in 0..count {
                let operand_offset = relative_offset + 4 + case_index * 4;
                let old_delta = read_i32(bytes, operand_offset)? as isize;
                let old_target = checked_branch_target(old_base, 0, old_delta, code_len)?;
                let new_target = map_il_offset_after_insertions(old_target, insertions) as isize;
                let new_delta = new_target - new_base;
                if !(i32::MIN as isize..=i32::MAX as isize).contains(&new_delta) {
                    return Err(format!(
                        "switch target at IL offset 0x{:x} is out of range after patch",
                        instruction.offset
                    ));
                }
                bytes[operand_offset..operand_offset + 4]
                    .copy_from_slice(&(new_delta as i32).to_le_bytes());
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn switch_operand_len(code: &[u8], operand_offset: usize) -> Result<usize, String> {
    let count = read_u32(code, operand_offset)? as usize;
    count
        .checked_mul(4)
        .and_then(|value| value.checked_add(4))
        .ok_or_else(|| format!("switch operand at 0x{operand_offset:x} is too large"))
}

pub(super) fn instruction_call_token(instruction: &IlInstruction, code: &[u8]) -> Option<u32> {
    if !matches!(instruction.opcode, IlOpcode::Single(0x28 | 0x6f)) || instruction.operand_len != 4
    {
        return None;
    }
    read_u32(code, instruction.operand_offset).ok()
}

pub(super) fn method_body_has_extra_sections(body: &MethodBody) -> bool {
    if body.header.len() < 2 {
        return false;
    }
    let flags_size = u16::from_le_bytes([body.header[0], body.header[1]]);
    flags_size & 0x0008 != 0
}

pub(super) fn increase_method_body_max_stack(body: &mut MethodBody, extra: u16) -> Result<(), String> {
    if body.header.len() < 4 {
        return Err("method body must use a fat header to adjust max stack".to_string());
    }
    let current = u16::from_le_bytes([body.header[2], body.header[3]]);
    let updated = current
        .checked_add(extra)
        .ok_or_else(|| "method body max stack overflowed".to_string())?;
    body.header[2..4].copy_from_slice(&updated.to_le_bytes());
    Ok(())
}

pub(super) fn checked_branch_target(
    offset: usize,
    size: usize,
    delta: isize,
    code_len: usize,
) -> Result<usize, String> {
    let base = offset
        .checked_add(size)
        .ok_or_else(|| "branch base offset overflowed".to_string())? as isize;
    let target = base + delta;
    if target < 0 || target as usize > code_len {
        return Err(format!(
            "branch at IL offset 0x{offset:x} targets invalid offset 0x{target:x}"
        ));
    }
    Ok(target as usize)
}

pub(super) fn map_il_offset_after_insertions(offset: usize, insertions: &[(usize, usize)]) -> usize {
    offset
        + insertions
            .iter()
            .filter(|(insert_at, _)| offset >= *insert_at)
            .map(|(_, len)| *len)
            .sum::<usize>()
}

pub(super) fn is_short_branch_opcode(opcode: u8) -> bool {
    matches!(opcode, 0x2b..=0x37 | 0xde)
}

pub(super) fn is_long_branch_opcode(opcode: u8) -> bool {
    matches!(opcode, 0x38..=0x44)
}

pub(crate) fn repair_cp1251_mojibake(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }
    let bytes = value
        .chars()
        .map(cp1251_byte_for_char)
        .collect::<Option<Vec<_>>>()?;
    let repaired = std::str::from_utf8(&bytes).ok()?;
    if repaired == value || !looks_like_repaired_mojibake(value, repaired) {
        return None;
    }
    Some(repaired.to_string())
}

pub(super) fn looks_like_repaired_mojibake(original: &str, repaired: &str) -> bool {
    let repaired_cyrillic = repaired.chars().filter(|ch| is_cyrillic(*ch)).count();
    if repaired_cyrillic == 0 {
        return false;
    }
    let original_markers = original
        .chars()
        .filter(|ch| matches!(*ch, '\u{0420}' | '\u{0421}') || is_cp1251_punctuation_marker(*ch))
        .count();
    original_markers >= 2
}

pub(super) fn is_cyrillic(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0400..=0x052f | 0x2de0..=0x2dff | 0xa640..=0xa69f
    )
}

pub(super) fn is_cp1251_punctuation_marker(ch: char) -> bool {
    matches!(
        ch,
        '\u{0402}'
            | '\u{0403}'
            | '\u{201a}'
            | '\u{0453}'
            | '\u{201e}'
            | '\u{2026}'
            | '\u{2020}'
            | '\u{2021}'
            | '\u{20ac}'
            | '\u{2030}'
            | '\u{0409}'
            | '\u{2039}'
            | '\u{040a}'
            | '\u{040c}'
            | '\u{040b}'
            | '\u{040f}'
            | '\u{0452}'
            | '\u{2018}'
            | '\u{2019}'
            | '\u{201c}'
            | '\u{201d}'
            | '\u{2022}'
            | '\u{2013}'
            | '\u{2014}'
            | '\u{2122}'
            | '\u{0459}'
            | '\u{203a}'
            | '\u{045a}'
            | '\u{045c}'
            | '\u{045b}'
            | '\u{045f}'
    )
}

pub(super) fn cp1251_byte_for_char(ch: char) -> Option<u8> {
    match ch as u32 {
        0x0000..=0x007f => Some(ch as u8),
        0x0402 => Some(0x80),
        0x0403 => Some(0x81),
        0x201a => Some(0x82),
        0x0453 => Some(0x83),
        0x201e => Some(0x84),
        0x2026 => Some(0x85),
        0x2020 => Some(0x86),
        0x2021 => Some(0x87),
        0x20ac => Some(0x88),
        0x2030 => Some(0x89),
        0x0409 => Some(0x8a),
        0x2039 => Some(0x8b),
        0x040a => Some(0x8c),
        0x040c => Some(0x8d),
        0x040b => Some(0x8e),
        0x040f => Some(0x8f),
        0x0452 => Some(0x90),
        0x2018 => Some(0x91),
        0x2019 => Some(0x92),
        0x201c => Some(0x93),
        0x201d => Some(0x94),
        0x2022 => Some(0x95),
        0x2013 => Some(0x96),
        0x2014 => Some(0x97),
        0x2122 => Some(0x99),
        0x0459 => Some(0x9a),
        0x203a => Some(0x9b),
        0x045a => Some(0x9c),
        0x045c => Some(0x9d),
        0x045b => Some(0x9e),
        0x045f => Some(0x9f),
        0x00a0 => Some(0xa0),
        0x040e => Some(0xa1),
        0x045e => Some(0xa2),
        0x0408 => Some(0xa3),
        0x00a4 => Some(0xa4),
        0x0490 => Some(0xa5),
        0x00a6 => Some(0xa6),
        0x00a7 => Some(0xa7),
        0x0401 => Some(0xa8),
        0x00a9 => Some(0xa9),
        0x0404 => Some(0xaa),
        0x00ab => Some(0xab),
        0x00ac => Some(0xac),
        0x00ad => Some(0xad),
        0x00ae => Some(0xae),
        0x0407 => Some(0xaf),
        0x00b0 => Some(0xb0),
        0x00b1 => Some(0xb1),
        0x0406 => Some(0xb2),
        0x0456 => Some(0xb3),
        0x0491 => Some(0xb4),
        0x00b5 => Some(0xb5),
        0x00b6 => Some(0xb6),
        0x00b7 => Some(0xb7),
        0x0451 => Some(0xb8),
        0x2116 => Some(0xb9),
        0x0454 => Some(0xba),
        0x00bb => Some(0xbb),
        0x0458 => Some(0xbc),
        0x0405 => Some(0xbd),
        0x0455 => Some(0xbe),
        0x0457 => Some(0xbf),
        0x0410..=0x044f => Some((ch as u32 - 0x0410 + 0xc0) as u8),
        _ => None,
    }
}

pub(super) fn has_panel_equip_translations(entries: &[JsonValue]) -> bool {
    entries.iter().any(|entry| {
        entry.get("type").and_then(JsonValue::as_str) == Some("Panel_Equip")
            && has_text(entry.get("translation"))
    })
}

pub(super) fn bypass_panel_equip_prelocalized_getstr(context: &mut AssemblyContext) -> Result<usize, String> {
    let Some(getstr_token) = method_def_token(context, "TextManager::GetStr(System.String)") else {
        return Ok(0);
    };
    let mut patched = 0usize;
    for type_index in 1..=context.metadata.type_defs.len() as u32 {
        let type_name = context
            .type_names
            .get(type_index as usize)
            .map(String::as_str)
            .unwrap_or_default();
        if type_name != "Panel_Equip" {
            continue;
        }
        for (method_index, method) in context.metadata.get_type_methods(type_index) {
            if method.rva == 0 {
                continue;
            }
            let method_name = context
                .method_names
                .get(method_index as usize)
                .map(String::as_str)
                .unwrap_or_default();
            if !matches!(
                method_name,
                "Panel_Equip::Start()" | "Panel_Equip::DoEquipPanel()"
            ) {
                continue;
            }
            let body = read_method_body(&context.pe, method.rva)?;
            for (il_offset, token) in scan_call_tokens(&body.code)? {
                if token != getstr_token {
                    continue;
                }
                context
                    .pe
                    .write_at_rva(body.code_rva + il_offset as u32, &[0, 0, 0, 0, 0])
                    .ok_or_else(|| {
                        format!(
                            "could not patch Panel_Equip GetStr call at RVA 0x{:x}",
                            body.code_rva + il_offset as u32
                        )
                    })?;
                patched += 1;
            }
        }
    }
    Ok(patched)
}

pub(super) fn resource_locator_character_names(
    context: &AssemblyContext,
    body: &MethodBody,
) -> Result<BTreeSet<String>, String> {
    let mut result = BTreeSet::new();
    for (_, token) in scan_ldstr_tokens(&body.code)? {
        let user_string_offset = token & 0x00ff_ffff;
        let Ok(source) = context.metadata.user_strings.get(user_string_offset) else {
            continue;
        };
        let lower = source.to_ascii_lowercase();
        if lower.ends_with(".resourcefile")
            && (lower.starts_with("character_")
                || lower.starts_with("coreshared")
                || lower.starts_with("tutorialaudio")
                || lower.starts_with("uiaudio")
                || lower.starts_with("npcvoiceshared")
                || lower.starts_with("worldshared_")
                || lower.starts_with("npc_pack_")
                || lower.starts_with("hnpc_pack_")
                || lower.starts_with("nano_pack_")
                || lower.starts_with("playercharacter_pack_")
                || lower.starts_with("items_pack_")
                || lower.starts_with("icons_pack_"))
        {
            result.insert(lower);
        }
    }
    Ok(result)
}

pub(super) fn resource_locator_character_insert_offset(body: &MethodBody) -> Result<usize, String> {
    let prefix = [
        0x04, // ldarg.2
        0x02, // ldarg.0
        0x51, // stind.ref
    ];
    body.code
        .windows(prefix.len())
        .position(|window| window == prefix)
        .map(|offset| offset + prefix.len())
        .ok_or_else(|| "ResourceLocator insertion point was not found".to_string())
}

pub(super) fn resource_locator_character_template(
    _context: &AssemblyContext,
    body: &MethodBody,
) -> Result<ResourceLocatorCharacterTemplate, String> {
    let code = &body.code;
    let Some(start) = find_subslice(
        code,
        &[
            0x7e, 0, 0, 0, 0, // ldsfld AssetLoader::m_LoadWWWClasses
            0x74, 0, 0, 0, 0, // castclass Hashtable
            0x72, 0, 0, 0, 0, // ldstr existing bundle
            0x6f, 0, 0, 0, 0, // callvirt Hashtable::get_Item
            0x74, 0, 0, 0, 0, // castclass AssetBundle
        ],
    ) else {
        return Err("ResourceLocator bundle load template was not found".to_string());
    };
    let object_inequality = find_call_after(code, start, &[0x14, 0x28])?;
    let contains = find_call_after(code, object_inequality + 5, &[0x04, 0x50, 0x6f])?;
    Ok(ResourceLocatorCharacterTemplate {
        load_www_field_token: [
            code[start + 1],
            code[start + 2],
            code[start + 3],
            code[start + 4],
        ],
        hashtable_type_token: [
            code[start + 6],
            code[start + 7],
            code[start + 8],
            code[start + 9],
        ],
        hashtable_get_item_token: [
            code[start + 16],
            code[start + 17],
            code[start + 18],
            code[start + 19],
        ],
        asset_bundle_type_token: [
            code[start + 21],
            code[start + 22],
            code[start + 23],
            code[start + 24],
        ],
        object_inequality_token: [
            code[object_inequality + 1],
            code[object_inequality + 2],
            code[object_inequality + 3],
            code[object_inequality + 4],
        ],
        asset_bundle_contains_token: [
            code[contains + 1],
            code[contains + 2],
            code[contains + 3],
            code[contains + 4],
        ],
    })
}

pub(super) fn resource_locator_character_block(
    template: &ResourceLocatorCharacterTemplate,
    name_token: u32,
) -> Vec<u8> {
    let mut block = Vec::with_capacity(57);
    block.push(0x7e); // ldsfld AssetLoader::m_LoadWWWClasses
    block.extend_from_slice(&template.load_www_field_token);
    block.push(0x74); // castclass Hashtable
    block.extend_from_slice(&template.hashtable_type_token);
    block.push(0x72); // ldstr bundle name
    block.extend_from_slice(&name_token.to_le_bytes());
    block.push(0x6f); // callvirt Hashtable::get_Item
    block.extend_from_slice(&template.hashtable_get_item_token);
    block.push(0x74); // castclass AssetBundle
    block.extend_from_slice(&template.asset_bundle_type_token);
    block.push(0x13); // stloc.s 14
    block.push(0x0e);
    block.push(0x11); // ldloc.s 14
    block.push(0x0e);
    block.push(0x14); // ldnull
    block.push(0x28); // call UnityEngine.Object::op_Inequality
    block.extend_from_slice(&template.object_inequality_token);
    block.push(0x2c); // brfalse.s next
    block.push(0x14);
    block.push(0x11); // ldloc.s 14
    block.push(0x0e);
    block.push(0x04); // ldarg.2
    block.push(0x50); // ldind.ref
    block.push(0x6f); // callvirt AssetBundle::Contains
    block.extend_from_slice(&template.asset_bundle_contains_token);
    block.push(0x2c); // brfalse.s next
    block.push(0x09);
    block.push(0x03); // ldarg.1
    block.push(0x72); // ldstr bundle name
    block.extend_from_slice(&name_token.to_le_bytes());
    block.push(0x51); // stind.ref
    block.push(0x17); // ldc.i4.1
    block.push(0x2a); // ret
    block
}

pub(super) fn rebuild_method_body_with_code(body: &MethodBody, code: &[u8]) -> Result<Vec<u8>, String> {
    if body.header.len() < 12 {
        return Err("AssetLoader::Awake() must use a fat method header to be extended".to_string());
    }
    if code.len() > u32::MAX as usize {
        return Err("AssetLoader::Awake() patched method body is too large".to_string());
    }
    let mut output = body.header.clone();
    output[4..8].copy_from_slice(&(code.len() as u32).to_le_bytes());
    output.extend_from_slice(code);
    while output.len() % 4 != 0 {
        output.push(0);
    }
    Ok(output)
}

pub(super) fn method_body_rva_replacement_tables(
    context: &AssemblyContext,
    method_index: u32,
    expected_rva: u32,
    new_rva: u32,
) -> Result<(String, Vec<u8>), String> {
    let tables_stream_name = context
        .metadata
        .root
        .tables_stream()
        .map(|stream| stream.name.clone())
        .ok_or_else(|| "CLR metadata has no tables stream".to_string())?;
    let mut tables = metadata_stream_payload(
        &context.metadata,
        &context.metadata_bytes,
        &tables_stream_name,
    )?;
    let method_row_offset =
        method_def_row_offset_in_tables(&context.metadata, &tables, method_index)?;
    let current_rva = read_u32(&tables, method_row_offset)?;
    if current_rva != expected_rva {
        return Err(format!(
            "MethodDef row mismatch: expected RVA 0x{expected_rva:x}, found 0x{current_rva:x}",
        ));
    }
    tables
        .get_mut(method_row_offset..method_row_offset + 4)
        .ok_or_else(|| "MethodDef RVA is out of range".to_string())?
        .copy_from_slice(&new_rva.to_le_bytes());
    Ok((tables_stream_name, tables))
}
