use super::*;

pub(crate) fn patch_resource_locator_character_bundles(
    root: &Path,
    bundle_names: &[String],
) -> Result<usize, String> {
    if bundle_names.is_empty() {
        return Ok(0);
    }
    let assembly_path = root.join("Assembly - CSharp - first pass.dll");
    if !assembly_path.is_file() {
        return Err(format!(
            "ResourceLocator assembly was not found: {}",
            assembly_path.display()
        ));
    }

    let mut context = AssemblyContext::from_path(&assembly_path)?;
    let patched = patch_resource_locator_character_bundles_in_context(&mut context, bundle_names)?;
    if patched == 0 {
        return Ok(0);
    }

    write_pe_preserve_layout(&mut context.pe, &assembly_path)?;
    let reparsed = AssemblyContext::from_path(&assembly_path)?;
    let _ = reparsed.metadata.version();
    println!(
        "{}: added {patched} standalone NPC bundle(s) to ResourceLocator lookup",
        assembly_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("assembly")
    );
    Ok(patched)
}

pub(crate) fn patch_binary_reader_unicode_strings(root: &Path) -> Result<usize, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }

    let mut total = 0usize;
    for assembly in DEFAULT_ASSEMBLIES {
        let assembly_path = root.join(assembly);
        if !assembly_path.is_file() {
            continue;
        }
        let patched = patch_binary_reader_unicode_strings_in_assembly(&assembly_path)?;
        if patched > 0 {
            println!(
                "{}: patched {patched} BinaryReader Unicode string decode site(s)",
                assembly_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("assembly")
            );
        }
        total += patched;
    }

    Ok(total)
}

pub(super) fn patch_binary_reader_unicode_strings_in_assembly(assembly_path: &Path) -> Result<usize, String> {
    let mut context = AssemblyContext::from_path(assembly_path)?;
    let Some(tokens) = binary_reader_unicode_tokens(&context)? else {
        return Ok(0);
    };

    let mut patched_methods = Vec::<PatchedManagedMethod>::new();
    let mut patched_sites = 0usize;
    for (index, method) in context.metadata.method_defs.iter().enumerate() {
        if method.rva == 0 {
            continue;
        }
        let body = read_method_body(&context.pe, method.rva)?;
        let patched = patch_binary_reader_unicode_code(&body.code, &tokens)?;
        if patched.sites == 0 {
            continue;
        }
        let method_index = index as u32 + 1;
        let method_name = context
            .method_names
            .get(method_index as usize)
            .map(String::as_str)
            .unwrap_or("<method>");
        if method_body_has_extra_sections(&body) {
            return Err(format!(
                "{}: {method_name} has extra IL sections and cannot be safely extended",
                assembly_path.display()
            ));
        }

        let mut rebuilt_body = body.clone();
        if patched.code.len() > body.code.len() {
            increase_method_body_max_stack(&mut rebuilt_body, 1)?;
        }
        let new_body = rebuild_method_body_with_code(&rebuilt_body, &patched.code)?;
        patched_methods.push(PatchedManagedMethod {
            method_index,
            expected_rva: method.rva,
            body: new_body,
            new_rva: 0,
        });
        patched_sites += patched.sites;
    }

    if patched_methods.is_empty() {
        return Ok(0);
    }

    let mut section_data = Vec::new();
    for method in &mut patched_methods {
        let aligned_offset = align_up_usize(section_data.len(), 4);
        section_data.resize(aligned_offset, 0);
        method.new_rva = aligned_offset as u32;
        section_data.extend_from_slice(&method.body);
    }
    let base_rva = append_managed_patch_section(&mut context.pe, section_data)?;
    for method in &mut patched_methods {
        method.new_rva = base_rva + method.new_rva;
    }

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
    for method in &patched_methods {
        let method_row_offset =
            method_def_row_offset_in_tables(&context.metadata, &tables, method.method_index)?;
        let current_rva = read_u32(&tables, method_row_offset)?;
        if current_rva != method.expected_rva {
            return Err(format!(
                "MethodDef row mismatch: expected RVA 0x{:x}, found 0x{current_rva:x}",
                method.expected_rva
            ));
        }
        tables
            .get_mut(method_row_offset..method_row_offset + 4)
            .ok_or_else(|| "MethodDef RVA is out of range".to_string())?
            .copy_from_slice(&method.new_rva.to_le_bytes());
    }

    let mut replacements = BTreeMap::new();
    replacements.insert(tables_stream_name, tables);
    embed_metadata_with_replacements(&mut context, &replacements)?;
    write_pe_preserve_layout(&mut context.pe, assembly_path)?;
    let reparsed = AssemblyContext::from_path(assembly_path)?;
    let _ = reparsed.metadata.version();
    Ok(patched_sites)
}

pub(super) fn patch_binary_reader_unicode_code(
    code: &[u8],
    tokens: &BinaryReaderUnicodeTokens,
) -> Result<PatchedBinaryReaderUnicodeCode, String> {
    let instructions = parse_il_instructions(code)?;
    let unicode_token = tokens
        .get_unicode
        .iter()
        .next()
        .copied()
        .ok_or_else(|| "Encoding.get_Unicode token set is empty".to_string())?;
    let mut insertions = BTreeMap::<usize, Vec<u8>>::new();
    let mut replacements = BTreeMap::<usize, u32>::new();
    let mut sites = 0usize;
    let mut index = 0usize;

    while index + 4 < instructions.len() {
        let Some(read_bytes_token) = instruction_call_token(&instructions[index], code) else {
            index += 1;
            continue;
        };
        if !tokens.read_bytes.contains(&read_bytes_token) {
            index += 1;
            continue;
        }
        let Some(has_multiplier) =
            binary_reader_readbytes_has_length_operand(&instructions, code, index)
        else {
            index += 1;
            continue;
        };
        let Some(bytes_local) = store_local_index(&instructions[index + 1], code) else {
            index += 1;
            continue;
        };
        let Some(default_token) = instruction_call_token(&instructions[index + 2], code) else {
            index += 1;
            continue;
        };
        if !tokens.get_default.contains(&default_token) {
            index += 1;
            continue;
        }
        if load_local_index(&instructions[index + 3], code) != Some(bytes_local) {
            index += 1;
            continue;
        }
        let Some(get_string_token) = instruction_call_token(&instructions[index + 4], code) else {
            index += 1;
            continue;
        };
        if !tokens.get_string.contains(&get_string_token) {
            index += 1;
            continue;
        }

        if !has_multiplier {
            insertions.insert(instructions[index].offset, vec![0x18, 0x5a]);
        }
        replacements.insert(instructions[index + 2].operand_offset, unicode_token);
        sites += 1;
        index += 5;
    }

    if sites == 0 {
        return Ok(PatchedBinaryReaderUnicodeCode {
            code: code.to_vec(),
            sites: 0,
        });
    }

    Ok(PatchedBinaryReaderUnicodeCode {
        code: rewrite_il_code(code, &instructions, &insertions, &replacements)?,
        sites,
    })
}

pub(super) fn patch_assembly_by_entries(
    assembly_path: &Path,
    entries: &[JsonValue],
    options: &ManagedApplyOptions,
) -> Result<usize, String> {
    if !assembly_path.is_file() {
        if options.allow_missing {
            return Ok(0);
        }
        return Err(format!(
            "Assembly was not found: {}",
            assembly_path.display()
        ));
    }

    let mut context = AssemblyContext::from_path(assembly_path)?;
    let mut pending = entries
        .iter()
        .map(|entry| (entry_apply_key(entry), false))
        .collect::<BTreeMap<_, _>>();
    let instructions = scan_ldstrs(&context)?;
    let mut user_strings = UserStringAppender::from_metadata(&context.metadata);
    let mut patched = 0usize;

    for instruction in instructions {
        for entry in entries {
            let key = entry_apply_key(entry);
            if pending.get(&key).copied().unwrap_or(false) {
                continue;
            }
            if !same_instruction(entry, &instruction) {
                continue;
            }
            let translation = entry
                .get("translation")
                .and_then(JsonValue::as_str)
                .unwrap_or("");
            let translation =
                repair_cp1251_mojibake(translation).unwrap_or_else(|| translation.to_string());
            let user_string_offset = user_strings.add(&translation)?;
            let token = 0x7000_0000u32 | user_string_offset;
            context
                .pe
                .write_at_rva(instruction.operand_rva, &token.to_le_bytes())
                .ok_or_else(|| {
                    format!(
                        "{}: could not patch ldstr operand at RVA 0x{:x}",
                        assembly_path.display(),
                        instruction.operand_rva
                    )
                })?;
            pending.insert(key, true);
            println!(
                "{}: {} -> {}",
                entry_file(entry),
                instruction.source,
                translation
            );
            patched += 1;
            break;
        }
    }

    let missing = pending
        .iter()
        .filter(|(_, applied)| !**applied)
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    if !missing.is_empty() && !options.allow_missing {
        return Err(format!(
            "{}: {} translated entries were not found. First missing: {}",
            assembly_path.display(),
            missing.len(),
            missing.into_iter().take(10).collect::<Vec<_>>().join("; ")
        ));
    }
    let bypassed = if has_panel_equip_translations(entries) {
        bypass_panel_equip_prelocalized_getstr(&mut context)?
    } else {
        0
    };
    if bypassed > 0 {
        println!(
            "{}: bypassed {bypassed} Panel_Equip TextManager.GetStr call(s) for prelocalized labels",
            assembly_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("assembly")
        );
    }
    if patched == 0 && bypassed == 0 {
        return Ok(0);
    }

    if options.backup {
        let backup_path = assembly_path.with_extension(format!(
            "{}.bak",
            assembly_path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
        ));
        if !backup_path.exists() {
            fs::copy(assembly_path, &backup_path)
                .map_err(|err| format!("{}: {err}", backup_path.display()))?;
        }
    }

    if patched > 0 {
        embed_metadata(&mut context, user_strings.into_bytes())?;
    }
    write_pe_preserve_layout(&mut context.pe, assembly_path)?;
    if patched > 0 {
        let reparsed = AssemblyContext::from_path(assembly_path)?;
        let _ = reparsed.metadata.version();
    }
    Ok(patched)
}

pub(super) fn patch_resource_locator_character_bundles_in_context(
    context: &mut AssemblyContext,
    bundle_names: &[String],
) -> Result<usize, String> {
    let Some(method_index) = context.method_names.iter().position(|method_name| {
        method_name
            == "ResourceLocator::LocateWWWResource(System.String,System.String&,System.String&)"
    }) else {
        return Err("ResourceLocator::LocateWWWResource() was not found".to_string());
    };
    let method = context
        .metadata
        .method_defs
        .get(method_index.saturating_sub(1))
        .ok_or_else(|| {
            "ResourceLocator::LocateWWWResource() metadata row was not found".to_string()
        })?;
    if method.rva == 0 {
        return Err("ResourceLocator::LocateWWWResource() has no method body".to_string());
    }
    let body = read_method_body(&context.pe, method.rva)?;
    let existing = resource_locator_character_names(context, &body)?;
    // Layout packs (NPC_Pack_*, Nano_Pack_*, shared audio packs, and future
    // category packs) use the same early AssetBundle.Contains lookup as the
    // original per-character bundles. Restricting this to Character_* made a
    // consolidated layout impossible even though AssetLoader could download it.
    let additions = requested_asset_loader_downloads(bundle_names, &existing);
    if additions.is_empty() {
        return Ok(0);
    }

    let template = resource_locator_character_template(context, &body)?;
    let insert_at = resource_locator_character_insert_offset(&body)?;
    let mut user_strings = UserStringAppender::from_metadata(&context.metadata);
    let mut inserted = Vec::new();
    for bundle_name in &additions {
        let name_offset = user_strings.add(bundle_name)?;
        inserted.extend(resource_locator_character_block(
            &template,
            0x7000_0000u32 | name_offset,
        ));
    }

    let mut new_code = Vec::with_capacity(body.code.len() + inserted.len());
    new_code.extend_from_slice(&body.code[..insert_at]);
    new_code.extend_from_slice(&inserted);
    new_code.extend_from_slice(&body.code[insert_at..]);

    let new_body = rebuild_method_body_with_code(&body, &new_code)?;
    let new_body_rva = append_managed_patch_section(&mut context.pe, new_body)?;
    let (tables_stream_name, tables) =
        method_body_rva_replacement_tables(context, method_index as u32, method.rva, new_body_rva)?;
    let mut replacements = BTreeMap::new();
    replacements.insert(
        StreamHeader::USER_STRINGS.to_string(),
        user_strings.into_bytes(),
    );
    replacements.insert(tables_stream_name, tables);
    embed_metadata_with_replacements(context, &replacements)?;
    Ok(additions.len())
}

pub(super) fn append_managed_patch_section(pe: &mut PE, data: Vec<u8>) -> Result<u32, String> {
    if data.is_empty() {
        return Err("managed patch section cannot be empty".to_string());
    }
    let file_alignment = pe.optional_header.file_alignment().max(1);
    let section_alignment = pe.optional_header.section_alignment().max(1);
    let headers_size = pe.optional_header.size_of_headers() as usize;
    let pe_offset = pe.dos_header.e_lfanew as usize;
    let optional_offset = pe_offset + 4 + portex::coff::CoffHeader::SIZE;
    let sections_offset = optional_offset + pe.coff_header.size_of_optional_header as usize;
    let needed_header_size = sections_offset + (pe.sections.len() + 1) * SectionHeader::SIZE;
    if needed_header_size > headers_size {
        return append_managed_patch_to_last_section(pe, data);
    }

    let virtual_end = pe
        .sections
        .iter()
        .map(|section| {
            section.header.virtual_address
                + align_up(section.header.virtual_size.max(1), section_alignment)
        })
        .max()
        .unwrap_or(headers_size as u32);
    let raw_end = pe
        .sections
        .iter()
        .map(|section| section.header.pointer_to_raw_data + section.header.size_of_raw_data)
        .max()
        .unwrap_or(headers_size as u32);

    let mut section = Section::new(
        ".ffil",
        section_characteristics::CODE
            | section_characteristics::INITIALIZED_DATA
            | section_characteristics::EXECUTE
            | section_characteristics::READ,
    );
    section.header.virtual_address = align_up(virtual_end, section_alignment);
    section.header.pointer_to_raw_data = align_up(raw_end, file_alignment);
    section.header.virtual_size = data.len() as u32;
    section.header.size_of_raw_data = align_up(data.len() as u32, file_alignment);
    section.data = data;
    let rva = section.header.virtual_address;
    pe.add_section(section);
    Ok(rva)
}

pub(super) fn append_managed_patch_to_last_section(pe: &mut PE, data: Vec<u8>) -> Result<u32, String> {
    let section = pe
        .sections
        .last_mut()
        .ok_or_else(|| "PE has no sections".to_string())?;
    let aligned_offset = align_up_usize(section.data.len(), 4);
    if section.data.len() < aligned_offset {
        section.data.resize(aligned_offset, 0);
    }
    let rva = section.header.virtual_address + aligned_offset as u32;
    section.data.extend_from_slice(&data);
    section.header.virtual_size = section.header.virtual_size.max(section.data.len() as u32);
    section.header.characteristics &= !section_characteristics::DISCARDABLE;
    section.header.characteristics |= section_characteristics::CODE
        | section_characteristics::INITIALIZED_DATA
        | section_characteristics::EXECUTE
        | section_characteristics::READ;
    Ok(rva)
}
