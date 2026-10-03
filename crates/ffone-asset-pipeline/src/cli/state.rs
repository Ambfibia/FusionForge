use super::*;

pub(super) fn run_runtime_metadata_cleanup(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if !matches!(args.len(), 3 | 4) {
        return Err(format!(
            "clean-runtime-metadata requires <PROJECT_ROOT> <SOURCE_BUILD> [--apply]\n\n{USAGE}"
        ));
    }
    let project_root = PathBuf::from(&args[1]);
    let source_build = args[2]
        .to_str()
        .ok_or_else(|| "source build must be valid UTF-8".to_owned())?;
    let apply = if args.len() == 4 {
        if args[3] != "--apply" {
            return Err(format!(
                "unknown clean-runtime-metadata argument {:?}\n\n{USAGE}",
                args[3]
            ));
        }
        true
    } else {
        false
    };
    let report = clean_runtime_metadata(
        &CleanRuntimeMetadataOptions::new(project_root, source_build).with_apply(apply),
    )
    .map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize cleanup report: {error}"))
}

pub(super) fn run_runtime_world_migration(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if !matches!(args.len(), 3 | 4) {
        return Err(format!(
            "migrate-runtime-world requires <PROJECT_ROOT> <SOURCE_BUILD> [--apply]\n\n{USAGE}"
        ));
    }
    let project_root = PathBuf::from(&args[1]);
    let source_build = args[2]
        .to_str()
        .ok_or_else(|| "source build must be valid UTF-8".to_owned())?;
    let apply = if args.len() == 4 {
        if args[3] != "--apply" {
            return Err(format!(
                "unknown migrate-runtime-world argument {:?}\n\n{USAGE}",
                args[3]
            ));
        }
        true
    } else {
        false
    };
    let report = migrate_runtime_world(
        &RuntimeWorldMigrationOptions::new(project_root, source_build).with_apply(apply),
    )
    .map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize runtime-world migration report: {error}"))
}
