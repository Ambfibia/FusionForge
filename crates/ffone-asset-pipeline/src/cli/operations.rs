use super::*;

pub fn run(args: impl IntoIterator<Item = std::ffi::OsString>) -> Result<String, String> {
    let args = args.into_iter().collect::<Vec<_>>();
    if args
        .first()
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.first().is_some_and(|arg| arg == "audit-models") {
        return run_model_audit(&args);
    }
    if args.first().is_some_and(|arg| arg == "compose-assets") {
        return run_asset_composition(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "plan-semantic-assets")
    {
        return run_semantic_asset_plan(&args);
    }
    if args.first().is_some_and(|arg| arg == "cook-player-avatar") {
        return run_player_avatar_cook(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "audit-logical-model-gpu-evidence")
    {
        return run_logical_model_gpu_evidence_audit(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "audit-logical-model-tree")
    {
        return run_logical_model_tree_audit(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "publish-logical-model-batch")
    {
        return run_logical_model_batch_publish(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "publish-equipment-logical-model-batch")
    {
        return run_equipment_logical_model_batch_publish(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "accept-equipment-gpu-batch")
    {
        return run_equipment_gpu_acceptance_batch(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-logical-characters")
    {
        return run_logical_character_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-tutorial-models")
    {
        return run_tutorial_model_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "dedupe-tutorial-character-models")
    {
        return run_tutorial_character_model_dedupe(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "promote-tutorial-characters")
    {
        return run_tutorial_character_promotion(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "promote-tutorial-props")
    {
        return run_tutorial_prop_promotion(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "update-tutorial-npc-building")
    {
        return run_tutorial_npc_building_update(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-semantic-characters")
    {
        return run_semantic_character_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "replace-runtime-character-model")
    {
        return run_runtime_character_model_replace(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-runtime-character-model")
    {
        return run_runtime_character_model_install(&args);
    }
    if args.first().is_some_and(|arg| arg == "encode-native-model") {
        return run_native_model_encode(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "rename-runtime-character-model")
    {
        return run_runtime_character_model_rename(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "remove-runtime-character-model")
    {
        return run_runtime_character_model_remove(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "repair-character-registry-aliases")
    {
        return run_character_registry_alias_repair(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "apply-character-registry-alias-plan")
    {
        return run_character_registry_alias_plan(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-player-item-models")
    {
        return run_player_item_model_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "refresh-avatar-item-models")
    {
        return run_avatar_item_model_refresh(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-player-equipment")
    {
        return run_player_equipment_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-character-creation-data")
    {
        return run_character_creation_data_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "register-character-runtime-textures")
    {
        return run_character_runtime_texture_registration(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "refresh-manifest-entry")
    {
        return run_manifest_entry_refresh(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "register-manifest-entry")
    {
        return run_manifest_entry_registration(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "clean-runtime-metadata")
    {
        return run_runtime_metadata_cleanup(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "migrate-runtime-world")
    {
        return run_runtime_world_migration(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "migrate-audio-taxonomy")
    {
        return run_audio_taxonomy_migration(&args);
    }
    if args.first().is_some_and(|arg| arg == "install-gameplay-ui") {
        return run_native_gameplay_ui_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "convert-legacy-gui-skins")
    {
        return run_legacy_gui_skin_conversion(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-tutorial-effects")
    {
        return run_tutorial_effect_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-tutorial-static-world")
    {
        return run_tutorial_static_world_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-world-map-static-world")
    {
        return run_world_map_static_world_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "repair-static-world-winding")
    {
        return run_static_world_winding_repair(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "verify-static-world-winding")
    {
        return run_static_world_winding_verification(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "archive-repaired-tutorial-winding")
    {
        return run_static_world_winding_archive(&args);
    }
    if args.first().is_some_and(|arg| arg == "organize-map") {
        return run_world_prefab_organizer(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "organize-resource-sets")
    {
        return run_resource_set_organizer(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "normalize-object-routes")
    {
        return run_object_route_normalization(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "dedupe-published-terrain")
    {
        return run_published_terrain_dedup(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "restore-published-terrain-shifts")
    {
        return run_published_terrain_shift_restore(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "organize-player-item-sets")
    {
        return run_player_item_set_organizer(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "verify-player-item-sets")
    {
        return run_player_item_set_verification(&args);
    }
    if args.first().is_some_and(|arg| arg == "verify-map") {
        return run_world_prefab_verification(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-world-behaviours")
    {
        return run_world_behaviour_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-semantic-audio")
    {
        return run_semantic_audio_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-localized-voice")
    {
        return run_localized_voice_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "install-semantic-icons")
    {
        return run_semantic_icon_install(&args);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "publish-logical-model")
    {
        return run_logical_model_publish(&args);
    }
    if args.first().is_none_or(|arg| arg != "import") {
        return Err(format!(
            "expected a documented ffone-asset-pipeline command\n\n{USAGE}"
        ));
    }

    let mut pack = None;
    let mut output = PathBuf::from(DEFAULT_OUTPUT);
    let mut index = 1;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| "arguments must be valid UTF-8".to_owned())?;
        match flag {
            "--pack" => {
                if pack.is_some() {
                    return Err("--pack may only be supplied once".to_owned());
                }
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--pack requires a directory".to_owned())?;
                pack = Some(PathBuf::from(value));
            }
            "--output" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--output requires a directory".to_owned())?;
                output = PathBuf::from(value);
            }
            "--help" | "-h" => return Ok(USAGE.to_owned()),
            unknown => return Err(format!("unknown argument {unknown:?}\n\n{USAGE}")),
        }
        index += 1;
    }

    let pack = pack.ok_or_else(|| format!("--pack <DIR> is required\n\n{USAGE}"))?;
    let options = ImportOptions::new(pack).with_output(output.clone());
    let manifest = import_content_pack(&options).map_err(|error| error.to_string())?;
    let bytes = manifest.files.iter().map(|file| file.bytes).sum::<u64>();
    Ok(format!(
        "imported {} native assets ({} bytes) into {}",
        manifest.files.len(),
        bytes,
        output.display()
    ))
}

pub(super) fn run_player_avatar_cook(args: &[std::ffi::OsString]) -> Result<String, String> {
    let mut bundle = None;
    let mut objects = None;
    let mut logical_plan = None;
    let mut output = None;
    let mut part_sources = Vec::<(String, PathBuf)>::new();
    let mut index = 1;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| "arguments must be valid UTF-8".to_owned())?;
        if flag == "--help" || flag == "-h" {
            return Ok(USAGE.to_owned());
        }
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| format!("{flag} requires a value\n\n{USAGE}"))?;
        match flag {
            "--bundle" => bundle = Some(PathBuf::from(value)),
            "--objects" => objects = Some(PathBuf::from(value)),
            "--logical-plan" => logical_plan = Some(PathBuf::from(value)),
            "--output" => output = Some(PathBuf::from(value)),
            "--part-source" => {
                let value = value
                    .to_str()
                    .ok_or_else(|| "--part-source must be valid UTF-8".to_owned())?;
                let (route, path) = value
                    .split_once('=')
                    .ok_or_else(|| "--part-source must be EXACT_ROUTE=SOURCE_JSON".to_owned())?;
                if route.is_empty() || path.is_empty() {
                    return Err("--part-source route and path must be non-empty".to_owned());
                }
                part_sources.push((route.to_owned(), PathBuf::from(path)));
            }
            unknown => return Err(format!("unknown argument {unknown:?}\n\n{USAGE}")),
        }
        index += 1;
    }
    let mut options = PlayerAvatarCookOptions::new(
        bundle.ok_or_else(|| "--bundle is required".to_owned())?,
        objects.ok_or_else(|| "--objects is required".to_owned())?,
        logical_plan.ok_or_else(|| "--logical-plan is required".to_owned())?,
        output.ok_or_else(|| "--output is required".to_owned())?,
    );
    for (route, path) in part_sources {
        options = options.with_part_source(route, path);
    }
    let report = cook_player_avatar(&options).map_err(|error| error.to_string())?;
    Ok(format!(
        "indexed {} exact male skeleton nodes and {} AnimationClip objects; verified {} Test Ser part ownership/remap records; assembled GLB blocked by {} explicit gates; report={}",
        report.base_skeleton.node_count,
        report.base_skeleton.indexed_animation_clip_count,
        report.parts.len(),
        report.blockers.len(),
        options
            .output_root
            .join("data/catalog/avatar/test_ser_male_cook_report.json")
            .display()
    ))
}

pub(super) fn run_legacy_gui_skin_conversion(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 4 {
        return Err(format!(
            "convert-legacy-gui-skins requires <DUMP_OBJECT_ALL_JSON> <OUTPUT_JSON> <SOURCE_BUILD>\n\n{USAGE}"
        ));
    }
    let source_build = args[3]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let report = convert_legacy_gui_skins(&LegacyGuiSkinConversionOptions::new(
        PathBuf::from(&args[1]),
        PathBuf::from(&args[2]),
        source_build,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "wrote editor-only {} Unity GUI-skin candidate / {} skins / {} styles ({} textures, {} fonts) from build {} into {}; publicationAllowed={}; unresolvedPointers={}; diagnostics={}",
        report.evidence_level,
        report.skins,
        report.styles,
        report.referenced_textures,
        report.referenced_fonts,
        report.source_build,
        report.output,
        report.publication_allowed,
        report.unresolved_pointer_count,
        report.diagnostics,
    ))
}

pub(super) fn run_static_world_winding_repair(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if !(args.len() == 3 || (args.len() == 4 && args[3] == "--apply")) {
        return Err(format!(
            "repair-static-world-winding requires <PROJECT_ROOT> <REPORT_JSON> [--apply]\n\n{USAGE}"
        ));
    }
    let apply = args.len() == 4;
    let report = repair_static_world_winding(&StaticWorldWindingRepairOptions::new(
        &args[1], &args[2], apply,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "{} static-world winding repair: {} visual GLBs ({} converted, {} already aligned, {} resumed), {} triangles, {} scenes; sourceSetBlake3={}, resultSetBlake3={}, report={}",
        if apply { "applied" } else { "planned" },
        report.counts.visual_files,
        report.counts.converted_files,
        report.counts.unchanged_files,
        report.counts.resumed_files,
        report.counts.triangles,
        report.counts.scene_files,
        report.source_set_blake3,
        report.result_set_blake3,
        report.report_path,
    ))
}

pub(super) fn run_player_item_set_organizer(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args.len() != 3 {
        return Err(format!(
            "organize-player-item-sets requires <PROJECT_ROOT> <REPORT_JSON>\n\n{USAGE}"
        ));
    }
    let report = organize_player_item_sets(&args[1]).map_err(|error| error.to_string())?;
    let report_path = PathBuf::from(&args[2]);
    if let Some(parent) = report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    fs::write(&report_path, bytes).map_err(|error| error.to_string())?;
    Ok(format!(
        "organized {} player models into {} item sets; texture files {} -> {} atlases + {} renderer maps ({} duplicates and {} conversion reports removed); catalog={}",
        report.models,
        report.sets,
        report.source_texture_files,
        report.published_atlases,
        report.rendering_textures,
        report.duplicate_texture_files_removed,
        report.conversion_reports_removed,
        report.catalog.path,
    ))
}

pub(super) fn run_player_item_set_verification(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args.len() != 2 {
        return Err(format!(
            "verify-player-item-sets requires <PROJECT_ROOT>\n\n{USAGE}"
        ));
    }
    verify_player_item_sets(&args[1]).map_err(|error| error.to_string())?;
    Ok("verified self-contained player item resource sets and route catalog".to_owned())
}

pub(super) fn run_resource_set_organizer(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args.len() != 3 {
        return Err(format!(
            "organize-resource-sets requires <PROJECT_ROOT> <REPORT_JSON>\n\n{USAGE}"
        ));
    }
    let report = organize_resource_sets(&args[1]).map_err(|error| error.to_string())?;
    let report_path = PathBuf::from(&args[2]);
    if let Some(parent) = report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    fs::write(&report_path, bytes).map_err(|error| error.to_string())?;
    Ok(format!(
        "organized {} map objects into {} resource sets; textures {} -> {} ({} duplicate files removed); catalog={}",
        report.map_objects,
        report.map_sets,
        report.map_textures_before,
        report.map_textures_after,
        report.duplicate_texture_files_removed,
        report.catalog.path,
    ))
}

pub(super) fn run_static_world_winding_verification(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 3 {
        return Err(format!(
            "verify-static-world-winding requires <PROJECT_ROOT> <APPLY_REPORT_JSON>\n\n{USAGE}"
        ));
    }
    let verification =
        verify_static_world_winding(&args[1], &args[2]).map_err(|error| error.to_string())?;
    Ok(format!(
        "verified static-world winding: {} visual GLBs, {} triangles, {} scenes, resultSetBlake3={}, runtimeWorldBlake3={}",
        verification.visual_files,
        verification.triangles,
        verification.scene_files,
        verification.result_set_blake3,
        verification.runtime_world_blake3,
    ))
}

pub(super) fn run_static_world_winding_archive(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 2 {
        return Err(format!(
            "archive-repaired-tutorial-winding requires <PROJECT_ROOT>\n\n{USAGE}"
        ));
    }
    let report =
        archive_repaired_tutorial_winding_ownership(&args[1]).map_err(|error| error.to_string())?;
    Ok(format!(
        "{} repaired tutorial winding ownership: {} bytes, blake3={}, revisionPlanBlake3={}, archive={}",
        if report.already_archived {
            "verified archived"
        } else {
            "archived"
        },
        report.bytes,
        report.blake3,
        report.revision_plan_blake3,
        report.ownership_archive,
    ))
}

pub(super) fn run_tutorial_character_promotion(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 5 || args.len() > 6 {
        return Err(format!(
            "promote-tutorial-characters requires <ASSET_ROOT> <CANDIDATE_ROOT> <SOURCE_ROOT> <GPU_EVIDENCE_ROOT> and optional --apply\n\n{USAGE}"
        ));
    }
    let apply = match args.get(5).and_then(|arg| arg.to_str()) {
        None => false,
        Some("--apply") => true,
        Some(other) => {
            return Err(format!(
                "unknown promote-tutorial-characters argument {other:?}\n\n{USAGE}"
            ));
        }
    };
    let report = promote_tutorial_characters(
        &TutorialCharacterPromotionOptions::new(
            PathBuf::from(&args[1]),
            PathBuf::from(&args[2]),
            PathBuf::from(&args[3]),
            PathBuf::from(&args[4]),
        )
        .with_apply(apply),
    )
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "tutorial character promotion {:?}: ready={}, alreadyPromoted={}, blocked={}, files={}, bytes={}, registryModels={}->{}, manifestFiles={}->{}, applied={}, regenerateAssetIndex={}",
        report.mode,
        report.counts.ready,
        report.counts.already_promoted,
        report.counts.blocked,
        report.counts.promoted_files,
        report.counts.promoted_bytes,
        report.counts.registry_models_before,
        report.counts.registry_models_after,
        report.counts.manifest_files_before,
        report.counts.manifest_files_after,
        report.applied,
        report.requires_asset_index_regeneration,
    ))
}

pub(super) fn run_tutorial_prop_promotion(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 5 || args.len() > 6 {
        return Err(format!(
            "promote-tutorial-props requires <PROJECT_ROOT> <CANDIDATE_ROOT> <SOURCE_ROOT> <GPU_EVIDENCE_ROOT> and optional --apply\n\n{USAGE}"
        ));
    }
    let apply = match args.get(5).and_then(|arg| arg.to_str()) {
        None => false,
        Some("--apply") => true,
        Some(other) => {
            return Err(format!(
                "unknown promote-tutorial-props argument {other:?}\n\n{USAGE}"
            ));
        }
    };
    let report = promote_tutorial_props(
        &TutorialPropPromotionOptions::new(
            PathBuf::from(&args[1]),
            PathBuf::from(&args[2]),
            PathBuf::from(&args[3]),
            PathBuf::from(&args[4]),
        )
        .with_apply(apply),
    )
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "tutorial prop promotion {:?}: ready={}, alreadyPromoted={}, blocked={}, files={}, bytes={}, runtimeRefs={}, manifestFiles={}->{}, applied={}, regenerateAssetIndex={}",
        report.mode,
        report.counts.ready,
        report.counts.already_promoted,
        report.counts.blocked,
        report.counts.promoted_files,
        report.counts.promoted_bytes,
        report.counts.runtime_references_updated,
        report.counts.manifest_files_before,
        report.counts.manifest_files_after,
        report.applied,
        report.requires_asset_index_regeneration,
    ))
}

pub(super) fn run_equipment_gpu_acceptance_batch(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 6 {
        return Err(format!(
            "accept-equipment-gpu-batch requires <CANDIDATE_ROOT> <EVIDENCE_ROOT> <PREVIEW_EXE> <smoke|full> <NEW_REPORT_JSON>\n\n{USAGE}"
        ));
    }
    let mode = args[4]
        .to_str()
        .ok_or_else(|| "equipment GPU batch mode must be valid UTF-8".to_owned())
        .and_then(EquipmentGpuBatchMode::parse)?;
    let mut options = EquipmentGpuBatchOptions::new(
        PathBuf::from(&args[1]),
        PathBuf::from(&args[2]),
        PathBuf::from(&args[3]),
        mode,
        PathBuf::from(&args[5]),
    );
    let mut index = 6;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| "equipment GPU batch arguments must be valid UTF-8".to_owned())?;
        index += 1;
        let value = args.get(index).ok_or_else(|| {
            format!("accept-equipment-gpu-batch {flag} requires a value\n\n{USAGE}")
        })?;
        match flag {
            "--frames" => {
                options.max_frames = value
                    .to_str()
                    .ok_or_else(|| "--frames must be valid UTF-8".to_owned())?
                    .parse()
                    .map_err(|_| "--frames must be an unsigned integer".to_owned())?;
            }
            "--timeout" => {
                options.timeout_seconds = value
                    .to_str()
                    .ok_or_else(|| "--timeout must be valid UTF-8".to_owned())?
                    .parse()
                    .map_err(|_| "--timeout must be a number".to_owned())?;
            }
            "--shard" => {
                options.shard = Some(EquipmentGpuShard::parse(
                    value
                        .to_str()
                        .ok_or_else(|| "--shard must be valid UTF-8".to_owned())?,
                )?);
            }
            unknown => {
                return Err(format!(
                    "unknown accept-equipment-gpu-batch option {unknown:?}\n\n{USAGE}"
                ));
            }
        }
        index += 1;
    }
    let report = run_equipment_gpu_batch(&options).map_err(|error| error.to_string())?;
    Ok(format!(
        "equipment GPU {} batch: {}/{} selected standalone gates passed ({} executed, {} resumed, {} execution blockers); playerAttachmentParity=not-asserted; report={}",
        match report.mode {
            EquipmentGpuBatchMode::Smoke => "smoke",
            EquipmentGpuBatchMode::Full => "full",
        },
        report.counts.standalone_gpu_passed_models,
        report.counts.selected_models,
        report.counts.executed_models,
        report.counts.resumed_valid_models,
        report.counts.execution_blockers,
        options.report_path.display(),
    ))
}
