use super::*;

pub(super) fn parse_il_instructions(code: &[u8]) -> Result<Vec<IlInstruction>, String> {
    let mut result = Vec::new();
    let mut offset = 0usize;
    while offset < code.len() {
        let instruction_offset = offset;
        let opcode_byte = code[offset];
        offset += 1;
        let opcode = if opcode_byte == 0xfe {
            let sub = *code.get(offset).ok_or_else(|| {
                format!("truncated extended IL opcode at 0x{instruction_offset:x}")
            })?;
            offset += 1;
            IlOpcode::Extended(sub)
        } else {
            IlOpcode::Single(opcode_byte)
        };
        let operand_offset = offset;
        let operand_len = match opcode {
            IlOpcode::Single(0x45) => switch_operand_len(code, operand_offset)?,
            IlOpcode::Single(value) => single_byte_operand_len(value),
            IlOpcode::Extended(value) => extended_operand_len(value),
        };
        offset = offset
            .checked_add(operand_len)
            .ok_or_else(|| format!("IL instruction at 0x{instruction_offset:x} size overflowed"))?;
        if offset > code.len() {
            return Err(format!(
                "IL instruction at 0x{instruction_offset:x} extends past method body"
            ));
        }
        result.push(IlInstruction {
            offset: instruction_offset,
            opcode,
            operand_offset,
            operand_len,
            size: offset - instruction_offset,
        });
    }
    Ok(result)
}

pub(super) fn find_subslice(code: &[u8], pattern: &[u8]) -> Option<usize> {
    code.windows(pattern.len()).position(|window| {
        pattern
            .iter()
            .enumerate()
            .all(|(index, expected)| *expected == 0 || window[index] == *expected)
    })
}

pub(super) fn find_call_after(code: &[u8], start: usize, prefix: &[u8]) -> Result<usize, String> {
    let Some(offset) = code[start..]
        .windows(prefix.len() + 4)
        .position(|window| window.starts_with(prefix))
    else {
        return Err("ResourceLocator call token template was not found".to_string());
    };
    Ok(start + offset + prefix.len() - 1)
}

pub(super) fn find_all_bytes(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return Vec::new();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, value)| (value == needle).then_some(index))
        .collect()
}

pub(super) fn resolve_assemblies(root: &Path, options: &ManagedExportOptions) -> Result<Vec<PathBuf>, String> {
    if options.all_assemblies {
        let mut assemblies = fs::read_dir(root)
            .map_err(|err| format!("{}: {err}", root.display()))?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("dll"))
            .collect::<Vec<_>>();
        assemblies.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
        return Ok(assemblies);
    }

    let names = if options.assemblies.is_empty() {
        DEFAULT_ASSEMBLIES
            .iter()
            .map(|value| value.to_string())
            .collect()
    } else {
        options.assemblies.clone()
    };
    let mut assemblies = names
        .into_iter()
        .map(|name| root.join(name))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();
    assemblies.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    Ok(assemblies)
}

pub(super) fn read_cli_header(pe: &PE) -> Result<CliHeader, String> {
    let directory = pe
        .data_directory(DataDirectoryType::ClrRuntime)
        .filter(|directory| directory.is_present())
        .ok_or_else(|| "CLR runtime header was not found".to_string())?;
    let header = pe
        .read_at_rva(directory.virtual_address, 16)
        .ok_or_else(|| "CLR runtime header is out of range".to_string())?;
    Ok(CliHeader {
        rva: directory.virtual_address,
        metadata_rva: read_u32(header, 8)?,
        metadata_size: read_u32(header, 12)?,
    })
}

pub(super) fn read_method_body(pe: &PE, rva: u32) -> Result<MethodBody, String> {
    let first = *pe
        .read_at_rva(rva, 1)
        .and_then(|bytes| bytes.first())
        .ok_or_else(|| format!("method body RVA 0x{rva:x} is out of range"))?;
    let (header_size, code_size) = if first & 0x3 == 0x2 {
        (1usize, (first >> 2) as usize)
    } else if first & 0x3 == 0x3 {
        let header = pe
            .read_at_rva(rva, 12)
            .ok_or_else(|| format!("fat method body RVA 0x{rva:x} is out of range"))?;
        let flags_size = u16::from_le_bytes([header[0], header[1]]);
        let header_size = ((flags_size >> 12) as usize) * 4;
        let code_size = read_u32(header, 4)? as usize;
        (header_size, code_size)
    } else {
        return Err(format!("unsupported method body header at RVA 0x{rva:x}"));
    };
    let header = pe
        .read_at_rva(rva, header_size)
        .ok_or_else(|| format!("method header RVA 0x{rva:x} size {header_size} is out of range"))?
        .to_vec();
    let code_rva = rva + header_size as u32;
    let code = pe
        .read_at_rva(code_rva, code_size)
        .ok_or_else(|| format!("method code RVA 0x{code_rva:x} size {code_size} is out of range"))?
        .to_vec();
    Ok(MethodBody {
        header,
        code_rva,
        code,
    })
}

pub(super) fn read_json(path: &Path) -> Result<JsonValue, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|err| format!("{}: {err}", path.display()))
}

pub(super) fn read_u32(data: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| format!("u32 at offset {offset} is out of range"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub(super) fn read_i32(data: &[u8], offset: usize) -> Result<i32, String> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| format!("i32 at offset {offset} is out of range"))?;
    Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub(super) fn read_u16(data: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| format!("u16 at offset {offset} is out of range"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}
