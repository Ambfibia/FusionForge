use super::*;

pub(super) fn rebuild_metadata_with_replacements(
    metadata: &Metadata,
    original: &[u8],
    replacements: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<u8>, String> {
    for name in replacements.keys() {
        if metadata.root.find_stream(name).is_none() {
            return Err(format!("CLR metadata has no {name} stream"));
        }
    }

    let mut payloads = Vec::<(String, Vec<u8>)>::new();
    for stream in &metadata.root.streams {
        let payload = if let Some(replacement) = replacements.get(&stream.name) {
            replacement.clone()
        } else {
            original
                .get(stream.offset as usize..stream.offset as usize + stream.size as usize)
                .ok_or_else(|| format!("metadata stream {} is out of range", stream.name))?
                .to_vec()
        };
        payloads.push((stream.name.clone(), payload));
    }

    let mut root = metadata.root.clone();
    let mut offset = root.header_size();
    for stream in &mut root.streams {
        let payload = payloads
            .iter()
            .find(|(name, _)| name == &stream.name)
            .map(|(_, data)| data)
            .ok_or_else(|| format!("metadata stream {} payload was not found", stream.name))?;
        stream.offset = offset as u32;
        stream.size = payload.len() as u32;
        offset = align_up_usize(offset + payload.len(), 4);
    }

    let mut output = root.write();
    if output.len() != root.header_size() {
        return Err("rebuilt CLR metadata header size changed unexpectedly".to_string());
    }
    for stream in &root.streams {
        let payload = payloads
            .iter()
            .find(|(name, _)| name == &stream.name)
            .map(|(_, data)| data)
            .ok_or_else(|| format!("metadata stream {} payload was not found", stream.name))?;
        if output.len() > stream.offset as usize {
            return Err(format!("metadata stream {} overlaps header", stream.name));
        }
        output.resize(stream.offset as usize, 0);
        output.extend_from_slice(payload);
        while output.len() % 4 != 0 {
            output.push(0);
        }
    }
    Ok(output)
}

pub(super) fn same_instruction(entry: &JsonValue, instruction: &LdstrInstruction) -> bool {
    entry.get("type").and_then(JsonValue::as_str).unwrap_or("") == instruction.type_name
        && entry
            .get("method")
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            == instruction.method_name
        && entry
            .get("ilOffset")
            .or_else(|| entry.get("il_offset"))
            .and_then(JsonValue::as_u64)
            == Some(instruction.il_offset as u64)
        && entry
            .get("source")
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            == instruction.source
}

pub(super) fn managed_entry_sort_key(entry: &JsonValue) -> (String, String, String, u64) {
    (
        entry_file(entry),
        entry
            .get("type")
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            .to_string(),
        entry
            .get("method")
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            .to_string(),
        entry
            .get("ilOffset")
            .and_then(JsonValue::as_u64)
            .unwrap_or_default(),
    )
}

pub(super) fn entry_file(entry: &JsonValue) -> String {
    entry
        .get("file")
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .replace('\\', "/")
}

pub(super) fn build_entry_id(
    file: &str,
    type_name: &str,
    method_name: &str,
    il_offset: usize,
    occurrence: usize,
    source: &str,
) -> String {
    let value = format!("{type_name}|{method_name}|{il_offset}|{occurrence}|{source}");
    format!("managed.ldstr:{file}:{}", &sha1_hex(&value)[..16])
}

pub(super) fn entry_source_for_hash(source: &str) -> &str {
    source
}

pub(super) fn sha1_hex(value: &str) -> String {
    let digest = Sha1::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn file_source_key(file: &str, source: &str) -> String {
    format!("{}\u{1f}{source}", file.replace('\\', "/"))
}

pub(super) fn has_text(value: Option<&JsonValue>) -> bool {
    value
        .and_then(JsonValue::as_str)
        .is_some_and(|value| !value.is_empty())
}

pub(super) fn align_up(value: u32, alignment: u32) -> u32 {
    if alignment <= 1 {
        value
    } else {
        value.div_ceil(alignment) * alignment
    }
}

pub(super) fn align_up_usize(value: usize, alignment: usize) -> usize {
    if alignment <= 1 {
        value
    } else {
        value.div_ceil(alignment) * alignment
    }
}

/// Focused CLR metadata/IL from immutable container bytes; no decompiler project.
pub(crate) fn inspect_methods(bytes: &[u8], class: Option<&str>, method: Option<&str>, limit: usize) -> Result<JsonValue, String> {
    let context = AssemblyContext::from_bytes(Path::new("in-memory assembly"), bytes)?;
    let mut rows = Vec::new();
    let mut matches = 0;
    for (type_index, name) in context.type_names.iter().enumerate().skip(1) {
        if class.is_some_and(|filter| !name.to_lowercase().contains(&filter.to_lowercase())) {continue;}
        for (index, definition) in context.metadata.get_type_methods(type_index as u32) {
            let signature=&context.method_names[index as usize];
            if method.is_some_and(|filter| !signature.to_lowercase().contains(&filter.to_lowercase())) {continue;}
            matches += 1;
            if rows.len() >= limit {continue;}
            let il=if definition.rva == 0 {None} else {Some(read_method_body(&context.pe, definition.rva)?.code)};
            rows.push(json!({"class":name,"method":signature,"token":format!("0x{:08x}",0x06000000u32|index),
                "ilBytes":il.as_ref().map(|code|code.len()),"ilHex":il.as_ref().map(|code|code.iter().take(512).map(|b|format!("{b:02x}")).collect::<String>()),
                "instructions":il.as_ref().filter(|_|method.is_some()).map(|code|focused_instructions(&context,code)).transpose()?,
                "ilTruncated":il.as_ref().is_some_and(|code|code.len()>512),"nativeMapping":null,"mappingStatus":"unconfirmed"}));
        }
    }
    Ok(json!({"representation":"CLR metadata and raw IL, not original source or recovered behavior","matches":matches,"truncated":matches>limit,"methods":rows}))
}

/// Bounded decoded operands for a selected method, without an extracted assembly.
fn focused_instructions(context: &AssemblyContext, code: &[u8]) -> Result<Vec<String>, String> {
    let instructions = parse_il_instructions(code)?;
    let mut result = Vec::new();
    for instruction in instructions.iter().take(2048) {
        let operand = &code[instruction.operand_offset..instruction.operand_offset + instruction.operand_len];
        let mut value = operand.iter().map(|b| format!("{b:02x}")).collect::<String>();
        if operand.len() == 4 {
            let raw = u32::from_le_bytes(operand.try_into().unwrap());
            let row = (raw & 0x00ff_ffff) as usize;
            let symbol = match instruction.opcode {
                IlOpcode::Single(0x72) => context.metadata.user_strings.get(raw & 0x00ff_ffff).ok(),
                IlOpcode::Single(0x28 | 0x6f | 0x73 | 0x7b..=0x80) => match raw >> 24 {
                    6 => context.method_names.get(row).cloned(),
                    4 => row.checked_sub(1).and_then(|r| context.metadata.fields.get(r)).and_then(|f| context.metadata.strings.get(f.name).ok()).map(str::to_owned),
                    10 => row.checked_sub(1).and_then(|r| context.metadata.member_refs.get(r)).and_then(|m| {
                        let owner = super::operations_adjust_branch_operand::member_ref_owner_full_name(context, &m.class)?;
                        let name = context.metadata.strings.get(m.name).ok()?;
                        Some(format!("{owner}::{name}"))
                    }),
                    _ => None,
                },
                _ => None,
            };
            if let Some(symbol) = symbol { value = format!("0x{raw:08x} {symbol}"); }
            else if instruction.opcode == IlOpcode::Single(0x22) { value = format!("{}", f32::from_bits(raw)); }
            else if instruction.opcode == IlOpcode::Single(0x20) { value = format!("{}", raw as i32); }
        }
        result.push(format!("IL_{:04x} {:?} {value}", instruction.offset, instruction.opcode));
    }
    if instructions.len() > 2048 { result.push("[instruction limit reached]".to_owned()); }
    Ok(result)
}
