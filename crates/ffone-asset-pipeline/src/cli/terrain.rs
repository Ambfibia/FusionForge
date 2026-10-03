use super::*;

pub(super) fn run_published_terrain_dedup(args: &[std::ffi::OsString]) -> Result<String, String> {
    if !(args.len() == 3 || (args.len() == 4 && args[3] == "--apply")) {
        return Err(format!(
            "dedupe-published-terrain requires <PROJECT_ROOT> <REPORT_JSON> [--apply]\n\n{USAGE}"
        ));
    }
    let apply = args.len() == 4;
    let report = dedupe_published_terrain(&PublishedTerrainDedupOptions::new(
        &args[1], &args[2], apply,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "{} {} terrain-detail source packages into {} shared packages; removed {} files / estimated {} bytes; catalog {} -> {}; report={}",
        if apply { "deduplicated" } else { "planned" },
        report.counts.detail_source_packages,
        report.counts.detail_shared_packages,
        report.counts.removed_files,
        report.counts.estimated_net_bytes_saved,
        report.source_catalog_blake3,
        report.result_catalog_blake3,
        PathBuf::from(&args[2]).display(),
    ))
}

pub(super) fn run_published_terrain_shift_restore(args: &[std::ffi::OsString]) -> Result<String, String> {
    if !(args.len() == 4 || (args.len() == 5 && args[4] == "--apply")) {
        return Err(format!(
            "restore-published-terrain-shifts requires <PROJECT_ROOT> <PRIMARY_EXPORT_ROOT> <REPORT_JSON> [--apply]\n\n{USAGE}"
        ));
    }
    let apply = args.len() == 5;
    let report = restore_published_terrain_shifts(&PublishedTerrainShiftRestoreOptions::new(
        &args[1], &args[2], &args[3], apply,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "{} {} clean-primary vertex shifts across {} terrain tiles; catalog {} -> {}; report={}",
        if apply { "restored" } else { "planned" },
        report.counts.restored_vertex_shifts,
        report.counts.restored_tiles,
        report.source_catalog_blake3,
        report.result_catalog_blake3,
        PathBuf::from(&args[3]).display(),
    ))
}
