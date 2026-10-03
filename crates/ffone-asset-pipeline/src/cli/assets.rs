use super::*;

pub(super) fn run_asset_composition(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 6 {
        return Err(format!(
            "compose-assets requires <BASE_ASSET_ROOT> <OVERLAY_ASSET_ROOT> <FRESH_OUTPUT_ROOT> and at least one --overlay-prefix <RELATIVE_DIR/>\n\n{USAGE}"
        ));
    }
    let mut options = AssetCompositionOptions::new(&args[1], &args[2], &args[3]);
    let mut index = 4;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| "compose-assets arguments must be valid UTF-8".to_owned())?;
        if flag != "--overlay-prefix" {
            return Err(format!(
                "unknown compose-assets argument {flag:?}\n\n{USAGE}"
            ));
        }
        index += 1;
        let prefix = args
            .get(index)
            .ok_or_else(|| "--overlay-prefix requires a relative directory".to_owned())?
            .to_str()
            .ok_or_else(|| "overlay prefix must be valid UTF-8".to_owned())?;
        options = options.with_overlay_prefix(prefix);
        index += 1;
    }
    let output = options.output_root.clone();
    let report = compose_asset_roots(&options).map_err(|error| error.to_string())?;
    Ok(format!(
        "composed {} verified assets ({} bytes) into {}; baseRetained={}, baseReplaced={}, overlaySelected={}, conflicts={}, hardLinks={}, copies={}",
        report.output_files,
        report.output_bytes,
        output.display(),
        report.base_files_retained,
        report.base_files_replaced,
        report.overlay_files_selected,
        report.resolved_path_conflicts,
        report.hard_linked_files,
        report.copied_files,
    ))
}

pub(super) fn run_manifest_entry_refresh(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() != 3 {
        return Err(format!(
            "refresh-manifest-entry requires <ASSET_ROOT> <RELATIVE_PATH>\n\n{USAGE}"
        ));
    }
    let asset_root = PathBuf::from(&args[1]);
    let relative_path = args[2]
        .to_str()
        .ok_or_else(|| "RELATIVE_PATH must be valid UTF-8".to_owned())?;
    let refresh = refresh_project_asset_manifest_entry(&asset_root, relative_path)
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "manifest entry {}: changed={} bytes {} -> {} BLAKE3 {} -> {}",
        refresh.path,
        refresh.changed,
        refresh.previous_bytes,
        refresh.bytes,
        refresh.previous_blake3,
        refresh.blake3
    ))
}

pub(super) fn run_manifest_entry_registration(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args.len() != 4 {
        return Err(format!(
            "register-manifest-entry requires <ASSET_ROOT> <RELATIVE_PATH> <KIND>\n\n{USAGE}"
        ));
    }
    let asset_root = PathBuf::from(&args[1]);
    let relative_path = args[2]
        .to_str()
        .ok_or_else(|| "RELATIVE_PATH must be valid UTF-8".to_owned())?;
    let kind = match args[3]
        .to_str()
        .ok_or_else(|| "KIND must be valid UTF-8".to_owned())?
    {
        "model" => crate::ProjectAssetKind::Model,
        "texture" => crate::ProjectAssetKind::Texture,
        "audio" => crate::ProjectAssetKind::Audio,
        "font" => crate::ProjectAssetKind::Font,
        "data" => crate::ProjectAssetKind::Data,
        "shader" => crate::ProjectAssetKind::Shader,
        kind => return Err(format!("unsupported project asset kind {kind:?}")),
    };
    let registered = crate::register_project_asset_manifest_entry(
        &asset_root,
        relative_path,
        kind,
    )
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "registered manifest entry {}: kind={kind:?} bytes={} BLAKE3={}",
        registered.path, registered.bytes, registered.blake3
    ))
}

pub(super) fn run_object_route_normalization(args: &[std::ffi::OsString]) -> Result<String, String> {
    if !(args.len() == 3 || (args.len() == 4 && args[3] == "--apply")) {
        return Err(format!(
            "normalize-object-routes requires <PROJECT_ROOT> <REPORT_JSON> [--apply]\n\n{USAGE}"
        ));
    }
    let apply = args.len() == 4;
    let report = normalize_object_routes(&ObjectRouteNormalizationOptions::new(
        &args[1], &args[2], apply,
    ))
    .map_err(|error| error.to_string())?;
    Ok(format!(
        "{} {} reusable object packages / {} files at top-level objects/; max path {} -> {}, paths over 180 {} -> {}; report={}",
        if apply { "normalized" } else { "planned" },
        report.counts.packages,
        report.counts.files,
        report.path_lengths.maximum_before,
        report.path_lengths.maximum_after,
        report.path_lengths.over_180_before,
        report.path_lengths.over_180_after,
        PathBuf::from(&args[2]).display(),
    ))
}

pub(super) fn run_character_registry_alias_repair(args: &[std::ffi::OsString]) -> Result<String, String> {
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
            "repair-character-registry-aliases requires <ASSET_ROOT> <REPORT_JSON> [--apply]\n\n{USAGE}"
        ));
    }
    let report = repair_character_registry_aliases(&CharacterRegistryAliasRepairOptions::new(
        PathBuf::from(positional[0]),
        apply,
    ))
    .map_err(|error| error.to_string())?;
    let report_path = PathBuf::from(positional[1]);
    let serialized = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("failed to serialize repair report: {error}"))?;
    if let Some(parent) = report_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    std::fs::write(&report_path, serialized)
        .map_err(|error| format!("could not write {}: {error}", report_path.display()))?;
    Ok(format!(
        "character registry alias repair: models={}, canonical={}, alreadyAliased={}, tableSetMeshNames={}, changes={}, applied={} (report {})",
        report.models,
        report.canonical,
        report.already_aliased,
        report.table_set_mesh_names,
        report.changes.len(),
        report.applied,
        report_path.display()
    ))
}

pub(super) fn run_character_registry_alias_plan(args: &[std::ffi::OsString]) -> Result<String, String> {
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
            "apply-character-registry-alias-plan requires <ASSET_ROOT> <PLAN_JSON> <REPORT_JSON> [--apply]\n\n{USAGE}"
        ));
    }
    let report = apply_character_registry_alias_plan(&CharacterRegistryAliasPlanOptions::new(
        PathBuf::from(positional[0]),
        PathBuf::from(positional[1]),
        apply,
    ))
    .map_err(|error| error.to_string())?;
    let report_path = PathBuf::from(positional[2]);
    let serialized = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("failed to serialize alias plan report: {error}"))?;
    if let Some(parent) = report_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    std::fs::write(&report_path, serialized)
        .map_err(|error| format!("could not write {}: {error}", report_path.display()))?;
    Ok(format!(
        "character registry alias plan: planned={}, alreadyAliased={}, changes={}, applied={} (report {})",
        report.planned,
        report.already_aliased,
        report.changes.len(),
        report.applied,
        report_path.display()
    ))
}

pub(super) fn run_semantic_asset_plan(args: &[std::ffi::OsString]) -> Result<String, String> {
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
            "--asset-manifest"
                | "--content-index"
                | "--cook-report"
                | "--table-set"
                | "--logical-model-plan"
                | "--output"
        ) {
            return Err(format!(
                "unknown semantic-plan argument {flag:?}\n\n{USAGE}"
            ));
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
    let options = SemanticAssetOrganizerOptions::new(
        required("--asset-manifest")?,
        required("--content-index")?,
        required("--cook-report")?,
        required("--table-set")?,
        required("--logical-model-plan")?,
        required("--output")?,
    );
    let output = options.output.clone();
    let report = plan_semantic_assets(&options).map_err(|error| error.to_string())?;
    Ok(format!(
        "planned {} entities and {} ready logical models: eligible={}, blockers={}, productionAssetsMutated=false, report={}",
        report.counts.entities,
        report.counts.model_proposals,
        report.counts.eligible_model_proposals,
        report.counts.blockers,
        output.display()
    ))
}
