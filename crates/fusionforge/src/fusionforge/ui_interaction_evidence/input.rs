use super::*;

pub(super) fn load_managed_authority(
    input: &ManagedAssemblyInput,
) -> Result<(ManagedAuthority, Vec<u8>), String> {
    let report_bytes = fs::read(&input.evidence_report)
        .map_err(|error| format!("{}: {error}", input.evidence_report.display()))?;
    let report: ManagedEvidenceDocument = serde_json::from_slice(&report_bytes)
        .map_err(|error| format!("{}: {error}", input.evidence_report.display()))?;
    if report.schema != MANAGED_EVIDENCE_SCHEMA || report.schema_version != 1 {
        return Err(format!(
            "managed evidence must be {MANAGED_EVIDENCE_SCHEMA} with schemaVersion 1"
        ));
    }
    let payload = if let Some(path) = &input.payload {
        fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?
    } else if report.materialization_is_embedded()? {
        BASE64_STANDARD
            .decode(
                report
                    .payload
                    .materialization
                    .base64
                    .as_deref()
                    .ok_or_else(|| {
                        "embedded-base64 materialization has no base64 payload".to_string()
                    })?,
            )
            .map_err(|error| format!("invalid embedded managed payload base64: {error}"))?
    } else {
        return Err(
            "portable managed evidence requires explicit request managed.payload bytes".to_string(),
        );
    };
    require_bytes_and_hash(
        "managed payload",
        &payload,
        report.payload.bytes,
        &report.payload.sha256,
    )?;
    validate_sha256("raw container", &report.source.raw_container.sha256)?;
    validate_sha256("CLR metadata", &report.managed_format.clr_metadata_sha256)?;
    if report.source.raw_container.bytes == 0 {
        return Err("managed evidence raw container byte count may not be zero".to_string());
    }
    let authority = ManagedAuthority {
        evidence_report_bytes: report_bytes.len() as u64,
        evidence_report_sha256: sha256_hex(&report_bytes),
        source_alias: report.source.alias,
        raw_container_relative_path: report.source.raw_container.relative_path,
        raw_container_sha256: normalize_sha256(&report.source.raw_container.sha256)?,
        selected_level: report.selection.level,
        selected_exact_entry: report.selection.exact_entry,
        selected_route: report.selection.route,
        payload_bytes: report.payload.bytes,
        payload_sha256: normalize_sha256(&report.payload.sha256)?,
        clr_metadata_bytes: report.managed_format.clr_metadata_bytes,
        clr_metadata_sha256: normalize_sha256(&report.managed_format.clr_metadata_sha256)?,
    };
    Ok((authority, payload))
}

pub(super) fn read_method_body(pe: &PE, rva: u32) -> Result<DecodedMethodBody, String> {
    let first = *pe
        .read_at_rva(rva, 1)
        .and_then(|bytes| bytes.first())
        .ok_or_else(|| format!("method body RVA 0x{rva:X} is out of range"))?;
    let (header_size, code_size, has_extra_sections) = if first & 0x3 == 0x2 {
        (1usize, (first >> 2) as usize, false)
    } else if first & 0x3 == 0x3 {
        let header = pe
            .read_at_rva(rva, 12)
            .ok_or_else(|| format!("fat method body RVA 0x{rva:X} is out of range"))?;
        let flags_size = u16::from_le_bytes([header[0], header[1]]);
        let header_size = ((flags_size >> 12) as usize) * 4;
        if header_size < 12 {
            return Err(format!(
                "fat method body RVA 0x{rva:X} has invalid header size"
            ));
        }
        (
            header_size,
            read_u32_le(header, 4)? as usize,
            flags_size & 0x0008 != 0,
        )
    } else {
        return Err(format!("unsupported method body header at RVA 0x{rva:X}"));
    };
    let code_rva = rva
        .checked_add(header_size as u32)
        .ok_or_else(|| "method body RVA overflowed".to_string())?;
    let code = pe
        .read_at_rva(code_rva, code_size)
        .ok_or_else(|| format!("method code RVA 0x{code_rva:X} size {code_size} is out of range"))?
        .to_vec();
    Ok(DecodedMethodBody {
        code,
        has_extra_sections,
    })
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
        metadata_rva: read_u32_le(header, 8)?,
        metadata_size: read_u32_le(header, 12)?,
    })
}

pub(super) fn resolve_exact_method(
    context: &AssemblyContext,
    selector: &ManagedMethodSelector,
) -> Result<(u32, ExactManagedMethod), String> {
    let type_rows = context
        .type_names
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(row, name)| (name == &selector.declaring_type).then_some(row as u32))
        .collect::<Vec<_>>();
    let type_row = match type_rows.as_slice() {
        [only] => *only,
        [] => {
            return Err(format!(
                "exact managed type '{}' was not found",
                selector.declaring_type
            ));
        }
        _ => {
            return Err(format!(
                "exact managed type '{}' is ambiguous",
                selector.declaring_type
            ));
        }
    };
    let mut matches = Vec::new();
    for (method_row, method) in context.metadata.get_type_methods(type_row) {
        let name = context
            .metadata
            .strings
            .get(method.name)
            .map_err(|error| format!("invalid MethodDef name heap index: {error}"))?;
        if name != selector.name {
            continue;
        }
        let Some((return_type, parameter_types)) = method_signature(&context.metadata, method)
        else {
            continue;
        };
        if parameter_types != selector.parameter_types {
            continue;
        }
        if selector
            .return_type
            .as_ref()
            .is_some_and(|expected| expected != &return_type)
        {
            continue;
        }
        matches.push((
            method_row,
            ExactManagedMethod {
                declaring_type: selector.declaring_type.clone(),
                type_token: metadata_token(0x02, type_row),
                name: selector.name.clone(),
                parameter_types,
                return_type,
                method_token: metadata_token(0x06, method_row),
                rva: method.rva,
            },
        ));
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(format!(
            "exact managed method {}::{}({}) was not found",
            selector.declaring_type,
            selector.name,
            selector.parameter_types.join(",")
        )),
        count => Err(format!(
            "exact managed method {}::{}({}) matched {count} rows; add returnType",
            selector.declaring_type,
            selector.name,
            selector.parameter_types.join(",")
        )),
    }
}

pub(super) fn resolve_method_token(context: &AssemblyContext, token: u32) -> Result<ResolvedMethod, String> {
    let table = (token >> 24) as u8;
    let row = token & 0x00ff_ffff;
    if row == 0 {
        return Err("metadata token has row zero".to_string());
    }
    match table {
        0x06 => {
            let _method = context
                .metadata
                .method_defs
                .get(row as usize - 1)
                .ok_or_else(|| format!("MethodDef row {row} is out of range"))?;
            let exact = exact_method_for_row(context, row)?;
            Ok(ResolvedMethod {
                identity: ManagedMemberIdentity {
                    declaring_type: exact.declaring_type,
                    name: exact.name,
                    parameter_types: exact.parameter_types,
                    return_type: exact.return_type,
                    metadata_token: metadata_token(0x06, row),
                },
                method_def_row: Some(row),
            })
        }
        0x0a => {
            let member = context
                .metadata
                .member_refs
                .get(row as usize - 1)
                .ok_or_else(|| format!("MemberRef row {row} is out of range"))?;
            let declaring_type =
                member_ref_owner_full_name(context, &member.class).ok_or_else(|| {
                    "MemberRef owner is not an exactly resolvable TypeRef/TypeDef".to_string()
                })?;
            let name = context
                .metadata
                .strings
                .get(member.name)
                .map_err(|error| format!("invalid MemberRef name heap index: {error}"))?
                .to_string();
            let signature = context
                .metadata
                .blobs
                .get(member.signature)
                .map_err(|error| format!("invalid MemberRef signature heap index: {error}"))
                .and_then(|blob| {
                    MethodSig::parse_blob(blob)
                        .map_err(|error| format!("MemberRef is not a method signature: {error}"))
                })?;
            Ok(ResolvedMethod {
                identity: ManagedMemberIdentity {
                    declaring_type,
                    name,
                    parameter_types: signature
                        .params
                        .iter()
                        .map(|value| format_type_sig(&context.metadata, value))
                        .collect(),
                    return_type: format_type_sig(&context.metadata, &signature.return_type),
                    metadata_token: metadata_token(0x0a, row),
                },
                method_def_row: None,
            })
        }
        0x2b => Err("MethodSpec targets are not decoded by the bounded v1 core".to_string()),
        _ => Err(format!("token 0x{token:08X} is not MethodDef/MemberRef")),
    }
}

pub(super) fn resolve_field_token(
    context: &AssemblyContext,
    token: u32,
) -> Result<ManagedFieldIdentity, String> {
    let table = (token >> 24) as u8;
    let row = token & 0x00ff_ffff;
    if row == 0 {
        return Err("metadata token has row zero".to_string());
    }
    match table {
        0x04 => {
            let field = context
                .metadata
                .fields
                .get(row as usize - 1)
                .ok_or_else(|| format!("Field row {row} is out of range"))?;
            let type_row = *context
                .field_owners
                .get(row as usize)
                .ok_or_else(|| format!("Field row {row} has no declaring type"))?;
            let declaring_type = context
                .type_names
                .get(type_row as usize)
                .cloned()
                .ok_or_else(|| format!("Field row {row} has no declaring type name"))?;
            let name = context
                .metadata
                .strings
                .get(field.name)
                .map_err(|error| format!("invalid Field name heap index: {error}"))?
                .to_string();
            Ok(ManagedFieldIdentity {
                declaring_type,
                name,
                metadata_token: metadata_token(0x04, row),
            })
        }
        0x0a => {
            let member = context
                .metadata
                .member_refs
                .get(row as usize - 1)
                .ok_or_else(|| format!("MemberRef row {row} is out of range"))?;
            let declaring_type =
                member_ref_owner_full_name(context, &member.class).ok_or_else(|| {
                    "MemberRef field owner is not an exact TypeRef/TypeDef".to_string()
                })?;
            let name = context
                .metadata
                .strings
                .get(member.name)
                .map_err(|error| format!("invalid MemberRef name heap index: {error}"))?
                .to_string();
            Ok(ManagedFieldIdentity {
                declaring_type,
                name,
                metadata_token: metadata_token(0x0a, row),
            })
        }
        _ => Err(format!("token 0x{token:08X} is not Field/MemberRef")),
    }
}

pub(super) fn parse_metadata_token_row(token: &str) -> u32 {
    u32::from_str_radix(token.trim_start_matches("0x"), 16).unwrap_or(u32::MAX)
}

pub(super) fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("truncated four-byte value at offset {offset}"))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

pub(super) fn resolve_portable_input(root: &Path, relative: &Path, label: &str) -> Result<PathBuf, String> {
    let parts = portable_relative_parts(relative, label)?;
    let candidate = root.join(PathBuf::from_iter(parts));
    let canonical = fs::canonicalize(&candidate)
        .map_err(|error| format!("{}: {error}", candidate.display()))?;
    let canonical_root =
        fs::canonicalize(root).map_err(|error| format!("{}: {error}", root.display()))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(format!("{label} resolves outside the request directory"));
    }
    if !canonical.is_file() {
        return Err(format!("{label} is not a file: {}", canonical.display()));
    }
    Ok(canonical)
}
