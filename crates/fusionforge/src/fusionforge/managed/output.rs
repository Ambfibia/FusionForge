use super::*;

#[derive(Debug, Clone)]
pub(crate) struct ManagedExportOptions {
    pub assemblies: Vec<String>,
    pub all_assemblies: bool,
    pub ui_only: bool,
    pub min_length: usize,
    pub include_empty: bool,
    pub include_control_chars: bool,
    pub merge_paths: Vec<PathBuf>,
}

impl Default for ManagedExportOptions {
    fn default() -> Self {
        Self {
            assemblies: Vec::new(),
            all_assemblies: false,
            ui_only: false,
            min_length: 1,
            include_empty: false,
            include_control_chars: false,
            merge_paths: Vec::new(),
        }
    }
}

pub(super) fn export_assembly(
    root: &Path,
    assembly_path: &Path,
    options: &ManagedExportOptions,
    merge: &TranslationMerge,
) -> Result<Vec<JsonValue>, String> {
    let relative_file = assembly_path
        .strip_prefix(root)
        .unwrap_or(assembly_path)
        .to_string_lossy()
        .replace('\\', "/");
    let context = AssemblyContext::from_path(assembly_path)?;
    let mut entries = Vec::new();
    for instruction in scan_ldstrs(&context)? {
        if !should_export(
            &instruction.source,
            &instruction.type_name,
            &instruction.method_name,
            options,
        ) {
            continue;
        }
        let id = build_entry_id(
            &relative_file,
            &instruction.type_name,
            &instruction.method_name,
            instruction.il_offset,
            instruction.occurrence,
            &instruction.source,
        );
        let mut entry = json!({
            "file": relative_file,
            "id": id,
            "ilOffset": instruction.il_offset,
            "kind": MANAGED_KIND,
            "method": instruction.method_name,
            "occurrence": instruction.occurrence,
            "source": instruction.source,
            "sourceSha1": sha1_hex(entry_source_for_hash(&instruction.source))[..16].to_string(),
            "translation": "",
            "type": instruction.type_name,
        });
        if let Some(translation) = merge.find(&entry) {
            entry["translation"] =
                json!(repair_cp1251_mojibake(&translation).unwrap_or(translation));
        }
        entries.push(entry);
    }
    Ok(entries)
}

pub(super) fn write_compressed_uint(output: &mut Vec<u8>, value: u32) {
    if value < 0x80 {
        output.push(value as u8);
    } else if value < 0x4000 {
        output.push((0x80 | (value >> 8)) as u8);
        output.push(value as u8);
    } else {
        output.push((0xc0 | (value >> 24)) as u8);
        output.push((value >> 16) as u8);
        output.push((value >> 8) as u8);
        output.push(value as u8);
    }
}

pub(super) fn should_export(
    source: &str,
    type_name: &str,
    method_name: &str,
    options: &ManagedExportOptions,
) -> bool {
    if source.len() < options.min_length {
        return false;
    }
    if !options.include_empty && source.trim().is_empty() {
        return false;
    }
    if source.contains('\0') {
        return false;
    }
    if !options.include_control_chars
        && source
            .chars()
            .any(|ch| ch.is_control() && ch != '\r' && ch != '\n' && ch != '\t')
    {
        return false;
    }
    if !contains_only_latin_or_cyrillic_letters(source) {
        return false;
    }
    if options.ui_only && !is_ui_string(type_name, method_name, source) {
        return false;
    }
    true
}

pub(super) fn write_cli_metadata_pointer(
    pe: &mut PE,
    cli_rva: u32,
    metadata_rva: u32,
    metadata_size: u32,
) -> Result<(), String> {
    pe.write_at_rva(cli_rva + 8, &metadata_rva.to_le_bytes())
        .ok_or_else(|| "could not patch CLI metadata RVA".to_string())?;
    pe.write_at_rva(cli_rva + 12, &metadata_size.to_le_bytes())
        .ok_or_else(|| "could not patch CLI metadata size".to_string())?;
    Ok(())
}

pub(super) fn write_pe_preserve_layout(pe: &mut PE, path: &Path) -> Result<(), String> {
    let file_alignment = pe.optional_header.file_alignment().max(1);
    let section_alignment = pe.optional_header.section_alignment().max(1);
    for section in &mut pe.sections {
        section.header.virtual_size = section.header.virtual_size.max(section.data.len() as u32);
        section.header.size_of_raw_data = align_up(section.data.len() as u32, file_alignment);
    }
    let size_of_image = pe
        .sections
        .iter()
        .map(|section| {
            section.header.virtual_address
                + align_up(section.header.virtual_size.max(1), section_alignment)
        })
        .max()
        .unwrap_or_else(|| pe.optional_header.size_of_image());
    match &mut pe.optional_header {
        OptionalHeader::Pe32(header) => header.size_of_image = size_of_image,
        OptionalHeader::Pe32Plus(header) => header.size_of_image = size_of_image,
    }

    let headers_size = pe.optional_header.size_of_headers() as usize;
    let mut file_size = headers_size;
    for section in &pe.sections {
        if section.header.pointer_to_raw_data > 0 {
            let end = section.header.pointer_to_raw_data as usize
                + section.header.size_of_raw_data as usize;
            file_size = file_size.max(end);
        }
    }
    let mut output = vec![0u8; file_size];

    pe.dos_header
        .write(&mut output[0..portex::dos::DosHeader::SIZE])
        .map_err(|err| err.to_string())?;
    let stub_start = portex::dos::DosHeader::SIZE;
    let stub_end = stub_start + pe.dos_stub.len();
    if stub_end <= output.len() {
        output[stub_start..stub_end].copy_from_slice(&pe.dos_stub);
    }
    let pe_offset = pe.dos_header.e_lfanew as usize;
    output
        .get_mut(pe_offset..pe_offset + 4)
        .ok_or_else(|| "PE signature offset is out of range".to_string())?
        .copy_from_slice(&0x0000_4550u32.to_le_bytes());
    let coff_offset = pe_offset + 4;
    pe.coff_header
        .write(&mut output[coff_offset..coff_offset + portex::coff::CoffHeader::SIZE])
        .map_err(|err| err.to_string())?;
    let optional_offset = coff_offset + portex::coff::CoffHeader::SIZE;
    let optional_size = pe.optional_header.size();
    pe.optional_header
        .write(&mut output[optional_offset..optional_offset + optional_size])
        .map_err(|err| err.to_string())?;
    let sections_offset = optional_offset + pe.coff_header.size_of_optional_header as usize;
    for (index, section) in pe.sections.iter().enumerate() {
        let offset = sections_offset + index * portex::section::SectionHeader::SIZE;
        section
            .header
            .write(&mut output[offset..offset + portex::section::SectionHeader::SIZE])
            .map_err(|err| err.to_string())?;
    }
    for section in &pe.sections {
        let start = section.header.pointer_to_raw_data as usize;
        let raw_size = section.header.size_of_raw_data as usize;
        if start == 0 || raw_size == 0 {
            continue;
        }
        let end = start + raw_size;
        let data_len = section.data.len().min(raw_size);
        output[start..start + data_len].copy_from_slice(&section.data[..data_len]);
        if start + data_len < end {
            output[start + data_len..end].fill(0);
        }
    }
    fs::write(path, output).map_err(|err| format!("{}: {err}", path.display()))
}
