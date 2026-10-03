use std::{fs, path::Path, process::ExitCode};

use crate::{LogicalModelPublishOptions, publish_logical_model};

pub(super) fn run_command(command_args: &[String]) -> ExitCode {
    let args = command_args.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("usage: publish_player_head_models <SOURCE_ROOT> <FRESH_OUTPUT_ROOT>");
        return ExitCode::FAILURE;
    }
    match run(Path::new(&args[0]), Path::new(&args[1])) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("publish_player_head_models: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(source_root: &Path, output_root: &Path) -> Result<(), String> {
    if output_root.exists() {
        return Err(format!(
            "output root must be fresh: {}",
            output_root.display()
        ));
    }
    let mut sources = fs::read_dir(source_root)
        .map_err(|error| format!("could not read {}: {error}", source_root.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("could not enumerate {}: {error}", source_root.display()))?;
    sources.retain(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".source.json"))
    });
    sources.sort();
    if sources.is_empty() {
        return Err(format!(
            "{} has no *.source.json inputs",
            source_root.display()
        ));
    }

    for source in sources {
        let filename = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("source filename is not UTF-8: {}", source.display()))?;
        let true_name = filename
            .strip_suffix(".source.json")
            .ok_or_else(|| format!("source filename has no .source.json suffix: {filename}"))?;
        if !is_player_head_true_name(true_name) {
            return Err(format!(
                "refusing non-player-head logical model {true_name:?}"
            ));
        }
        let options = LogicalModelPublishOptions::new(&source, "characters", output_root)
            .with_semantic_directories(["player", "equipment", "head", true_name])
            .with_semantic_root_layout();
        let report = publish_logical_model(&options).map_err(|error| error.to_string())?;
        println!(
            "{}: excludedHelpers={}, excludedParts={}, preservedNodes={}, preservedSkins={}, glb={}",
            true_name,
            report.source_geometry_filter.excluded_rigid_mesh_bindings,
            report.source_geometry_filter.excluded_mesh_parts,
            report.source_geometry_filter.preserved_transform_nodes,
            report
                .source_geometry_filter
                .preserved_skinned_mesh_bindings,
            report.contract.output_glb
        );
    }
    Ok(())
}

fn is_player_head_true_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    matches!(bytes.first(), Some(b'm' | b'f'))
        && value
            .strip_prefix("m_head_")
            .or_else(|| value.strip_prefix("f_head_"))
            .is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            })
}
