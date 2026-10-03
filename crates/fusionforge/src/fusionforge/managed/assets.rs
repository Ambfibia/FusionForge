use super::*;

pub(crate) fn patch_asset_loader_downloads(
    root: &Path,
    bundle_names: &[String],
) -> Result<usize, String> {
    if bundle_names.is_empty() {
        return Ok(0);
    }
    let assembly_path = root.join("Assembly - CSharp - first pass.dll");
    if !assembly_path.is_file() {
        return Err(format!(
            "AssetLoader assembly was not found: {}",
            assembly_path.display()
        ));
    }

    let mut context = AssemblyContext::from_path(&assembly_path)?;
    let patched = patch_asset_loader_downloads_in_context(&mut context, bundle_names)?;
    if patched == 0 {
        return Ok(0);
    }

    write_pe_preserve_layout(&mut context.pe, &assembly_path)?;
    let reparsed = AssemblyContext::from_path(&assembly_path)?;
    let _ = reparsed.metadata.version();
    println!(
        "{}: added {patched} standalone NPC bundle(s) to AssetLoader download list",
        assembly_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("assembly")
    );
    Ok(patched)
}

pub(super) fn load_local_index(instruction: &IlInstruction, code: &[u8]) -> Option<u16> {
    match instruction.opcode {
        IlOpcode::Single(0x06) => Some(0),
        IlOpcode::Single(0x07) => Some(1),
        IlOpcode::Single(0x08) => Some(2),
        IlOpcode::Single(0x09) => Some(3),
        IlOpcode::Single(0x11) => code
            .get(instruction.operand_offset)
            .map(|value| *value as u16),
        IlOpcode::Extended(0x0c) => read_u16(code, instruction.operand_offset).ok(),
        _ => None,
    }
}

pub(super) fn store_local_index(instruction: &IlInstruction, code: &[u8]) -> Option<u16> {
    match instruction.opcode {
        IlOpcode::Single(0x0a) => Some(0),
        IlOpcode::Single(0x0b) => Some(1),
        IlOpcode::Single(0x0c) => Some(2),
        IlOpcode::Single(0x0d) => Some(3),
        IlOpcode::Single(0x13) => code
            .get(instruction.operand_offset)
            .map(|value| *value as u16),
        IlOpcode::Extended(0x0e) => read_u16(code, instruction.operand_offset).ok(),
        _ => None,
    }
}

pub(super) fn patch_asset_loader_downloads_in_context(
    context: &mut AssemblyContext,
    bundle_names: &[String],
) -> Result<usize, String> {
    let Some(method_index) = context
        .method_names
        .iter()
        .position(|method_name| method_name == "AssetLoader::Awake()")
    else {
        return Err("AssetLoader::Awake() was not found".to_string());
    };
    let method = context
        .metadata
        .method_defs
        .get(method_index.saturating_sub(1))
        .ok_or_else(|| "AssetLoader::Awake() metadata row was not found".to_string())?;
    if method.rva == 0 {
        return Err("AssetLoader::Awake() has no method body".to_string());
    }
    let body = read_method_body(&context.pe, method.rva)?;
    let existing = asset_loader_download_names(context, &body)?;
    let additions = requested_asset_loader_downloads(bundle_names, &existing);
    if additions.is_empty() {
        return Ok(0);
    }

    let template = asset_loader_download_add_template(context, &body)?;
    let mut user_strings = UserStringAppender::from_metadata(&context.metadata);
    let mut new_code = body
        .code
        .strip_suffix(&[0x2a])
        .ok_or_else(|| "AssetLoader::Awake() does not end with ret".to_string())?
        .to_vec();
    for bundle_name in &additions {
        let name_offset = user_strings.add(bundle_name)?;
        let description = asset_loader_bundle_description(bundle_name);
        let description_offset = user_strings.add(&description)?;
        new_code.extend(asset_loader_download_add_block(
            &template,
            0x7000_0000u32 | name_offset,
            0x7000_0000u32 | description_offset,
        ));
    }
    new_code.push(0x2a);

    let new_body = rebuild_method_body_with_code(&body, &new_code)?;
    let new_body_rva = append_managed_patch_section(&mut context.pe, new_body)?;

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
        method_def_row_offset_in_tables(&context.metadata, &tables, method_index as u32)?;
    let current_rva = read_u32(&tables, method_row_offset)?;
    if current_rva != method.rva {
        return Err(format!(
            "AssetLoader::Awake() MethodDef row mismatch: expected RVA 0x{:x}, found 0x{current_rva:x}",
            method.rva
        ));
    }
    tables
        .get_mut(method_row_offset..method_row_offset + 4)
        .ok_or_else(|| "AssetLoader::Awake() MethodDef RVA is out of range".to_string())?
        .copy_from_slice(&new_body_rva.to_le_bytes());

    let mut replacements = BTreeMap::new();
    replacements.insert(
        StreamHeader::USER_STRINGS.to_string(),
        user_strings.into_bytes(),
    );
    replacements.insert(tables_stream_name, tables);
    embed_metadata_with_replacements(context, &replacements)?;
    Ok(additions.len())
}

#[derive(Debug, Clone)]
pub(super) struct AssetLoaderAddTemplate {
    pub(super) ldfld_token: [u8; 4],
    pub(super) castclass_token: [u8; 4],
    pub(super) add_token: [u8; 4],
}

pub(super) fn asset_loader_download_names(
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
        if lower.ends_with(".resourcefile") || lower.ends_with(".unity3d") {
            result.insert(lower);
        }
    }
    Ok(result)
}

pub(super) fn requested_asset_loader_downloads(
    bundle_names: &[String],
    existing: &BTreeSet<String>,
) -> Vec<String> {
    // The caller supplies dependency/load order. Do not sort this list: when two
    // packs contain the same legacy path, ResourceLocator must inspect the
    // canonical owner first, and dependencies must be loaded before consumers.
    let mut seen = existing.clone();
    let mut requested = Vec::new();
    for name in bundle_names {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if !(lower.ends_with(".resourcefile") || lower.ends_with(".unity3d")) {
            continue;
        }
        if !seen.insert(lower) {
            continue;
        }
        requested.push(trimmed.to_string());
    }
    requested
}

pub(super) fn asset_loader_download_add_template(
    context: &AssemblyContext,
    body: &MethodBody,
) -> Result<AssetLoaderAddTemplate, String> {
    for (il_offset, token) in scan_ldstr_tokens(&body.code)? {
        let user_string_offset = token & 0x00ff_ffff;
        let Ok(source) = context.metadata.user_strings.get(user_string_offset) else {
            continue;
        };
        let lower = source.to_ascii_lowercase();
        if !(lower.ends_with(".resourcefile") || lower.ends_with(".unity3d")) {
            continue;
        }
        let Some(start) = il_offset.checked_sub(11) else {
            continue;
        };
        let Some(block) = body.code.get(start..start + 26) else {
            continue;
        };
        if block[0] == 0x02
            && block[1] == 0x7b
            && block[6] == 0x74
            && block[11] == 0x72
            && block[16] == 0x72
            && block[21] == 0x6f
        {
            return Ok(AssetLoaderAddTemplate {
                ldfld_token: [block[2], block[3], block[4], block[5]],
                castclass_token: [block[7], block[8], block[9], block[10]],
                add_token: [block[22], block[23], block[24], block[25]],
            });
        }
    }
    Err("Could not find AssetLoader dicDownloadFilename.Add IL template".to_string())
}

pub(super) fn asset_loader_download_add_block(
    template: &AssetLoaderAddTemplate,
    name_token: u32,
    description_token: u32,
) -> Vec<u8> {
    let mut block = Vec::with_capacity(26);
    block.push(0x02); // ldarg.0
    block.push(0x7b); // ldfld AssetLoader::dicDownloadFilename
    block.extend_from_slice(&template.ldfld_token);
    block.push(0x74); // castclass Dictionary<string,string>
    block.extend_from_slice(&template.castclass_token);
    block.push(0x72); // ldstr bundle name
    block.extend_from_slice(&name_token.to_le_bytes());
    block.push(0x72); // ldstr human-readable download label
    block.extend_from_slice(&description_token.to_le_bytes());
    block.push(0x6f); // callvirt Dictionary<string,string>::Add
    block.extend_from_slice(&template.add_token);
    block
}

pub(super) fn asset_loader_bundle_description(bundle_name: &str) -> String {
    let stem = bundle_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(bundle_name);
    let lower = stem.to_ascii_lowercase();
    let category = if lower.starts_with("coreshared") {
        "Shared Core Data"
    } else if lower.starts_with("tutorialaudio") {
        "Tutorial Audio"
    } else if lower.starts_with("uiaudio") {
        "Interface Audio"
    } else if lower.starts_with("npcvoiceshared") {
        "NPC Voice Data"
    } else if lower.starts_with("worldshared") {
        "Shared World Data"
    } else if lower.starts_with("nano_pack") {
        "Nano Data"
    } else if lower.starts_with("hnpc_pack") {
        "HNPC Character Data"
    } else if lower.starts_with("playercharacter_pack") {
        "Player Character Data"
    } else if lower.starts_with("items_pack") {
        "Item Data"
    } else if lower.starts_with("icons_pack") {
        "Icon Data"
    } else {
        "NPC Character Data"
    };
    let detail = stem.replace('_', " ");
    format!("{category} - {detail}")
}
