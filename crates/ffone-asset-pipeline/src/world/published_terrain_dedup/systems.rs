use super::*;

pub(super) fn apply_plan(
    project_root: &Path,
    asset_root: &Path,
    map_root: &Path,
    plan: &MigrationPlan,
) -> Result<PublishedTerrainDedupVerification> {
    let lock = acquire_lock(asset_root)?;
    let stage = asset_root.join(STAGE_DIRECTORY);
    let backup = asset_root.join(BACKUP_DIRECTORY);
    for path in [&stage, &backup] {
        if path.exists() {
            return invalid(format!(
                "stale terrain dedup path exists: {}",
                path.display()
            ));
        }
    }
    verify_source_catalog(map_root, &plan.source_catalog_blake3)?;
    clone_tree(map_root, &stage)?;
    let stage_result = (|| -> Result<()> {
        for route in &plan.removals {
            let relative = map_relative(route)?;
            let target = checked_join(&stage, relative)?;
            if !target.is_file() {
                return invalid(format!("planned duplicate is absent from stage: {route}"));
            }
            fs::remove_file(&target).map_err(|error| io_at(&target, error))?;
        }
        for (route, bytes) in &plan.replacements {
            replace_staged_file(&stage, route, bytes, true)?;
        }
        for (route, bytes) in &plan.additions {
            replace_staged_file(&stage, route, bytes, false)?;
        }
        validate_migrated_map(asset_root, &stage, &plan.result_catalog_blake3)
    })();
    if let Err(error) = stage_result {
        let _ = remove_transaction_directory(asset_root, &stage, STAGE_DIRECTORY);
        return Err(error);
    }
    verify_source_catalog(map_root, &plan.source_catalog_blake3)?;
    fs::rename(map_root, &backup).map_err(|error| io_at(map_root, error))?;
    if let Err(error) = fs::rename(&stage, map_root) {
        let _ = fs::rename(&backup, map_root);
        return Err(io_at(map_root, error));
    }
    let verification = (|| -> Result<PublishedTerrainDedupVerification> {
        validate_migrated_map(asset_root, map_root, &plan.result_catalog_blake3)?;
        let mut verification = verify_physical_library(asset_root, map_root, false)?;
        verification.scene_links = validate_all_scene_links(asset_root, map_root)?;
        Ok(verification)
    })();
    let verification = match verification {
        Ok(value) => value,
        Err(error) => {
            let failed = asset_root.join(STAGE_DIRECTORY);
            let rollback =
                fs::rename(map_root, &failed).and_then(|()| fs::rename(&backup, map_root));
            if let Err(rollback_error) = rollback {
                return invalid(format!(
                    "terrain migration verification failed ({error}); automatic rollback also failed ({rollback_error}); recovery paths were preserved"
                ));
            }
            let _ = remove_transaction_directory(asset_root, &failed, STAGE_DIRECTORY);
            return Err(error);
        }
    };
    remove_transaction_directory(asset_root, &backup, BACKUP_DIRECTORY)?;
    drop(lock);
    let _ = project_root;
    Ok(verification)
}
