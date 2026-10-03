use super::*;

pub(super) fn run_world_prefab_organizer(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 3 {
        return Err(format!(
            "organize-map requires <PROJECT_ROOT> <REPORT_JSON> [--tutorial-metadata-root <DIR>] [--apply] [--replace]\n\n{USAGE}"
        ));
    }
    let mut apply = false;
    let mut replace = false;
    let mut tutorial_metadata_root = None;
    let mut index = 3;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| "organize-map arguments must be UTF-8".to_owned())?;
        match flag {
            "--apply" => {
                if apply {
                    return Err("--apply may only be supplied once".to_owned());
                }
                apply = true;
                index += 1;
            }
            "--replace" => {
                if replace {
                    return Err("--replace may only be supplied once".to_owned());
                }
                replace = true;
                index += 1;
            }
            "--tutorial-metadata-root" => {
                if tutorial_metadata_root.is_some() {
                    return Err("--tutorial-metadata-root may only be supplied once".to_owned());
                }
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--tutorial-metadata-root requires a directory".to_owned())?;
                tutorial_metadata_root = Some(PathBuf::from(value));
                index += 2;
            }
            other => return Err(format!("unknown organize-map flag {other:?}")),
        }
    }
    let mut options = WorldPrefabOrganizerOptions::new(&args[1], &args[2], apply);
    if let Some(root) = tutorial_metadata_root {
        options = options.with_tutorial_metadata_root(root);
    }
    if replace {
        if !apply {
            return Err("--replace requires --apply".to_owned());
        }
        options = options.replacing_existing();
    }
    let report = organize_world_prefabs(&options).map_err(|error| error.to_string())?;
    Ok(format!(
        "{} map: {} tiles, {} instances, {} geometry parts ({} visual, {} collision), {} objects, {} prefixes, {} categories, {} files/{} bytes; sourceSetBlake3={}, resultSetBlake3={}, report={}",
        if apply { "published" } else { "planned" },
        report.counts.tiles,
        report.counts.placements,
        report.counts.reusable_resources,
        report.counts.visual_resources,
        report.counts.collider_resources,
        report.counts.prefabs,
        report.prefixes.len(),
        report.categories.len(),
        report.counts.output_files,
        report.counts.output_bytes,
        report.source_set_blake3,
        report.result_set_blake3.as_deref().unwrap_or("plan-only"),
        report.report_path,
    ))
}

pub(super) fn run_world_prefab_verification(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args.len() != 3 {
        return Err(format!(
            "verify-map requires <PROJECT_ROOT> <REPORT_JSON>\n\n{USAGE}"
        ));
    }
    let verification = verify_world_prefab_library_to_report(&args[1], &args[2])
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "verified map: geometry={}, objects={}, tiles={}, placements={}, textures={}, files={}, bytes={}, maximumPositionError={}, resultSetBlake3={}",
        verification.resources,
        verification.prefabs,
        verification.placement_sets,
        verification.placements,
        verification.textures,
        verification.files,
        verification.bytes,
        verification.maximum_position_reconstruction_error,
        verification.result_set_blake3,
    ))
}
