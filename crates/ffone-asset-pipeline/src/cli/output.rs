use super::*;

pub(super) fn run_native_gameplay_ui_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 3 {
        return Err(format!(
            "install-gameplay-ui requires <ASSET_ROOT> <SOURCE_BUILD>\n\n{USAGE}"
        ));
    }
    let source_build = args[2]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let report = install_native_gameplay_ui(&NativeGameplayUiInstallOptions::new(
        PathBuf::from(&args[1]),
        source_build,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "installed {} exact native gameplay UI files ({} bytes) into {}; manifestFiles={}",
        report.installed_files, report.installed_bytes, report.root, report.manifest_files,
    ))
}

pub(super) fn run_tutorial_effect_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 5 {
        return Err(format!(
            "install-tutorial-effects requires <ASSET_ROOT> <EFFECTS_SERIALIZED_ASSET> <DUMP_OBJECT_ALL_JSON> <SOURCE_BUILD> and two --dependency <SERIALIZED_ASSET> <DUMP_OBJECT_ALL_JSON> pairs\n\n{USAGE}"
        ));
    }
    let source_build = args[4]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let mut options = TutorialEffectInstallOptions::new(
        PathBuf::from(&args[1]),
        PathBuf::from(&args[2]),
        PathBuf::from(&args[3]),
    );
    options.source_build = source_build.to_owned();
    let mut index = 5;
    while index < args.len() {
        if args[index].to_str() != Some("--dependency") || index + 2 >= args.len() {
            return Err(format!(
                "each tutorial effect dependency must be `--dependency <SERIALIZED_ASSET> <DUMP_OBJECT_ALL_JSON>`\n\n{USAGE}"
            ));
        }
        options.add_dependency(
            PathBuf::from(&args[index + 1]),
            PathBuf::from(&args[index + 2]),
        );
        index += 3;
    }
    let report = install_tutorial_effects(&options).map_err(|error| error.to_string())?;
    Ok(format!(
        "installed {} exact tutorial effects, {} projectile particle effects and {} exact BulletTable rows from {} dependencies as {} manifest-owned files ({} bytes); closureObjects={}, sourceBundleBlake3={}, rendererStatus={}",
        report.effect_count,
        report.projectile_effect_count,
        report.bullet_row_count,
        report.dependency_count,
        report.installed_files,
        report.installed_bytes,
        report.closure_object_count,
        report.source_bundle_blake3,
        report.renderer_status,
    ))
}

pub(super) fn run_world_behaviour_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 4 {
        return Err(format!(
            "install-world-behaviours requires <EXPORT_ROOT> <ASSET_ROOT> <SOURCE_BUILD>\n\n{USAGE}"
        ));
    }
    let report = install_world_behaviours(&WorldBehaviourInstallOptions::new(
        &args[1],
        &args[2],
        args[3].to_string_lossy().into_owned(),
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "installed native world behaviours: {} tiles, {} billboards, {} visibility switches, {} effect emitters, {} animation players, {} triggers, {} waypoints, {} trigger volumes, {} rigid bodies, {} blockers; {} files ({} bytes), manifestFiles={}, replacedPreviousInstall={}, sourceSetBlake3={}",
        report.tile_count,
        report.billboard_count,
        report.visibility_switch_count,
        report.effect_emitter_count,
        report.animation_count,
        report.trigger_count,
        report.waypoint_count,
        report.trigger_volume_count,
        report.rigid_body_count,
        report.blocker_count,
        report.installed_files,
        report.installed_bytes,
        report.manifest_files,
        report.replaced_previous_install,
        report.source_set_blake3,
    ))
}

pub(super) fn run_world_map_static_world_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 4 {
        return Err(format!(
            "install-world-map-static-world requires <EXPORT_ROOT> <ASSET_ROOT> <CONTRACT_JSON>\n\n{USAGE}"
        ));
    }
    let report = install_world_map_static_world(&WorldMapStaticWorldInstallOptions::new(
        &args[1], &args[2], &args[3],
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "installed exact world-map static world: {} tiles, {} GLBs, {} textures, {} metadata files and {} merged scenes as {} manifest publications ({} bytes); nodes={}, runtimeVisuals={}, runtimeColliders={}, vertices={}, indices={}, replacedPreviousInstall={}, manifestFiles={}, sourceSetBlake3={}",
        report.tile_count,
        report.model_count,
        report.texture_count,
        report.static_metadata_count,
        report.merged_scene_count,
        report.installed_files,
        report.installed_bytes,
        report.scene_node_count,
        report.runtime_visual_count,
        report.runtime_collider_count,
        report.vertex_count,
        report.index_count,
        report.replaced_previous_install,
        report.manifest_files,
        report.source_set_blake3,
    ))
}

pub(super) fn run_tutorial_static_world_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 3 {
        return Err(format!(
            "install-tutorial-static-world requires <EXPORT_ROOT> <ASSET_ROOT>\n\n{USAGE}"
        ));
    }
    let report =
        install_tutorial_static_world(&TutorialStaticWorldInstallOptions::new(&args[1], &args[2]))
            .map_err(|error| error.to_string())?;
    Ok(format!(
        "installed exact tutorial static world: {} tiles, {} GLBs, {} textures, {} metadata files and {} merged scenes as {} manifest publications ({} bytes); nodes={}, runtimeVisuals={}, runtimeColliders={}, vertices={}, indices={}, replacedPreviousInstall={}, manifestFiles={}, sourceSetBlake3={}",
        report.tile_count,
        report.model_count,
        report.texture_count,
        report.static_metadata_count,
        report.merged_scene_count,
        report.installed_files,
        report.installed_bytes,
        report.scene_node_count,
        report.runtime_visual_count,
        report.runtime_collider_count,
        report.vertex_count,
        report.index_count,
        report.replaced_previous_install,
        report.manifest_files,
        report.source_set_blake3,
    ))
}

pub(super) fn run_semantic_icon_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 4 {
        return Err(format!(
            "install-semantic-icons requires <ASSET_ROOT> <TABLE_SET_JSON> <SOURCE_BUILD>\n\n{USAGE}"
        ));
    }
    let source_build = args[3]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let report = install_semantic_icons(&SemanticIconInstallOptions::new(
        PathBuf::from(&args[1]),
        PathBuf::from(&args[2]),
        source_build,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "installed {} strictly classified semantic icons ({} copied bytes); unmatched={}, classificationSchema={}, runtimeMetadataFiles=0, manifestFiles={}",
        report.published,
        report.copied_bytes,
        report.unmatched,
        report.catalog.schema,
        report.manifest_files,
    ))
}

pub(super) fn run_logical_character_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 5 {
        return Err(format!(
            "install-logical-characters requires <CANDIDATE_ROOT> <GPU_AUDIT_JSON> <ASSET_ROOT> <SOURCE_BUILD>\n\n{USAGE}"
        ));
    }
    let source_build = args[4]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let report = install_logical_characters(&LogicalCharacterInstallOptions::new(
        PathBuf::from(&args[1]),
        PathBuf::from(&args[2]),
        PathBuf::from(&args[3]),
        source_build,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "installed {} audited native character assets ({} bytes, {} models) into permanent project assets; manifestFiles={}, runtimeAccepted={}, productionApproved={}, visualParityPending={}",
        report.installed_files,
        report.installed_bytes,
        report.catalog.models.len(),
        report.manifest_files,
        report.catalog.runtime_accepted,
        report.catalog.production_approved,
        report.catalog.visual_parity_pending,
    ))
}

pub(super) fn run_semantic_character_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 7 && args.len() != 8 {
        return Err(format!(
            "install-semantic-characters requires <CANDIDATE_ROOT> <GPU_AUDIT_JSON> <TABLE_SET_JSON> <ASSET_ROOT> <SOURCE_BUILD> <REPORT_JSON> [--replace-existing]\n\n{USAGE}"
        ));
    }
    let replace_existing = match args.get(7).and_then(|value| value.to_str()) {
        None => false,
        Some("--replace-existing") => true,
        Some(other) => {
            return Err(format!(
                "unknown install-semantic-characters argument {other:?}\n\n{USAGE}"
            ));
        }
    };
    let source_build = args[5]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let report = install_semantic_characters(
        &SemanticCharacterInstallOptions::new(
            PathBuf::from(&args[1]),
            PathBuf::from(&args[2]),
            PathBuf::from(&args[3]),
            PathBuf::from(&args[4]),
            source_build,
            PathBuf::from(&args[6]),
        )
        .with_replace_existing(replace_existing),
    )
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "installed {} individually GPU-passed semantic characters (replaced={}; nanos={}, npcs={}, mobs={}, fusions={}, shared={}; glbs={}, pngs={}, skipped={}, rawRecoveryModelsMutated={}); manifestFiles={}",
        report.counts.published_models,
        report.counts.replaced_models,
        report.counts.published_nanos,
        report.counts.published_npcs,
        report.counts.published_mobs,
        report.counts.published_fusions,
        report.counts.published_shared,
        report.counts.installed_glbs,
        report.counts.installed_pngs,
        report.counts.skipped_models,
        report.raw_recovery_models_mutated,
        report.counts.manifest_files,
    ))
}

pub(super) fn write_lifecycle_report(
    report: &RuntimeCharacterModelLifecycleReport,
    report_path: &PathBuf,
) -> Result<String, String> {
    let serialized = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("failed to serialize lifecycle report: {error}"))?;
    if let Some(parent) = report_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    std::fs::write(report_path, serialized)
        .map_err(|error| format!("could not write {}: {error}", report_path.display()))?;
    Ok(format!(
        "runtime character model {}: id={}, newId={}, logicalName={}, aliases={:?}, applied={} (report {})",
        report.operation,
        report.id,
        report.new_id.as_deref().unwrap_or("-"),
        report.logical_name,
        report.legacy_aliases,
        report.applied,
        report_path.display()
    ))
}

pub(super) fn run_player_equipment_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 5 {
        return Err(format!(
            "install-player-equipment requires <CANDIDATE_ROOT> <GPU_BATCH_JSON> <ASSET_ROOT> <SOURCE_BUILD>\n\n{USAGE}"
        ));
    }
    let source_build = args[4]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let report = install_player_equipment(&PlayerEquipmentInstallOptions::new(
        PathBuf::from(&args[1]),
        PathBuf::from(&args[2]),
        PathBuf::from(&args[3]),
        source_build,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "installed {} audited native player-equipment files ({} bytes, {} models) into {}; manifestFiles={}, standaloneGpuPassed={}, playerAssemblyParityPending={}",
        report.installed_files,
        report.installed_bytes,
        report.installed_models,
        report.destination,
        report.manifest_files,
        report.standalone_gpu_passed,
        report.player_assembly_parity_pending,
    ))
}

pub(super) fn run_character_creation_data_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 4 {
        return Err(format!(
            "install-character-creation-data requires <TABLE_SET_JSON> <ASSET_ROOT> <SOURCE_BUILD>\n\n{USAGE}"
        ));
    }
    let source_build = args[3]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let report = install_character_creation_data(&CharacterCreationDataInstallOptions::new(
        PathBuf::from(&args[1]),
        PathBuf::from(&args[2]),
        source_build,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "installed {} native character-creation JSON files ({} bytes) into {}; names={}/{}/{}, creationRows={}, avatarItems={}, avatarLookupComplete={}",
        report.installed_files,
        report.installed_bytes,
        report.destination,
        report.first_names,
        report.middle_names,
        report.last_names,
        report.creation_rows,
        report.avatar_items,
        report.avatar_lookup_complete,
    ))
}
