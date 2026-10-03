use super::*;

pub(super) fn method_def_row_offset_in_tables(
    metadata: &Metadata,
    tables: &[u8],
    row_index: u32,
) -> Result<usize, String> {
    if row_index == 0 || row_index as usize > metadata.method_defs.len() {
        return Err(format!("MethodDef row {row_index} is out of range"));
    }
    let ctx = metadata.tables_header.context();
    let row_size = ctx.row_size(TableId::MethodDef);
    let table_size = row_size
        .checked_mul(metadata.method_defs.len())
        .ok_or_else(|| "MethodDef table is too large".to_string())?;
    if table_size > tables.len() {
        return Err("MethodDef table is larger than the tables stream".to_string());
    }
    let probes = metadata
        .method_defs
        .iter()
        .enumerate()
        .filter(|(_, method)| method.rva != 0)
        .take(24)
        .collect::<Vec<_>>();
    let Some((probe_index, probe_method)) = probes.first().copied() else {
        return table_row_offset_in_tables(metadata, TableId::MethodDef, row_index);
    };
    let needle = probe_method.rva.to_le_bytes();
    let max_base = tables.len() - table_size;
    let probe_delta = probe_index
        .checked_mul(row_size)
        .ok_or_else(|| "MethodDef probe offset overflowed".to_string())?;

    for position in find_all_bytes(tables, &needle) {
        if position < probe_delta {
            continue;
        }
        let base = position - probe_delta;
        if base > max_base {
            continue;
        }
        if probes.iter().all(|(index, method)| {
            let offset = base + index * row_size;
            read_u32(tables, offset).ok() == Some(method.rva)
        }) {
            return Ok(base + (row_index as usize - 1) * row_size);
        }
    }

    table_row_offset_in_tables(metadata, TableId::MethodDef, row_index)
}

pub(super) fn table_row_offset_in_tables(
    metadata: &Metadata,
    table: TableId,
    row_index: u32,
) -> Result<usize, String> {
    if row_index == 0 || row_index > metadata.tables_header.row_count(table) {
        return Err(format!(
            "metadata table {table:?} row {row_index} is out of range"
        ));
    }
    let ctx = metadata.tables_header.context();
    let mut offset = metadata.tables_header.size();
    for (current, count) in metadata.tables_header.tables() {
        if current == table {
            return Ok(offset + (row_index as usize - 1) * ctx.row_size(table));
        }
        offset += count as usize * ctx.row_size(current);
    }
    Err(format!("metadata table {table:?} was not found"))
}

pub(super) fn method_def_token(context: &AssemblyContext, name: &str) -> Option<u32> {
    context
        .method_names
        .iter()
        .position(|method_name| method_name == name)
        .map(|index| 0x0600_0000u32 | index as u32)
}

pub(super) fn append_user_string(output: &mut Vec<u8>, value: &str) -> Result<(), String> {
    let utf16 = value.encode_utf16().collect::<Vec<_>>();
    let byte_len = utf16
        .len()
        .checked_mul(2)
        .ok_or_else(|| "user string is too large".to_string())?;
    let blob_len = byte_len
        .checked_add(1)
        .ok_or_else(|| "user string is too large".to_string())?;
    if blob_len > 0x1fff_ffff {
        return Err("user string is too large".to_string());
    }
    write_compressed_uint(output, blob_len as u32);
    for unit in &utf16 {
        output.extend_from_slice(&unit.to_le_bytes());
    }
    let has_special = utf16
        .iter()
        .any(|&unit| unit > 0x7f || matches!(unit, 0x01..=0x08 | 0x0e..=0x1f | 0x27 | 0x2d));
    output.push(if has_special { 1 } else { 0 });
    Ok(())
}

pub(super) fn scan_ldstrs(context: &AssemblyContext) -> Result<Vec<LdstrInstruction>, String> {
    let mut result = Vec::new();
    for type_index in 1..=context.metadata.type_defs.len() as u32 {
        let type_name = context
            .type_names
            .get(type_index as usize)
            .cloned()
            .unwrap_or_default();
        for (method_index, method) in context.metadata.get_type_methods(type_index) {
            if method.rva == 0 {
                continue;
            }
            let method_name = context
                .method_names
                .get(method_index as usize)
                .cloned()
                .unwrap_or_else(|| format!("{type_name}::<method-{method_index}>"));
            let body = read_method_body(&context.pe, method.rva)?;
            let mut occurrence = 0usize;
            for (il_offset, token) in scan_ldstr_tokens(&body.code)? {
                let user_string_offset = token & 0x00ff_ffff;
                let Ok(source) = context.metadata.user_strings.get(user_string_offset) else {
                    continue;
                };
                occurrence += 1;
                result.push(LdstrInstruction {
                    type_name: type_name.clone(),
                    method_name: method_name.clone(),
                    il_offset,
                    occurrence,
                    source,
                    operand_rva: body.code_rva + il_offset as u32 + 1,
                });
            }
        }
    }
    Ok(result)
}

pub(super) fn scan_ldstr_tokens(code: &[u8]) -> Result<Vec<(usize, u32)>, String> {
    let mut result = Vec::new();
    let mut offset = 0usize;
    while offset < code.len() {
        let opcode_offset = offset;
        let opcode = code[offset];
        offset += 1;
        if opcode == 0xfe {
            if offset >= code.len() {
                break;
            }
            let sub = code[offset];
            offset += 1;
            let len = extended_operand_len(sub);
            if offset + len > code.len() {
                break;
            }
            offset += len;
            continue;
        }
        if opcode == 0x72 {
            if offset + 4 > code.len() {
                break;
            }
            let token = u32::from_le_bytes([
                code[offset],
                code[offset + 1],
                code[offset + 2],
                code[offset + 3],
            ]);
            if token & 0xff00_0000 == 0x7000_0000 {
                result.push((opcode_offset, token));
            }
            offset += 4;
            continue;
        }
        if opcode == 0x45 {
            if offset + 4 > code.len() {
                break;
            }
            let count = u32::from_le_bytes([
                code[offset],
                code[offset + 1],
                code[offset + 2],
                code[offset + 3],
            ]) as usize;
            let len = 4 + count.saturating_mul(4);
            if offset + len > code.len() {
                break;
            }
            offset += len;
            continue;
        }
        let len = single_byte_operand_len(opcode);
        if offset + len > code.len() {
            break;
        }
        offset += len;
    }
    Ok(result)
}

pub(super) fn scan_call_tokens(code: &[u8]) -> Result<Vec<(usize, u32)>, String> {
    let mut result = Vec::new();
    let mut offset = 0usize;
    while offset < code.len() {
        let opcode_offset = offset;
        let opcode = code[offset];
        offset += 1;
        if opcode == 0xfe {
            if offset >= code.len() {
                break;
            }
            let sub = code[offset];
            offset += 1;
            let len = extended_operand_len(sub);
            if offset + len > code.len() {
                break;
            }
            offset += len;
            continue;
        }
        if opcode == 0x28 {
            if offset + 4 > code.len() {
                break;
            }
            let token = u32::from_le_bytes([
                code[offset],
                code[offset + 1],
                code[offset + 2],
                code[offset + 3],
            ]);
            result.push((opcode_offset, token));
            offset += 4;
            continue;
        }
        if opcode == 0x45 {
            if offset + 4 > code.len() {
                break;
            }
            let count = u32::from_le_bytes([
                code[offset],
                code[offset + 1],
                code[offset + 2],
                code[offset + 3],
            ]) as usize;
            let len = 4 + count.saturating_mul(4);
            if offset + len > code.len() {
                break;
            }
            offset += len;
            continue;
        }
        let len = single_byte_operand_len(opcode);
        if offset + len > code.len() {
            break;
        }
        offset += len;
    }
    Ok(result)
}

pub(super) fn single_byte_operand_len(opcode: u8) -> usize {
    match opcode {
        0x0e..=0x13 | 0x1f | 0x2b..=0x37 | 0xde => 1,
        0x20
        | 0x22
        | 0x27..=0x29
        | 0x38..=0x44
        | 0x6f..=0x75
        | 0x79
        | 0x7b..=0x81
        | 0x8c
        | 0x8d
        | 0x8f
        | 0xa5
        | 0xc2
        | 0xc6
        | 0xd0 => 4,
        0x21 | 0x23 => 8,
        _ => 0,
    }
}

pub(super) fn extended_operand_len(opcode: u8) -> usize {
    match opcode {
        0x12 => 1,
        0x06 | 0x07 | 0x15 | 0x16 | 0x1c => 4,
        0x09..=0x0e => 2,
        _ => 0,
    }
}

pub(super) fn build_type_names(metadata: &Metadata) -> Vec<String> {
    let mut names = vec![String::new(); metadata.type_defs.len() + 1];
    let nested = metadata
        .nested_classes
        .iter()
        .map(|row| (row.nested_class, row.enclosing_class))
        .collect::<HashMap<_, _>>();
    for index in 1..=metadata.type_defs.len() as u32 {
        names[index as usize] = type_full_name(metadata, &nested, index);
    }
    names
}

pub(super) fn type_full_name(metadata: &Metadata, nested: &HashMap<u32, u32>, index: u32) -> String {
    let Some(row) = metadata.get_type_def(index) else {
        return format!("<TypeDef {index}>");
    };
    let name = metadata
        .strings
        .get(row.type_name)
        .unwrap_or("")
        .to_string();
    if let Some(enclosing) = nested.get(&index).copied() {
        let parent = type_full_name(metadata, nested, enclosing);
        if parent.is_empty() {
            return name;
        }
        return format!("{parent}/{name}");
    }
    let namespace = if row.type_namespace != 0 {
        metadata.strings.get(row.type_namespace).unwrap_or("")
    } else {
        ""
    };
    if namespace.is_empty() {
        name
    } else {
        format!("{namespace}.{name}")
    }
}

pub(super) fn build_method_names(metadata: &Metadata, type_names: &[String]) -> Vec<String> {
    let mut names = vec![String::new(); metadata.method_defs.len() + 1];
    let mut method_to_type = vec![0usize; metadata.method_defs.len() + 1];
    for type_index in 1..=metadata.type_defs.len() as u32 {
        for (method_index, _) in metadata.get_type_methods(type_index) {
            if let Some(slot) = method_to_type.get_mut(method_index as usize) {
                *slot = type_index as usize;
            }
        }
    }
    for (index, method) in metadata.method_defs.iter().enumerate() {
        let method_index = index + 1;
        let type_name = method_to_type
            .get(method_index)
            .and_then(|type_index| type_names.get(*type_index))
            .map(String::as_str)
            .unwrap_or("");
        names[method_index] = format_method_name(metadata, type_name, method);
    }
    names
}

pub(super) fn format_method_name(metadata: &Metadata, type_name: &str, method: &MethodDefRow) -> String {
    let name = metadata.strings.get(method.name).unwrap_or("<method>");
    let params = metadata
        .blobs
        .get(method.signature)
        .ok()
        .and_then(|blob| MethodSig::parse_blob(blob).ok())
        .map(|signature| {
            signature
                .params
                .iter()
                .map(|param| format_type_sig(metadata, param))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    format!("{type_name}::{name}({params})")
}

pub(super) fn format_type_sig(metadata: &Metadata, sig: &TypeSig) -> String {
    match sig {
        TypeSig::Primitive(kind) => primitive_type_name(*kind).to_string(),
        TypeSig::Class(token) | TypeSig::ValueType(token) => {
            let coded = clrmeta::CodedIndex::decode(clrmeta::CodedIndexKind::TypeDefOrRef, *token);
            metadata
                .resolve_type(&coded)
                .map(|value| value.full_name())
                .unwrap_or_else(|| format!("<type {token}>"))
        }
        TypeSig::SzArray(inner) => format!("{}[]", format_type_sig(metadata, inner)),
        TypeSig::Array { element_type, .. } => {
            format!("{}[]", format_type_sig(metadata, element_type))
        }
        TypeSig::Ptr(inner) => format!("{}*", format_type_sig(metadata, inner)),
        TypeSig::ByRef(inner) => format!("{}&", format_type_sig(metadata, inner)),
        TypeSig::GenericInst {
            type_ref,
            type_args,
            ..
        } => {
            let coded =
                clrmeta::CodedIndex::decode(clrmeta::CodedIndexKind::TypeDefOrRef, *type_ref);
            let base = metadata
                .resolve_type(&coded)
                .map(|value| value.full_name())
                .unwrap_or_else(|| format!("<type {type_ref}>"));
            let args = type_args
                .iter()
                .map(|arg| format_type_sig(metadata, arg))
                .collect::<Vec<_>>()
                .join(",");
            format!("{base}<{args}>")
        }
        TypeSig::Var(index) => format!("!{index}"),
        TypeSig::MVar(index) => format!("!!{index}"),
        TypeSig::FnPtr(_) => "method".to_string(),
        TypeSig::Modified { inner, .. } | TypeSig::Pinned(inner) => {
            format_type_sig(metadata, inner)
        }
        _ => "<type>".to_string(),
    }
}

pub(super) fn primitive_type_name(kind: ElementType) -> &'static str {
    match kind {
        ElementType::Void => "System.Void",
        ElementType::Boolean => "System.Boolean",
        ElementType::Char => "System.Char",
        ElementType::I1 => "System.SByte",
        ElementType::U1 => "System.Byte",
        ElementType::I2 => "System.Int16",
        ElementType::U2 => "System.UInt16",
        ElementType::I4 => "System.Int32",
        ElementType::U4 => "System.UInt32",
        ElementType::I8 => "System.Int64",
        ElementType::U8 => "System.UInt64",
        ElementType::R4 => "System.Single",
        ElementType::R8 => "System.Double",
        ElementType::String => "System.String",
        ElementType::TypedByRef => "System.TypedReference",
        ElementType::IntPtr => "System.IntPtr",
        ElementType::UIntPtr => "System.UIntPtr",
        ElementType::Object => "System.Object",
        _ => kind.name(),
    }
}

pub(super) fn contains_only_latin_or_cyrillic_letters(source: &str) -> bool {
    source.chars().all(|ch| {
        if !ch.is_alphabetic() {
            return true;
        }
        let code = ch as u32;
        matches!(
            code,
            0x0041..=0x007a
                | 0x00c0..=0x024f
                | 0x1e00..=0x1eff
                | 0x0400..=0x052f
                | 0x2de0..=0x2dff
                | 0xa640..=0xa69f
        )
    })
}

pub(super) fn is_ui_string(type_name: &str, method_name: &str, source: &str) -> bool {
    is_ui_context(type_name, method_name)
        && !is_technical_source(source)
        && is_likely_human_text(source)
}

pub(super) fn is_ui_context(type_name: &str, method_name: &str) -> bool {
    let text = format!("{type_name} {method_name}").to_ascii_lowercase();
    [
        "gui",
        "panel",
        "popup",
        "window",
        "login",
        "charselection",
        "charcreation",
        "namecreation",
        "serverselection",
        "missionjournal",
        "systemmessage",
        "optionmode",
        "guide",
        "race",
        "trade",
        "email",
        "vendor",
        "cashmall",
        "userclothes",
        "userstore",
        "quit",
        "worldmap",
        "nanocom",
        "menu",
        "inventory",
        "transport",
        "itemdisplay",
        "gum",
        "turing",
        "chat",
    ]
    .iter()
    .any(|hint| text.contains(hint))
}

pub(super) fn is_technical_source(source: &str) -> bool {
    let text = source.trim();
    if text.is_empty() {
        return true;
    }
    let lower = text.to_ascii_lowercase();
    if [
        "null",
        "none",
        "toggle",
        "label",
        "box",
        "button",
        "player",
        "transparent",
        "transparent3",
        "imagewindow",
        "left_box",
        "sel_box",
        "rightlabel",
        "closebut",
        "bigfont16",
        "jeff12skyblue",
        "redbutton",
    ]
    .iter()
    .any(|value| value.eq_ignore_ascii_case(text))
    {
        return true;
    }
    if !text.chars().any(char::is_alphabetic) {
        return true;
    }
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("www.") {
        return true;
    }
    if lower.contains(".com") || lower.contains(".php") || lower.contains(".do") {
        return true;
    }
    if lower.contains("gabage collect") || lower.contains("garbage collect") {
        return true;
    }
    if text.contains("sP_") || text.contains("P_FE2CL") || text.contains("P_CL2") {
        return true;
    }
    if contains_word_any(
        text,
        &[
            "csDefines",
            "TempItem",
            "InventoryManagerScript",
            "Bip01",
            "iUiMode",
            "slotType",
            "iColum",
            "PCUID",
        ],
    ) {
        return true;
    }
    if [
        "click slot",
        "inven slot",
        "bank slot",
        "request",
        "receive",
        "register form",
        "send sell",
        "buy ivendor",
        "char info slot",
        "icon is clicked",
    ]
    .iter()
    .any(|value| lower.contains(value))
    {
        return true;
    }
    if text.contains('/') || text.contains('\\') {
        return true;
    }
    if [
        ".png",
        ".dds",
        ".nif",
        ".mp3",
        ".wav",
        ".kfm",
        ".resourcefile",
        ".unity3d",
        ".dll",
        ".txt",
        ".xml",
        ".dat",
        ".jpg",
        ".ogg",
    ]
    .iter()
    .any(|suffix| lower.ends_with(suffix))
    {
        return true;
    }
    if text.contains('_') && !text.chars().any(char::is_whitespace) {
        return true;
    }
    contains_word_any(
        text,
        &[
            "Get", "Set", "Find", "Load", "Init", "Debug", "null", "curTime", "sendTime", "Event",
            "mDrop", "REQ", "REP",
        ],
    )
}

pub(super) fn contains_word_any(text: &str, words: &[&str]) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|part| words.iter().any(|word| part == *word))
}

pub(super) fn is_likely_human_text(source: &str) -> bool {
    let text = source.trim();
    if text.chars().any(char::is_whitespace) {
        return true;
    }
    if text
        .chars()
        .any(|ch| matches!(ch, '\'' | '!' | '?' | ':' | ','))
    {
        return true;
    }
    if text.chars().any(is_cyrillic) {
        return true;
    }
    if (2..=41).contains(&text.len())
        && text
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == ' ' || ch == '-')
    {
        return true;
    }
    let mut chars = text.chars();
    if let Some(first) = chars.next() {
        if first.is_ascii_uppercase()
            && chars.clone().count() <= 24
            && chars.all(|ch| ch.is_ascii_lowercase())
        {
            return true;
        }
    }
    text == "On" || text == "Off"
}

pub(super) fn embed_metadata(context: &mut AssemblyContext, user_strings: Vec<u8>) -> Result<(), String> {
    let mut replacements = BTreeMap::new();
    replacements.insert(StreamHeader::USER_STRINGS.to_string(), user_strings);
    embed_metadata_with_replacements(context, &replacements)
}

pub(super) fn embed_metadata_with_replacements(
    context: &mut AssemblyContext,
    replacements: &BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let mut metadata_bytes = rebuild_metadata_with_replacements(
        &context.metadata,
        &context.metadata_bytes,
        replacements,
    )?;
    while metadata_bytes.len() % 4 != 0 {
        metadata_bytes.push(0);
    }
    if metadata_bytes.len() <= context.cli.metadata_size as usize {
        let mut bytes = metadata_bytes;
        bytes.resize(context.cli.metadata_size as usize, 0);
        context
            .pe
            .write_at_rva(context.cli.metadata_rva, &bytes)
            .ok_or_else(|| "could not write updated metadata in-place".to_string())?;
        write_cli_metadata_pointer(
            &mut context.pe,
            context.cli.rva,
            context.cli.metadata_rva,
            bytes.len() as u32,
        )?;
        context.cli.metadata_size = bytes.len() as u32;
        return Ok(());
    }

    let section_index = context
        .pe
        .sections
        .len()
        .checked_sub(1)
        .ok_or_else(|| "PE has no sections".to_string())?;
    let section = &mut context.pe.sections[section_index];
    let aligned_offset = align_up_usize(section.data.len(), 4);
    if section.data.len() < aligned_offset {
        section.data.resize(aligned_offset, 0);
    }
    let new_metadata_rva = section.header.virtual_address + aligned_offset as u32;
    section.data.extend_from_slice(&metadata_bytes);
    section.header.virtual_size = section
        .header
        .virtual_size
        .max((aligned_offset + metadata_bytes.len()) as u32);
    write_cli_metadata_pointer(
        &mut context.pe,
        context.cli.rva,
        new_metadata_rva,
        metadata_bytes.len() as u32,
    )?;
    context.cli.metadata_rva = new_metadata_rva;
    context.cli.metadata_size = metadata_bytes.len() as u32;
    Ok(())
}
