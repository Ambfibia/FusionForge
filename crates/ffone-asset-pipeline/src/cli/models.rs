use super::*;

pub(super) fn run_tutorial_model_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 4 {
        return Err(format!(
            "install-tutorial-models requires <CANDIDATE_ROOT> <ASSET_ROOT> <SOURCE_BUILD>, at least one --evidence-root <DIR>, and at least one --model <models/.../*.glb>\n\n{USAGE}"
        ));
    }
    let source_build = args[3]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let mut options = TutorialModelInstallOptions::new(&args[1], &args[2], source_build);
    let mut evidence_roots = 0usize;
    let mut models = 0usize;
    let mut index = 4;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| "install-tutorial-models arguments must be valid UTF-8".to_owned())?;
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| format!("{flag} requires a value\n\n{USAGE}"))?;
        match flag {
            "--evidence-root" => {
                options = options.with_evidence_root(PathBuf::from(value));
                evidence_roots += 1;
            }
            "--model" => {
                let model = value
                    .to_str()
                    .ok_or_else(|| "--model must be valid UTF-8".to_owned())?;
                options = options.with_model(model);
                models += 1;
            }
            unknown => {
                return Err(format!(
                    "unknown install-tutorial-models argument {unknown:?}\n\n{USAGE}"
                ));
            }
        }
        index += 1;
    }
    if evidence_roots == 0 || models == 0 {
        return Err(format!(
            "install-tutorial-models requires at least one --evidence-root and at least one --model\n\n{USAGE}"
        ));
    }
    let report = install_tutorial_models(&options).map_err(|error| error.to_string())?;
    Ok(format!(
        "installed {} exact GPU-passed tutorial models as {} owned assets ({} bytes); replacedPreviousInstall={}, manifestFiles={}, catalog={}",
        report.installed_models,
        report.installed_files,
        report.installed_bytes,
        report.replaced_previous_install,
        report.manifest_files,
        crate::TUTORIAL_MODEL_CATALOG_PATH,
    ))
}

pub(super) fn run_tutorial_character_model_dedupe(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 2 || args.len() > 3 {
        return Err(format!(
            "dedupe-tutorial-character-models requires <ASSET_ROOT> and optional --apply\n\n{USAGE}"
        ));
    }
    let apply = match args.get(2).and_then(|arg| arg.to_str()) {
        None => false,
        Some("--apply") => true,
        Some(other) => {
            return Err(format!(
                "unknown dedupe-tutorial-character-models argument {other:?}\n\n{USAGE}"
            ));
        }
    };
    let report = dedupe_tutorial_character_models(
        &TutorialCharacterModelDedupeOptions::new(PathBuf::from(&args[1])).with_apply(apply),
    )
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "tutorial character-model dedupe {:?}: provenPackages={}, removedPackages={}, alreadyDeduplicatedPackages={}, removedFiles={}, removedBytes={}, manifestFiles={}->{}, regenerateAssetIndex={}",
        report.mode,
        report.counts.proven_packages,
        report.counts.removed_packages,
        report.counts.already_deduplicated_packages,
        report.counts.removed_files,
        report.counts.removed_bytes,
        report.counts.manifest_files_before,
        report.counts.manifest_files_after,
        report.requires_asset_index_regeneration,
    ))
}

pub(super) fn run_runtime_character_model_replace(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 4 {
        return Err(format!(
            "replace-runtime-character-model requires <ASSET_ROOT> <RELATIVE_GLB> <REPLACEMENT_GLB>\n\n{USAGE}"
        ));
    }
    let relative_glb = args[2]
        .to_str()
        .ok_or_else(|| "RELATIVE_GLB must be valid UTF-8".to_owned())?;
    let report = replace_runtime_character_model(&RuntimeCharacterModelReplaceOptions::new(
        PathBuf::from(&args[1]),
        relative_glb,
        PathBuf::from(&args[3]),
    ))
    .map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize replacement report: {error}"))
}

pub(super) fn run_runtime_character_model_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 5 {
        return Err(format!(
            "install-runtime-character-model requires <ASSET_ROOT> <ID> <CANDIDATE_GLB> <GPU_EVIDENCE_JSON>\n\n{USAGE}"
        ));
    }
    let id = args[2]
        .to_str()
        .ok_or_else(|| "ID must be valid UTF-8".to_owned())?;
    let report = install_runtime_character_model(&RuntimeCharacterModelInstallOptions::new(
        PathBuf::from(&args[1]),
        id,
        PathBuf::from(&args[3]),
        PathBuf::from(&args[4]),
    ))
    .map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize installation report: {error}"))
}

pub(super) fn run_runtime_character_model_rename(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    let apply = args.iter().any(|arg| arg == "--apply");
    let positional = args
        .iter()
        .skip(1)
        .filter(|arg| *arg != "--apply")
        .collect::<Vec<_>>();
    if positional.len() != 4 {
        return Err(format!(
            "rename-runtime-character-model requires <ASSET_ROOT> <ID> <NEW_ID> <REPORT_JSON> [--apply]\n\n{USAGE}"
        ));
    }
    let report = rename_runtime_character_model(&RuntimeCharacterModelRenameOptions::new(
        PathBuf::from(positional[0]),
        positional[1]
            .to_str()
            .ok_or_else(|| "id must be valid UTF-8".to_owned())?,
        positional[2]
            .to_str()
            .ok_or_else(|| "new id must be valid UTF-8".to_owned())?,
        apply,
    ))
    .map_err(|error| error.to_string())?;
    write_lifecycle_report(&report, &PathBuf::from(positional[3]))
}

pub(super) fn run_runtime_character_model_remove(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    let apply = args.iter().any(|arg| arg == "--apply");
    let positional = args
        .iter()
        .skip(1)
        .filter(|arg| *arg != "--apply")
        .collect::<Vec<_>>();
    if positional.len() != 3 {
        return Err(format!(
            "remove-runtime-character-model requires <ASSET_ROOT> <ID> <REPORT_JSON> [--apply]\n\n{USAGE}"
        ));
    }
    let report = remove_runtime_character_model(&RuntimeCharacterModelRemoveOptions::new(
        PathBuf::from(positional[0]),
        positional[1]
            .to_str()
            .ok_or_else(|| "id must be valid UTF-8".to_owned())?,
        apply,
    ))
    .map_err(|error| error.to_string())?;
    write_lifecycle_report(&report, &PathBuf::from(positional[2]))
}

pub(super) fn run_avatar_item_model_refresh(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    let apply = args.iter().any(|arg| arg == "--apply");
    let positional = args
        .iter()
        .skip(1)
        .filter(|arg| *arg != "--apply")
        .collect::<Vec<_>>();
    if positional.len() != 2 {
        return Err(format!(
            "refresh-avatar-item-models requires <ASSET_ROOT> <REPORT_JSON> [--apply]\n\n{USAGE}"
        ));
    }
    let report = refresh_avatar_item_models(&AvatarItemModelRefreshOptions::new(
        PathBuf::from(positional[0]),
        apply,
    ))
    .map_err(|error| error.to_string())?;
    let report_path = PathBuf::from(positional[1]);
    let serialized = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("failed to serialize refresh report: {error}"))?;
    if let Some(parent) = report_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    std::fs::write(&report_path, serialized)
        .map_err(|error| format!("could not write {}: {error}", report_path.display()))?;
    Ok(format!(
        "avatar item models: missingBefore={}, withoutSourceModel={}, resolved={}, resolvedModels {}->{}, applied={} (report {})",
        report.missing_slots_before,
        report.slots_without_a_source_model,
        report.changes.len(),
        report.resolved_models_before,
        report.resolved_models_after,
        report.applied,
        report_path.display()
    ))
}

pub(super) fn run_player_item_model_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    let apply = args.iter().any(|arg| arg == "--apply");
    let positional = args
        .iter()
        .skip(1)
        .filter(|arg| *arg != "--apply")
        .collect::<Vec<_>>();
    if positional.len() != 5 {
        return Err(format!(
            "install-player-item-models requires <CANDIDATE_ROOT> <GPU_BATCH_JSON> <ASSET_ROOT> <SOURCE_BUILD> <REPORT_JSON> [--apply]\n\n{USAGE}"
        ));
    }
    let source_build = positional[3]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let report = install_player_item_models(&PlayerItemModelInstallOptions::new(
        PathBuf::from(positional[0]),
        PathBuf::from(positional[1]),
        PathBuf::from(positional[2]),
        source_build,
        apply,
    ))
    .map_err(|error| error.to_string())?;
    let report_path = PathBuf::from(positional[4]);
    let serialized = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("failed to serialize install report: {error}"))?;
    if let Some(parent) = report_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    std::fs::write(&report_path, serialized)
        .map_err(|error| format!("could not write {}: {error}", report_path.display()))?;
    Ok(format!(
        "player item models: installed={}, files={}, bytes={}, sets {}->{}, models {}->{}, applied={} (report {})",
        report.installed.len(),
        report.installed_files,
        report.installed_bytes,
        report.catalog_sets_before,
        report.catalog_sets_after,
        report.catalog_models_before,
        report.catalog_models_after,
        report.applied,
        report_path.display()
    ))
}

pub(super) fn run_logical_model_publish(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 4 || (args.len() - 4) % 2 != 0 {
        return Err(format!(
            "publish-logical-model requires <SOURCE_JSON> <FAMILY> <OUTPUT_ROOT> [--texture-rebinds <JSON>] [--reuse-texture-index <JSON>] [--semantic-directory <NAME>]\n\n{USAGE}"
        ));
    }
    let family = args[2]
        .to_str()
        .ok_or_else(|| "FAMILY must be valid UTF-8".to_owned())?;
    let mut options =
        LogicalModelPublishOptions::new(PathBuf::from(&args[1]), family, PathBuf::from(&args[3]));
    for pair in args[4..].chunks_exact(2) {
        match pair[0].to_str() {
            Some("--texture-rebinds") => {
                options = options.with_reviewed_texture_rebinds(PathBuf::from(&pair[1]))
            }
            Some("--reuse-texture-index") => {
                options.reuse_texture_index = Some(PathBuf::from(&pair[1]))
            }
            Some("--semantic-directory") => options.semantic_directories.push(
                pair[1]
                    .to_str()
                    .ok_or("semantic directory must be UTF-8")?
                    .to_owned(),
            ),
            _ => {
                return Err(format!(
                    "unknown publish-logical-model option {:?}",
                    pair[0]
                ));
            }
        }
    }
    let output_root = options.output_root.clone();
    let report = publish_logical_model(&options).map_err(|error| error.to_string())?;
    let audit = audit_logical_model_tree(&output_root).map_err(|error| error.to_string())?;
    if !audit.passed {
        return Err(format!(
            "published candidate failed logical-model tree audit with {} violations",
            audit.violations.len()
        ));
    }
    Ok(format!(
        "staged logical model {:?}: GLB={}, report={}, materialPublish={}, publishable={}, treeAudit=passed/gpu-pending (output root {})",
        report.contract.legacy_name,
        report.contract.output_glb,
        report.report_path,
        report.material_publish.status,
        report.publishable,
        output_root.display()
    ))
}

pub(super) fn run_logical_model_batch_publish(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 3 {
        return Err(format!(
            "publish-logical-model-batch requires <SOURCE_ROOT> <FRESH_OUTPUT_ROOT>\n\n{USAGE}"
        ));
    }
    let output_root = PathBuf::from(&args[2]);
    let report = publish_logical_model_batch(&LogicalModelBatchPublishOptions::new(
        PathBuf::from(&args[1]),
        &output_root,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "atomically published {} logical models ({} GLBs, {} PNGs, {} families) into {}; structuralAudit=passed, gpuGatePending={}",
        report.counts.sources,
        report.counts.glbs,
        report.counts.pngs,
        report.counts.families,
        output_root.display(),
        report.gpu_gate_pending
    ))
}

pub(super) fn run_equipment_logical_model_batch_publish(
    args: &[std::ffi::OsString],
) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 3 {
        return Err(format!(
            "publish-equipment-logical-model-batch requires <SOURCE_ROOT> <FRESH_OUTPUT_ROOT>\n\n{USAGE}"
        ));
    }
    let output_root = PathBuf::from(&args[2]);
    let report = publish_equipment_logical_model_batch(
        &EquipmentLogicalModelBatchPublishOptions::new(PathBuf::from(&args[1]), &output_root),
    )
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "atomically published {} native equipment models ({} GLBs, {} PNGs, {} typed blockers) into {}; structuralAudit=passed, productionAssetsMutated={}",
        report.counts.published_models,
        report.counts.glbs,
        report.counts.pngs,
        report.counts.total_blockers,
        output_root.display(),
        report.production_assets_mutated
    ))
}

pub(super) fn run_logical_model_tree_audit(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if !(2..=3).contains(&args.len()) {
        return Err(format!(
            "audit-logical-model-tree requires <OUTPUT_ROOT> [REPORT_JSON]\n\n{USAGE}"
        ));
    }
    let output_root = PathBuf::from(&args[1]);
    let report = audit_logical_model_tree(&output_root).map_err(|error| error.to_string())?;
    if let Some(output) = args.get(2).map(PathBuf::from) {
        if output.exists() {
            return Err(format!(
                "audit report output is never overwritten: {}",
                output.display()
            ));
        }
        if let Some(parent) = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)
            .map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    if !report.passed {
        return Err(format!(
            "logical-model tree audit failed: {} root violations across {} GLBs",
            report.violations.len(),
            report.counts.glbs
        ));
    }
    Ok(format!(
        "audited {} logical GLBs and {} PNGs: passed=true, gpuGatePending={}",
        report.counts.glbs, report.counts.pngs, report.gpu_gate_pending
    ))
}

pub(super) fn run_logical_model_gpu_evidence_audit(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if !(3..=4).contains(&args.len()) {
        return Err(format!(
            "audit-logical-model-gpu-evidence requires <CANDIDATE_ROOT> <EVIDENCE_ROOT> [REPORT_JSON]\n\n{USAGE}"
        ));
    }
    let candidate_root = PathBuf::from(&args[1]);
    let evidence_root = PathBuf::from(&args[2]);
    let report = audit_logical_model_gpu_evidence(&candidate_root, &evidence_root)
        .map_err(|error| error.to_string())?;
    if let Some(output) = args.get(3).map(PathBuf::from) {
        if output.exists() {
            return Err(format!(
                "GPU evidence audit report is never overwritten: {}",
                output.display()
            ));
        }
        if let Some(parent) = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)
            .map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    if !report.automated_gpu_passed {
        return Err(format!(
            "logical-model GPU evidence audit failed: {} violations across {} candidate GLBs",
            report.violations.len(),
            report.counts.candidate_glbs
        ));
    }
    Ok(format!(
        "audited immutable GPU evidence for {} logical GLBs: automatedGpuPassed=true, visualParityPending={}, publishable={}",
        report.counts.matched_models, report.visual_parity_pending, report.publishable
    ))
}

pub(super) fn run_model_audit(args: &[std::ffi::OsString]) -> Result<String, String> {
    let mut values = std::collections::BTreeMap::<&str, PathBuf>::new();
    let mut index = 1;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| "arguments must be valid UTF-8".to_owned())?;
        if flag == "--help" || flag == "-h" {
            return Ok(USAGE.to_owned());
        }
        if !matches!(
            flag,
            "--assets" | "--cook-report" | "--layout-report" | "--table-set" | "--output"
        ) {
            return Err(format!("unknown audit argument {flag:?}\n\n{USAGE}"));
        }
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| format!("{flag} requires a path"))?;
        if values.insert(flag, PathBuf::from(value)).is_some() {
            return Err(format!("{flag} may only be supplied once"));
        }
        index += 1;
    }
    let mut required = |flag| {
        values
            .remove(flag)
            .ok_or_else(|| format!("{flag} <PATH> is required\n\n{USAGE}"))
    };
    let options = ModelAuditOptions {
        assets: required("--assets")?,
        cook_report: required("--cook-report")?,
        layout_report: required("--layout-report")?,
        table_set: required("--table-set")?,
        output: required("--output")?,
    };
    let output = options.output.clone();
    let report = audit_models(&options).map_err(|error| error.to_string())?;
    Ok(format!(
        "audited {} GLBs: status={}, report={}",
        report.glb.models,
        report.status,
        output.display()
    ))
}
