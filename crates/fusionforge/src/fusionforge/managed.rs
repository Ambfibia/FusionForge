use std::{
    collections::{hash_map::Entry, BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

use clrmeta::{ElementType, Metadata, MethodDefRow, MethodSig, StreamHeader, TableId, TypeSig};
use portex::section::{characteristics as section_characteristics, Section, SectionHeader};
use portex::{data_dir::DataDirectoryType, optional::OptionalHeader, PE};
use serde_json::{json, Value as JsonValue};
use sha1::{Digest, Sha1};

#[cfg(test)]
mod tests;

mod constants;
mod localization;
mod output;
mod systems;
mod types;
mod assets;
mod projects;
mod audio;
mod operations_adjust_branch_operand;
mod operations_is_technical_source;
mod operations_rebuild_metadata_with_replacements;
mod input;
mod codec;

pub(crate) use constants::MANAGED_KIND;
use constants::DEFAULT_ASSEMBLIES;
pub(crate) use localization::{MANAGED_TRANSLATION_GENERATOR, export_translation_index};
use localization::TranslationMerge;
pub(crate) use output::ManagedExportOptions;
use output::{
    export_assembly, write_compressed_uint, write_cli_metadata_pointer,
    write_pe_preserve_layout
};
pub(crate) use systems::{ManagedApplyOptions, apply_translations};
use systems::entry_apply_key;
use types::{
    CliHeader, AssemblyContext, LdstrInstruction, MethodBody, UserStringAppender,
    BinaryReaderUnicodeTokens, PatchedBinaryReaderUnicodeCode, PatchedManagedMethod, IlOpcode,
    IlInstruction, ResourceLocatorCharacterTemplate
};
pub(crate) use assets::patch_asset_loader_downloads;
use assets::{
    load_local_index, store_local_index, requested_asset_loader_downloads
};
pub(crate) use projects::{
    patch_resource_locator_character_bundles, patch_binary_reader_unicode_strings
};
use projects::{
    patch_assembly_by_entries,
    append_managed_patch_section
};
pub(crate) use operations_adjust_branch_operand::{
    managed_assembly_name, repair_cp1251_mojibake
};
use operations_adjust_branch_operand::{
    binary_reader_unicode_tokens, binary_reader_readbytes_has_length_operand, rewrite_il_code, switch_operand_len, instruction_call_token,
    method_body_has_extra_sections, increase_method_body_max_stack, is_cyrillic, has_panel_equip_translations,
    bypass_panel_equip_prelocalized_getstr, resource_locator_character_names,
    resource_locator_character_insert_offset, resource_locator_character_template,
    resource_locator_character_block, rebuild_method_body_with_code,
    method_body_rva_replacement_tables
};
use operations_is_technical_source::{
    method_def_row_offset_in_tables, method_def_token,
    append_user_string, scan_ldstrs, scan_ldstr_tokens, scan_call_tokens,
    single_byte_operand_len, extended_operand_len, build_type_names,
    build_method_names, format_type_sig,
    contains_only_latin_or_cyrillic_letters, is_ui_string, embed_metadata, embed_metadata_with_replacements
};
use operations_rebuild_metadata_with_replacements::{
    rebuild_metadata_with_replacements, same_instruction, managed_entry_sort_key, entry_file,
    build_entry_id, entry_source_for_hash, sha1_hex, file_source_key, has_text, align_up,
    align_up_usize
};
pub(crate) use operations_rebuild_metadata_with_replacements::inspect_methods;
use input::{
    parse_il_instructions, find_subslice, find_call_after, find_all_bytes, resolve_assemblies,
    read_cli_header, read_method_body, read_json, read_u32, read_i32, read_u16
};
use codec::metadata_stream_payload;
#[cfg(test)]
use assets::asset_loader_bundle_description;
#[cfg(test)]
use projects::patch_binary_reader_unicode_code;
#[cfg(test)]
use audio::patch_tutorial_voice_durations_in_context;
