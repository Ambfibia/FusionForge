use super::*;

pub(super) fn publish_native_logical_model(args: &[String]) -> Result<(), String> {
    if args.len() < 3 || (args.len() - 3) % 2 != 0 {
        return Err("publish-logical-model <source.json> <family> <Editor-stage-root> [--reuse-texture-index <index.json>] [--semantic-directory <name>] [--texture-rebinds <reviewed.json>]".into());
    }
    let mut options =
        ffone_asset_pipeline::LogicalModelPublishOptions::new(&args[0], &args[1], &args[2])
            .with_semantic_root_layout();
    for pair in args[3..].chunks_exact(2) {
        match pair[0].as_str() {
            "--reuse-texture-index" => options.reuse_texture_index = Some(PathBuf::from(&pair[1])),
            "--semantic-directory" => options.semantic_directories.push(pair[1].clone()),
            "--texture-rebinds" => options = options.with_reviewed_texture_rebinds(&pair[1]),
            other => return Err(format!("unknown publish-logical-model option {other}")),
        }
    }
    let report =
        ffone_asset_pipeline::publish_logical_model(&options).map_err(|e| e.to_string())?;
    let audit = ffone_asset_pipeline::audit_logical_model_tree(&options.output_root)
        .map_err(|e| e.to_string())?;
    if !audit.passed {
        return Err(format!(
            "staged native model failed tree audit: {:?}",
            audit.violations
        ));
    }
    println!("Staged {} with {} materials / {} textures; structural audit passed, runtime visual acceptance pending", report.contract.output_glb, report.material_publish.material_count, report.material_publish.texture_count);
    Ok(())
}

/// Recover, convert and validate before writing final runtime files.
pub(super) fn convert_native_model(args: &[String]) -> Result<(), String> {
    const USAGE: &str = "convert-native-model <bundle> <exact-container-route> <family> <native-output-root>";
    if args.len() != 4 { return Err(USAGE.into()); }
    let bundle = Path::new(&args[0]).canonicalize().map_err(|e| e.to_string())?;
    let output = super::super::workspace::resolve_destination(Path::new(&args[3]))?;
    if output.starts_with(bundle.parent().ok_or("bundle has no parent")?) {
        return Err("native output must be outside the source build".into());
    }
    let source = crate::preview_bundle_container_model_exact(args[0].clone(), None, args[1].clone())?;
    let bytes = serde_json::to_vec(&source).map_err(|e| e.to_string())?;
    let options = ffone_asset_pipeline::LogicalModelPublishOptions::new("in-memory", &args[2], output).with_semantic_root_layout();
    let report = ffone_asset_pipeline::convert_logical_model_bytes(&options, &bytes).map_err(|e| e.to_string())?;
    println!("Converted {} ({} materials, {} textures); visual runtime verification remains separate", report.contract.output_glb, report.material_publish.material_count, report.material_publish.texture_count);
    Ok(())
}

pub(super) fn plan_logical_model_exports(args: &[String]) -> Result<(), String> {
    let mut positional = Vec::new();
    let mut requested_kfm_route = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--kfm-route" {
            index += 1;
            let route = args
                .get(index)
                .ok_or_else(|| "--kfm-route requires one exact KFM container route".to_string())?;
            if requested_kfm_route.replace(route.clone()).is_some() {
                return Err("--kfm-route may be supplied only once".to_string());
            }
        } else if let Some(route) = args[index].strip_prefix("--kfm-route=") {
            if route.is_empty() {
                return Err("--kfm-route requires a non-empty route".to_string());
            }
            if requested_kfm_route.replace(route.to_string()).is_some() {
                return Err("--kfm-route may be supplied only once".to_string());
            }
        } else {
            positional.push(args[index].clone());
        }
        index += 1;
    }
    if !(1..=2).contains(&positional.len()) {
        return Err("plan-logical-model-exports <bundle-index.json> [out.json|-] [--kfm-route <exact-route>]".to_string());
    }
    let input_path = Path::new(&positional[0]);
    let plan = super::super::logical_model_export_plan::plan_logical_model_exports_from_path(
        input_path,
        super::super::logical_model_export_plan::LogicalModelExportPlanOptions {
            requested_kfm_route,
        },
    )?;
    let output = format!(
        "{}\n",
        serde_json::to_string_pretty(&plan).map_err(|err| err.to_string())?
    );
    match positional.get(1).map(String::as_str) {
        None | Some("-") => {
            print!("{output}");
            Ok(())
        }
        Some(output_path) => {
            let output_path = Path::new(output_path);
            if output_path == input_path {
                return Err("export plan must not overwrite bundle-index.json".to_string());
            }
            fs::write(output_path, output).map_err(|err| {
                format!(
                    "could not write logical-model export plan {}: {err}",
                    output_path.display()
                )
            })
        }
    }
}

pub(super) fn preview_container_model(args: &[String]) -> Result<(), String> {
    let bundle = required_path(
        args,
        0,
        "preview-container-model <bundle> <container-route> <out.json> [work-dir]",
    )?;
    let route = args
        .get(1)
        .cloned()
        .ok_or_else(|| "preview-container-model requires a container route".to_string())?;
    let output = required_path(
        args,
        2,
        "preview-container-model <bundle> <container-route> <out.json> [work-dir]",
    )?;
    let project = args.get(3).cloned();
    if args.len() > 4 {
        return Err(
            "preview-container-model accepts at most one optional work directory".to_string(),
        );
    }
    let preview = crate::preview_bundle_container_model(
        bundle.to_string_lossy().into_owned(),
        project,
        vec![route],
    )?;
    write_or_print_json(Some(&output), &preview)
}

pub(super) fn export_logical_model_source(args: &[String]) -> Result<(), String> {
    let bundle = required_path(
        args,
        0,
        "export-logical-model-source <bundle> <exact-container-route> <out.json> [work-dir]",
    )?;
    let route = args.get(1).cloned().ok_or_else(|| {
        "export-logical-model-source requires an exact container route".to_string()
    })?;
    let output = required_path(
        args,
        2,
        "export-logical-model-source <bundle> <exact-container-route> <out.json> [work-dir]",
    )?;
    let project = args.get(3).cloned();
    if args.len() > 4 {
        return Err(
            "export-logical-model-source accepts at most one optional work directory"
                .to_string(),
        );
    }
    let source = crate::preview_bundle_container_model_exact(
        bundle.to_string_lossy().into_owned(),
        project,
        route,
    )?;
    write_or_print_json(Some(&output), &source)
}

pub(super) fn export_logical_model_source_batch(args: &[String]) -> Result<(), String> {
    const USAGE: &str = "export-logical-model-source-batch <bundle> <work-dir> <exact-container-route> <out.json> [<exact-container-route> <out.json> ...]";
    if args.len() < 4 || (args.len() - 2) % 2 != 0 {
        return Err(USAGE.to_string());
    }
    let bundle = PathBuf::from(&args[0]);
    let project = PathBuf::from(&args[1]);
    let exports = args[2..]
        .chunks_exact(2)
        .map(|pair| (pair[0].clone(), PathBuf::from(&pair[1])))
        .collect::<Vec<_>>();
    let routes = exports
        .iter()
        .map(|(route, _)| route.clone())
        .collect::<Vec<_>>();

    crate::prewarm_exact_logical_model_environment(&bundle.to_string_lossy(), &project, &routes)?;
    let mut failures = Vec::new();
    for (route, output) in exports {
        let result = crate::preview_bundle_container_model_exact(
            bundle.to_string_lossy().into_owned(),
            Some(project.to_string_lossy().into_owned()),
            route.clone(),
        )
        .and_then(|source| write_or_print_json(Some(&output), &source));
        if let Err(error) = result {
            failures.push(format!("{route}: {error}"));
        }
    }
    if !failures.is_empty() {
        return Err(format!(
            "{} exact logical-model export(s) failed:\n{}",
            failures.len(),
            failures.join("\n")
        ));
    }
    Ok(())
}

pub(super) fn export_equipment_model_sources(args: &[String]) -> Result<(), String> {
    if args.len() != 3 && args.len() != 5 {
        return Err(
            "export-equipment-model-sources <work-dir|cache/bundle-index.json> <semantic-plan.json> <fresh-source-root> [--limit N]"
                .to_string(),
        );
    }
    let limit = if args.len() == 5 {
        if args[3] != "--limit" {
            return Err("the only optional equipment export flag is --limit N".to_string());
        }
        Some(
            args[4]
                .parse::<usize>()
                .map_err(|_| "--limit must be a positive integer".to_string())?,
        )
    } else {
        None
    };
    let manifest =
        super::super::equipment_model_batch_export::export_equipment_model_sources_batch_limited(
            Path::new(&args[0]),
            Path::new(&args[1]),
            Path::new(&args[2]),
            limit,
        )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&manifest).map_err(|err| err.to_string())?
    );
    Ok(())
}

pub(super) fn export_logical_model_sources(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err(
            "export-logical-model-sources <work-dir|cache/bundle-index.json> <fresh-source-root>"
                .to_string(),
        );
    }
    let manifest = super::super::logical_model_batch_export::export_logical_model_sources_batch(
        Path::new(&args[0]),
        Path::new(&args[1]),
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&manifest).map_err(|err| err.to_string())?
    );
    Ok(())
}

pub(super) fn audit_logical_model_root_transforms(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err(
            "audit-logical-model-root-transforms <logical-model-plan.json> <report.json>"
                .to_string(),
        );
    }
    let report = super::super::logical_model_root_transform_audit::audit_logical_model_root_transforms(
        Path::new(&args[0]),
        Path::new(&args[1]),
    )?;
    println!(
        "audited {}/{} logical-model roots: unit={}, nonunit={}, nonzero-origin={}, errors={}",
        report.counts.audited_roots,
        report.counts.planned_ready_roots,
        report.counts.unit_scale_roots,
        report.counts.nonunit_scale_roots,
        report.counts.nonzero_origin_roots,
        report.counts.errors
    );
    if report.passed() {
        Ok(())
    } else {
        Err(format!(
            "logical-model root-transform audit failed; complete report was written to {}",
            args[1]
        ))
    }
}

pub(super) fn replace_mesh(args: &[String]) -> Result<(), String> {
    let input = required_path(
        args,
        0,
        "replace-mesh <asset-or-bundle> <path-id> <mesh.obj|mesh.glb|mesh.gltf> <output-asset> [name]",
    )?;
    let path_id = args
        .get(1)
        .ok_or_else(|| "replace-mesh requires a path id".to_string())
        .and_then(|value| parse_i64(value))?;
    let mesh_path = required_path(
        args,
        2,
        "replace-mesh <asset-or-bundle> <path-id> <mesh.obj|mesh.glb|mesh.gltf> <output-asset> [name]",
    )?;
    let output = required_path(
        args,
        3,
        "replace-mesh <asset-or-bundle> <path-id> <mesh.obj|mesh.glb|mesh.gltf> <output-asset> [name]",
    )?;
    let name = args.get(4).cloned();

    let loaded = load_input(&input)?;
    let (asset_index, asset, info) = find_object(&loaded.env, path_id)?;
    if asset.object_type_name(info) != "Mesh" {
        return Err(format!(
            "{}#{} is {}, not Mesh",
            asset.name,
            path_id,
            asset.object_type_name(info)
        ));
    }
    let imported = ImportedMesh::from_model_path(&mesh_path, name)?;
    let mut body = asset.read_object(asset_index, info)?;
    apply_mesh_import(&mut body, imported)?;
    let mut replacements = std::collections::BTreeMap::new();
    replacements.insert(path_id, body);
    let rebuilt = asset.rebuild_with_object_values(asset_index, &replacements)?;
    fs::write(&output, rebuilt).map_err(|err| err.to_string())?;
    println!("Wrote rebuilt asset to {}", output.display());
    Ok(())
}
