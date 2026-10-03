use super::*;

pub(super) fn run_audio_taxonomy_migration(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if !matches!(args.len(), 3 | 4) {
        return Err(format!(
            "migrate-audio-taxonomy requires <ASSET_ROOT> <NANO_RESOURCE_ROOT> [--apply]\n\n{USAGE}"
        ));
    }
    let apply = if args.len() == 4 {
        if args[3] != "--apply" {
            return Err(format!(
                "unknown migrate-audio-taxonomy argument {:?}\n\n{USAGE}",
                args[3]
            ));
        }
        true
    } else {
        false
    };
    let report = migrate_audio_taxonomy(
        &AudioTaxonomyMigrationOptions::new(&args[1], &args[2]).with_apply(apply),
    )
    .map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize audio taxonomy report: {error}"))
}

pub(super) fn run_semantic_audio_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 4 {
        return Err(format!(
            "install-semantic-audio requires <ASSET_ROOT> <COOK_REPORT_JSON> <SOURCE_BUILD>\n\n{USAGE}"
        ));
    }
    let source_build = args[3]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let report = install_semantic_audio(&SemanticAudioInstallOptions::new(
        PathBuf::from(&args[1]),
        PathBuf::from(&args[2]),
        source_build,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "installed {} semantic OGG files ({} bytes) without deleting original hashed sources; catalog={}, manifestFiles={}, explicitSharedFallbacks={}, variantAssets={}",
        report.installed_audio_files,
        report.installed_audio_bytes,
        report.catalog_path,
        report.manifest_files,
        report.explicit_shared_fallbacks,
        report.variant_assets,
    ))
}
