use super::*;

pub(super) fn apply_plan(
    project_root: &Path,
    asset_root: &Path,
    map_root: &Path,
    source_root: &Path,
    destination_root: &Path,
    catalog: &JsonValue,
    plan: &NormalizationPlan,
) -> Result<(String, WorldPrefabVerification)> {
    let marker = std::process::id();
    let object_stage = asset_root.join(format!(".objects-route-stage-{marker}"));
    let map_stage = asset_root.join(format!(".objects-map-stage-{marker}"));
    let map_backup = asset_root.join(format!(".objects-map-backup-{marker}"));
    let object_backup = asset_root.join(format!(".objects-route-backup-{marker}"));
    for path in [&object_stage, &map_stage, &map_backup, &object_backup] {
        if path.exists() {
            return invalid(format!(
                "stale normalization path exists: {}",
                path.display()
            ));
        }
    }
    fs::create_dir(&object_stage).map_err(|error| io_at(&object_stage, error))?;

    let stage_result = (|| -> Result<Vec<String>> {
        for (source, destination) in &plan.file_routes {
            if source.ends_with(".json") {
                continue;
            }
            let source_path = checked_join(asset_root, source)?;
            let destination_path = stage_object_path(&object_stage, destination)?;
            create_parent(&destination_path)?;
            if source.to_ascii_lowercase().ends_with(".glb") {
                let bytes =
                    rewrite_glb_uris(&source_path, destination, &plan.file_routes, asset_root)?;
                write_new(&destination_path, &bytes)?;
            } else if fs::hard_link(&source_path, &destination_path).is_err() {
                fs::copy(&source_path, &destination_path)
                    .map_err(|error| io_at(&destination_path, error))?;
            }
        }

        let mut json_routes = plan
            .file_routes
            .iter()
            .filter(|(source, _)| source.ends_with(".json"))
            .collect::<Vec<_>>();
        json_routes.sort_by_key(|(source, _)| source.ends_with("/set.json"));
        for (source, destination) in json_routes {
            let source_path = checked_join(asset_root, source)?;
            let mut value = read_json(&source_path, "object JSON")?;
            replace_paths(&mut value, &plan.file_routes);
            refresh_artifacts(&mut value, asset_root, &object_stage, &map_stage)?;
            let destination_path = stage_object_path(&object_stage, destination)?;
            write_new(&destination_path, &pretty_json(&value)?)?;
        }

        stage_map_json(
            asset_root,
            map_root,
            &object_stage,
            &map_stage,
            catalog,
            &plan.file_routes,
        )
    })();
    let map_files = match stage_result {
        Ok(files) => files,
        Err(error) => {
            let _ = fs::remove_dir_all(&object_stage);
            let _ = fs::remove_dir_all(&map_stage);
            return Err(error);
        }
    };

    fs::create_dir(&map_backup).map_err(|error| io_at(&map_backup, error))?;
    for relative in &map_files {
        let source = checked_join(asset_root, relative)?;
        let backup = checked_join(&map_backup, relative)?;
        create_parent(&backup)?;
        fs::copy(&source, &backup).map_err(|error| io_at(&backup, error))?;
    }

    fs::rename(source_root, &object_backup).map_err(|error| io_at(source_root, error))?;
    if let Err(error) = fs::rename(&object_stage, destination_root) {
        let _ = fs::rename(&object_backup, source_root);
        return Err(io_at(destination_root, error));
    }
    let commit_result = (|| -> Result<()> {
        for relative in &map_files {
            let staged = checked_join(&map_stage, relative)?;
            let live = checked_join(asset_root, relative)?;
            let bytes = read_file(&staged, "staged map JSON")?;
            write_replace(&live, &bytes)?;
        }
        Ok(())
    })();
    if let Err(error) = commit_result {
        rollback(
            asset_root,
            source_root,
            destination_root,
            &object_backup,
            &map_backup,
            &map_files,
        );
        return Err(error);
    }

    let verification =
        match crate::world_prefab_organizer::verify_world_prefab_library(project_root) {
            Ok(verification) => verification,
            Err(error) => {
                rollback(
                    asset_root,
                    source_root,
                    destination_root,
                    &object_backup,
                    &map_backup,
                    &map_files,
                );
                return Err(error);
            }
        };
    let catalog_bytes = read_file(&map_root.join("catalog.json"), "normalized map catalog")?;
    fs::remove_dir_all(&object_backup).map_err(|error| io_at(&object_backup, error))?;
    fs::remove_dir_all(&map_stage).map_err(|error| io_at(&map_stage, error))?;
    fs::remove_dir_all(&map_backup).map_err(|error| io_at(&map_backup, error))?;
    Ok((hash(&catalog_bytes), verification))
}

pub(super) fn refresh_artifacts(
    value: &mut JsonValue,
    asset_root: &Path,
    object_stage: &Path,
    map_stage: &Path,
) -> Result<()> {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                refresh_artifacts(value, asset_root, object_stage, map_stage)?;
            }
        }
        JsonValue::Object(values) => {
            if let Some(path) = values
                .get("path")
                .and_then(JsonValue::as_str)
                .map(str::to_owned)
            {
                let staged = if path.starts_with("objects/") {
                    stage_object_path(object_stage, &path)?
                } else {
                    checked_join(map_stage, &path)?
                };
                let absolute = if staged.is_file() {
                    staged
                } else {
                    checked_join(asset_root, &path)?
                };
                if absolute.is_file() {
                    let bytes = read_file(&absolute, "referenced artifact")?;
                    if values.contains_key("bytes") {
                        values.insert("bytes".to_owned(), JsonValue::from(bytes.len() as u64));
                    }
                    if values.contains_key("blake3") {
                        values.insert("blake3".to_owned(), JsonValue::String(hash(&bytes)));
                    }
                }
            }
            for child in values.values_mut() {
                refresh_artifacts(child, asset_root, object_stage, map_stage)?;
            }
        }
        _ => {}
    }
    Ok(())
}
