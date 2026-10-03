use std::{
    env, fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use clrmeta::Metadata;
use ffbuildtool::bundle::AssetBundle;
use portex::{data_dir::DataDirectoryType, PE};
use serde::Serialize;
use sha2::{Digest, Sha256};

const SCHEMA: &str = "fftools.managed-assembly-evidence.v1";
const USAGE: &str = "export-managed-assembly-evidence <source-alias> <source-root> \
     <source-relative-container> <exact-entry> [--level <index>] \
     [--out <report.json|->] [--payload-out <portable-relative-path>]";

#[derive(Debug)]
struct CliOptions {
    source_alias: String,
    source_root: PathBuf,
    relative_container: String,
    exact_entry: String,
    level: Option<usize>,
    report_output: Option<PathBuf>,
    payload_output: Option<PortableOutput>,
}

#[derive(Debug)]
struct PortableOutput {
    path: PathBuf,
    locator: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedAssemblyEvidence {
    schema: &'static str,
    schema_version: u32,
    source: SourceEvidence,
    selection: EntrySelection,
    managed_format: ManagedFormatProof,
    payload: PayloadEvidence,
    reproduction: ReproductionEvidence,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceEvidence {
    alias: String,
    raw_container: RawContainerEvidence,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RawContainerEvidence {
    relative_path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntrySelection {
    container_format: &'static str,
    level: usize,
    exact_entry: String,
    route: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedFormatProof {
    pe_kind: &'static str,
    is_dll: bool,
    clr_header_major: u16,
    clr_header_minor: u16,
    clr_metadata_bytes: u64,
    clr_metadata_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PayloadEvidence {
    bytes: u64,
    sha256: String,
    materialization: PayloadMaterialization,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PayloadMaterialization {
    mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    portable_relative_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base64: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReproductionEvidence {
    extractor: &'static str,
    selector: EntrySelection,
    required_raw_container_sha256: String,
}

struct SelectedEntry<'a> {
    level: usize,
    name: &'a str,
    bytes: &'a [u8],
}

pub(crate) fn export_managed_assembly_evidence_cli(args: &[String]) -> Result<(), String> {
    let options = parse_cli(args)?;
    validate_source_alias(&options.source_alias)?;
    let (container_path, normalized_relative_container) =
        resolve_source_relative_container(&options.source_root, &options.relative_container)?;

    let raw_container = fs::read(&container_path)
        .map_err(|error| format!("{}: {error}", container_path.display()))?;
    let (_header, bundle) = AssetBundle::from_bytes(&raw_container)?;
    let selected = select_exact_entry(bundle.iter_files(), &options.exact_entry, options.level)?;
    let managed_format = validate_managed_assembly(selected.name, selected.bytes)?;
    let payload = selected.bytes.to_vec();

    let materialization = match &options.payload_output {
        Some(output) => PayloadMaterialization {
            mode: "portable-relative-output",
            portable_relative_path: Some(output.locator.clone()),
            base64: None,
        },
        None => PayloadMaterialization {
            mode: "embedded-base64",
            portable_relative_path: None,
            base64: Some(BASE64_STANDARD.encode(&payload)),
        },
    };
    let evidence = build_evidence(
        options.source_alias,
        normalized_relative_container,
        &raw_container,
        selected.level,
        selected.name,
        &payload,
        managed_format,
        materialization,
    );

    preflight_output_targets(
        &container_path,
        options.report_output.as_deref(),
        options
            .payload_output
            .as_ref()
            .map(|output| output.path.as_path()),
    )?;

    if let Some(output) = &options.payload_output {
        fs::write(&output.path, &payload)
            .map_err(|error| format!("{}: {error}", output.path.display()))?;
    }
    write_json(options.report_output.as_deref(), &evidence)
}

fn parse_cli(args: &[String]) -> Result<CliOptions, String> {
    if args.len() < 4 {
        return Err(USAGE.to_string());
    }

    let mut level = None;
    let mut report_output = None;
    let mut payload_output = None;
    let mut index = 4usize;
    while index < args.len() {
        match args[index].as_str() {
            "--level" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--level requires a non-negative index".to_string())?;
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid UnityWeb level index: {value}"))?;
                if level.replace(parsed).is_some() {
                    return Err("--level may be supplied only once".to_string());
                }
                index += 2;
            }
            "--out" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--out requires a JSON path or '-'".to_string())?;
                if report_output.replace(PathBuf::from(value)).is_some() {
                    return Err("--out may be supplied only once".to_string());
                }
                index += 2;
            }
            "--payload-out" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    "--payload-out requires a portable relative file path".to_string()
                })?;
                let parsed = portable_output(value)?;
                if payload_output.replace(parsed).is_some() {
                    return Err("--payload-out may be supplied only once".to_string());
                }
                index += 2;
            }
            option => {
                return Err(format!(
                    "unknown export-managed-assembly-evidence option '{option}'\n{USAGE}"
                ));
            }
        }
    }

    if args[3].is_empty() {
        return Err("exact managed assembly entry may not be empty".to_string());
    }

    Ok(CliOptions {
        source_alias: args[0].clone(),
        source_root: PathBuf::from(&args[1]),
        relative_container: args[2].clone(),
        exact_entry: args[3].clone(),
        level,
        report_output,
        payload_output,
    })
}

fn validate_source_alias(alias: &str) -> Result<(), String> {
    if alias.is_empty()
        || !alias
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
    {
        return Err(
            "source alias must contain only ASCII letters, digits, '.', '_' or '-'".to_string(),
        );
    }
    Ok(())
}

fn normalized_relative_parts(path: &Path, label: &str) -> Result<Vec<String>, String> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(format!("{label} must be a non-empty relative path"));
    }

    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => {
                let part = part
                    .to_str()
                    .ok_or_else(|| format!("{label} must be valid Unicode"))?;
                parts.push(part.to_string());
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "{label} may not contain a root or parent traversal"
                ));
            }
        }
    }
    if parts.is_empty() {
        return Err(format!("{label} resolves to an empty path"));
    }
    Ok(parts)
}

fn portable_relative_parts(value: &str, label: &str) -> Result<Vec<String>, String> {
    let normalized = value.replace('\\', "/");
    let bytes = normalized.as_bytes();
    let has_windows_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if has_windows_drive || normalized.starts_with("//") {
        return Err(format!("{label} must be a non-empty relative path"));
    }
    normalized_relative_parts(Path::new(&normalized), label)
}

fn resolve_source_relative_container(
    source_root: &Path,
    relative_container: &str,
) -> Result<(PathBuf, String), String> {
    let parts = portable_relative_parts(relative_container, "source-relative container")?;
    let canonical_root = fs::canonicalize(source_root)
        .map_err(|error| format!("{}: {error}", source_root.display()))?;
    if !canonical_root.is_dir() {
        return Err(format!(
            "source root is not a directory: {}",
            source_root.display()
        ));
    }

    let candidate = canonical_root.join(PathBuf::from_iter(&parts));
    let canonical_candidate = fs::canonicalize(&candidate)
        .map_err(|error| format!("{}: {error}", candidate.display()))?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(
            "source-relative container resolves outside its declared source root".to_string(),
        );
    }
    if !canonical_candidate.is_file() {
        return Err(format!(
            "source-relative container is not a file: {}",
            candidate.display()
        ));
    }

    Ok((canonical_candidate, parts.join("/")))
}

fn portable_output(value: &str) -> Result<PortableOutput, String> {
    if value == "-" {
        return Err("--payload-out does not accept '-'".to_string());
    }
    let parts = portable_relative_parts(value, "payload output")?;
    Ok(PortableOutput {
        path: PathBuf::from_iter(&parts),
        locator: parts.join("/"),
    })
}

fn select_exact_entry<'a, I>(
    entries: I,
    requested_name: &str,
    requested_level: Option<usize>,
) -> Result<SelectedEntry<'a>, String>
where
    I: IntoIterator<Item = (usize, &'a str, &'a [u8])>,
{
    let mut matches = entries
        .into_iter()
        .filter(|(level, name, _)| {
            *name == requested_name && requested_level.is_none_or(|expected| expected == *level)
        })
        .collect::<Vec<_>>();

    if matches.is_empty() {
        return Err(match requested_level {
            Some(level) => format!(
                "managed assembly entry '{requested_name}' was not found exactly in UnityWeb level {level}"
            ),
            None => format!(
                "managed assembly entry '{requested_name}' was not found exactly in the raw container"
            ),
        });
    }
    if matches.len() != 1 {
        let routes = matches
            .iter()
            .map(|(level, name, _)| format!("level{level}/{name}"))
            .collect::<Vec<_>>();
        return Err(format!(
            "managed assembly entry '{requested_name}' is ambiguous: {}; add --level with the exact level",
            routes.join(", ")
        ));
    }

    let (level, name, bytes) = matches.remove(0);
    Ok(SelectedEntry { level, name, bytes })
}

fn validate_managed_assembly(
    exact_entry: &str,
    payload: &[u8],
) -> Result<ManagedFormatProof, String> {
    if !exact_entry.to_ascii_lowercase().ends_with(".dll") {
        return Err(format!(
            "exact entry '{exact_entry}' is not a DLL managed assembly route"
        ));
    }

    let pe = PE::parse(payload).map_err(|error| {
        format!("exact entry '{exact_entry}' is not a readable PE DLL: {error}")
    })?;
    if !pe.is_dll() {
        return Err(format!(
            "exact entry '{exact_entry}' is a PE image but is not marked as a DLL"
        ));
    }
    let clr = pe
        .data_directory(DataDirectoryType::ClrRuntime)
        .filter(|directory| directory.is_present())
        .ok_or_else(|| format!("exact entry '{exact_entry}' has no CLR runtime data directory"))?;
    if clr.size < 16 {
        return Err(format!(
            "exact entry '{exact_entry}' has a truncated CLR runtime directory"
        ));
    }
    let cli_header = pe
        .read_at_rva(clr.virtual_address, 16)
        .ok_or_else(|| format!("exact entry '{exact_entry}' has an out-of-range CLR header"))?;
    let header_size = read_u32_le(cli_header, 0)? as usize;
    if header_size < 16 || header_size > clr.size as usize {
        return Err(format!(
            "exact entry '{exact_entry}' has invalid CLR header size {header_size}"
        ));
    }
    let major = read_u16_le(cli_header, 4)?;
    let minor = read_u16_le(cli_header, 6)?;
    let metadata_rva = read_u32_le(cli_header, 8)?;
    let metadata_size = read_u32_le(cli_header, 12)? as usize;
    if metadata_rva == 0 || metadata_size == 0 {
        return Err(format!(
            "exact entry '{exact_entry}' has an empty CLR metadata locator"
        ));
    }
    let metadata_bytes = pe
        .read_at_rva(metadata_rva, metadata_size)
        .ok_or_else(|| format!("exact entry '{exact_entry}' has out-of-range CLR metadata"))?;
    Metadata::parse(metadata_bytes).map_err(|error| {
        format!("exact entry '{exact_entry}' has unreadable CLR metadata: {error}")
    })?;

    Ok(ManagedFormatProof {
        pe_kind: if pe.is_64bit() { "PE32+" } else { "PE32" },
        is_dll: true,
        clr_header_major: major,
        clr_header_minor: minor,
        clr_metadata_bytes: metadata_bytes.len() as u64,
        clr_metadata_sha256: sha256_hex(metadata_bytes),
    })
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| "truncated CLR header".to_string())?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "truncated CLR header".to_string())?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

#[allow(clippy::too_many_arguments)]
fn build_evidence(
    source_alias: String,
    relative_container: String,
    raw_container: &[u8],
    level: usize,
    exact_entry: &str,
    payload: &[u8],
    managed_format: ManagedFormatProof,
    materialization: PayloadMaterialization,
) -> ManagedAssemblyEvidence {
    let raw_sha256 = sha256_hex(raw_container);
    let selection = EntrySelection {
        container_format: "UnityWeb",
        level,
        exact_entry: exact_entry.to_string(),
        route: format!("level{level}/{exact_entry}"),
    };
    ManagedAssemblyEvidence {
        schema: SCHEMA,
        schema_version: 1,
        source: SourceEvidence {
            alias: source_alias,
            raw_container: RawContainerEvidence {
                relative_path: relative_container,
                bytes: raw_container.len() as u64,
                sha256: raw_sha256.clone(),
            },
        },
        selection: selection.clone(),
        managed_format,
        payload: PayloadEvidence {
            bytes: payload.len() as u64,
            sha256: sha256_hex(payload),
            materialization,
        },
        reproduction: ReproductionEvidence {
            extractor: "ffbuildtool.unityweb.exact-entry.in-memory.v1",
            selector: selection,
            required_raw_container_sha256: raw_sha256,
        },
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:X}", hasher.finalize())
}

fn preflight_output_targets(
    source_container: &Path,
    report_output: Option<&Path>,
    payload_output: Option<&Path>,
) -> Result<(), String> {
    let source = fs::canonicalize(source_container)
        .map_err(|error| format!("{}: {error}", source_container.display()))?;
    let report = match report_output {
        Some(path) if path != Path::new("-") => Some(target_identity(path, "report output")?),
        _ => None,
    };
    let payload = payload_output
        .map(|path| target_identity(path, "payload output"))
        .transpose()?;

    if report
        .as_ref()
        .is_some_and(|path| same_target(path, &source))
    {
        return Err("report output must not overwrite the raw source container".to_string());
    }
    if payload
        .as_ref()
        .is_some_and(|path| same_target(path, &source))
    {
        return Err("payload output must not overwrite the raw source container".to_string());
    }
    if let (Some(report), Some(payload)) = (&report, &payload) {
        if same_target(report, payload) {
            return Err("report output and payload output must be different files".to_string());
        }
    }
    Ok(())
}

fn target_identity(path: &Path, label: &str) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err(format!("{label} path may not be empty"));
    }
    if path.is_dir() {
        return Err(format!("{label} is a directory: {}", path.display()));
    }
    if path.exists() {
        return fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()));
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let parent = absolute.parent().unwrap_or_else(|| Path::new("."));
    let canonical_parent =
        fs::canonicalize(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    let file_name = absolute
        .file_name()
        .ok_or_else(|| format!("{label} must name a file"))?;
    Ok(canonical_parent.join(file_name))
}

fn same_target(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn write_json(path: Option<&Path>, evidence: &ManagedAssemblyEvidence) -> Result<(), String> {
    let mut json = serde_json::to_vec_pretty(evidence).map_err(|error| error.to_string())?;
    json.push(b'\n');
    match path {
        None => io::stdout()
            .write_all(&json)
            .map_err(|error| error.to_string()),
        Some(path) if path == Path::new("-") => io::stdout()
            .write_all(&json)
            .map_err(|error| error.to_string()),
        Some(path) => fs::write(path, json).map_err(|error| format!("{}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests;

#[test]
#[ignore = "requires the local clean primary client build"]
fn primary_main_unity3d_managed_assembly_smoke() {
    let source = Path::new("../builds/retrobution-20260613/main.unity3d");
    let raw = fs::read(source).expect("read clean primary main.unity3d");
    assert_eq!(
        sha256_hex(&raw),
        "59788201962B6A1737B114486C361FE74EEF69F507D1D125CA3171377EEC602F"
    );

    let (_header, bundle) = AssetBundle::from_bytes(&raw).expect("parse primary UnityWeb");
    let selected = select_exact_entry(bundle.iter_files(), "Assembly - CSharp.dll", None)
        .expect("unique primary managed assembly");
    assert_eq!(selected.level, 0);
    assert_eq!(selected.bytes.len(), 1_517_568);
    assert_eq!(
        sha256_hex(selected.bytes),
        "33D6F70216B1C7BA05BCC0F270FBA97E767B129159755AF4C8835922E60ACADB"
    );

    let proof =
        validate_managed_assembly(selected.name, selected.bytes).expect("managed CLR proof");
    assert!(proof.is_dll);
    assert!(proof.clr_metadata_bytes > 0);

    let smoke_root = Path::new("work/managed-assembly-evidence-smoke");
    fs::create_dir_all(smoke_root).expect("smoke output directory");
    let report = smoke_root.join("primary-assembly-csharp.evidence.json");
    let payload = smoke_root.join("Assembly - CSharp.dll");
    export_managed_assembly_evidence_cli(&[
        "primary".to_string(),
        "../builds/retrobution-20260613".to_string(),
        "main.unity3d".to_string(),
        "Assembly - CSharp.dll".to_string(),
        "--level".to_string(),
        "0".to_string(),
        "--out".to_string(),
        report.to_string_lossy().to_string(),
        "--payload-out".to_string(),
        payload.to_string_lossy().to_string(),
    ])
    .expect("end-to-end evidence export");

    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("evidence report")).expect("report JSON");
    assert_eq!(document["schema"], SCHEMA);
    assert_eq!(
        document["source"]["rawContainer"]["sha256"],
        "59788201962B6A1737B114486C361FE74EEF69F507D1D125CA3171377EEC602F"
    );
    assert_eq!(
        document["payload"]["sha256"],
        "33D6F70216B1C7BA05BCC0F270FBA97E767B129159755AF4C8835922E60ACADB"
    );
    assert_eq!(
        document["payload"]["materialization"]["portableRelativePath"],
        "work/managed-assembly-evidence-smoke/Assembly - CSharp.dll"
    );
    assert_eq!(
        sha256_hex(&fs::read(&payload).expect("materialized assembly")),
        "33D6F70216B1C7BA05BCC0F270FBA97E767B129159755AF4C8835922E60ACADB"
    );
}
