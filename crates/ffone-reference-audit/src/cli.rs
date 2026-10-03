use std::{ffi::OsString, path::PathBuf};

use crate::{AuditOptions, UiParityEntryKind, load_ui_parity_matrix, scan, write_inventory};

const USAGE: &str = "FusionForge deterministic legacy reference audit

Usage:
  cargo reference-audit [OPTIONS]
  cargo fusionforge reference-audit scan [OPTIONS]
  cargo fusionforge reference-audit validate-ui-parity [OPTIONS]

Scan options:
  --csharp <DIR>       Effective Assembly-CSharp decompile (required)
  --firstpass <DIR>    Effective firstpass decompile (required)
  --openfusion <DIR>   OpenFusion source root (required)
  --coverage <FILE>    Explicit coverage map [default: docs/reference/evidence/legacy/ffone/managed-code/b8c3-native-port-coverage.json]
  --no-coverage        Generate an unclassified inventory without a coverage map
  --output <FILE>      Inventory output [default: stdout]

UI parity validation options:
  --input <FILE>       UI parity ledger [default: docs/reference/evidence/legacy/ffone/ui/parity-matrix.json]

  -h, --help           Show this help
";

pub fn run(args: impl IntoIterator<Item = OsString>) -> Result<String, String> {
    let args = args.into_iter().collect::<Vec<_>>();
    if args
        .first()
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.first().is_some_and(|arg| arg == "validate-ui-parity") {
        return validate_ui_parity_command(&args[1..]);
    }
    if args.first().is_none_or(|arg| arg != "scan") {
        return Err("expected `scan` or `validate-ui-parity` command".to_owned());
    }

    let mut assembly_csharp = None;
    let mut assembly_firstpass = None;
    let mut openfusion_src = None;
    let mut coverage = Some(PathBuf::from(
        "docs/reference/evidence/legacy/ffone/managed-code/b8c3-native-port-coverage.json",
    ));
    let mut output = None;
    let mut index = 1;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| "arguments must be valid UTF-8".to_owned())?;
        if flag == "--no-coverage" {
            coverage = None;
            index += 1;
            continue;
        }
        if matches!(flag, "--help" | "-h") {
            return Ok(USAGE.to_owned());
        }
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| format!("{flag} requires a path"))?;
        match flag {
            "--csharp" => assembly_csharp = Some(PathBuf::from(value)),
            "--firstpass" => assembly_firstpass = Some(PathBuf::from(value)),
            "--openfusion" => openfusion_src = Some(PathBuf::from(value)),
            "--coverage" => coverage = Some(PathBuf::from(value)),
            "--output" => output = Some(PathBuf::from(value)),
            unknown => return Err(format!("unknown argument {unknown:?}")),
        }
        index += 1;
    }

    let options = AuditOptions {
        assembly_csharp: assembly_csharp.ok_or_else(|| "--csharp is required".to_owned())?,
        assembly_firstpass: assembly_firstpass
            .ok_or_else(|| "--firstpass is required".to_owned())?,
        openfusion_src: openfusion_src.ok_or_else(|| "--openfusion is required".to_owned())?,
        coverage,
    };

    let inventory = scan(&options)?;
    let Some(output) = output else {
        return serde_json::to_string_pretty(&inventory).map_err(|e| e.to_string());
    };
    write_inventory(&output, &inventory)?;
    Ok(format!(
        "wrote {} scripts, {} classes, {} packet definitions and {} registered shard handlers to {}",
        inventory.summary.script_files,
        inventory.summary.classes,
        inventory.summary.packet_definitions,
        inventory.summary.registered_shard_packets,
        output.display()
    ))
}

fn validate_ui_parity_command(args: &[OsString]) -> Result<String, String> {
    let mut input = PathBuf::from("docs/reference/evidence/legacy/ffone/ui/parity-matrix.json");
    let mut index = 0;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| "arguments must be valid UTF-8".to_owned())?;
        if matches!(flag, "--help" | "-h") {
            return Ok(USAGE.to_owned());
        }
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| format!("{flag} requires a path"))?;
        match flag {
            "--input" => input = PathBuf::from(value),
            unknown => return Err(format!("unknown argument {unknown:?}")),
        }
        index += 1;
    }

    let matrix = load_ui_parity_matrix(&input)?;
    let modes = matrix
        .entries
        .iter()
        .filter(|entry| entry.kind == UiParityEntryKind::Mode)
        .count();
    let families = matrix
        .entries
        .iter()
        .filter(|entry| entry.kind == UiParityEntryKind::Family)
        .count();
    Ok(format!(
        "validated {modes} eGameMode entries and {families} cross-mode UI families in {}",
        input.display()
    ))
}

#[cfg(test)]
mod tests;
