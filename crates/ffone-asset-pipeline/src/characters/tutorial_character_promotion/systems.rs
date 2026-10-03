use super::*;

pub(super) fn apply_plan(plan: &PromotionPlan) -> Result<()> {
    if read_regular(&plan.manifest_path)? != plan.manifest_before
        || read_regular(&plan.registry_path)? != plan.registry_before
    {
        return invalid("manifest or character registry changed after promotion planning");
    }
    let stage = plan.asset_root.join(format!(
        ".tutorial-character-promotion-{}-{}",
        std::process::id(),
        unique_stamp()
    ));
    fs::create_dir(&stage).map_err(|source| io_at(&stage, source))?;
    let stage_packages = stage.join("packages");
    let stage_backup = stage.join("backup");
    fs::create_dir(&stage_packages).map_err(|source| io_at(&stage_packages, source))?;
    fs::create_dir(&stage_backup).map_err(|source| io_at(&stage_backup, source))?;

    let prepare = (|| -> Result<()> {
        for promotion in &plan.promotions {
            let package = stage_packages.join(promotion.candidate.id);
            for source in &promotion.source_files {
                let source_root = plan.asset_root.join(native_path(&promotion.source_root));
                let relative = source
                    .strip_prefix(&source_root)
                    .map_err(|_| invalid_error("promotion source file escaped its package root"))?;
                let destination = package.join(relative);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
                }
                fs::copy(source, &destination).map_err(|error| io_at(&destination, error))?;
            }
        }
        fs::write(stage.join("characters.json"), &plan.registry_after)
            .map_err(|source| io_at(stage.join("characters.json"), source))?;
        fs::write(stage.join("asset-manifest.json"), &plan.manifest_after)
            .map_err(|source| io_at(stage.join("asset-manifest.json"), source))?;
        Ok(())
    })();
    if let Err(error) = prepare {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    if read_regular(&plan.manifest_path)? != plan.manifest_before
        || read_regular(&plan.registry_path)? != plan.registry_before
    {
        let _ = fs::remove_dir_all(&stage);
        return invalid("manifest or character registry changed while promotion was staged");
    }

    let mut moved_sources = Vec::<(PathBuf, PathBuf)>::new();
    let mut moved_targets = Vec::<(PathBuf, PathBuf)>::new();
    let old_registry = stage_backup.join("characters.json");
    let old_manifest = stage_backup.join("asset-manifest.json");
    let mut registry_backed_up = false;
    let mut registry_installed = false;
    let mut manifest_backed_up = false;
    let mut manifest_installed = false;

    let commit = (|| -> Result<()> {
        for promotion in &plan.promotions {
            let source = plan.asset_root.join(native_path(&promotion.source_root));
            let backup = stage_backup.join(promotion.candidate.id);
            fs::rename(&source, &backup).map_err(|error| io_at(&source, error))?;
            moved_sources.push((source, backup));
        }
        for promotion in &plan.promotions {
            let staged = stage_packages.join(promotion.candidate.id);
            let target = plan.asset_root.join(native_path(&promotion.target_root));
            let parent = target
                .parent()
                .ok_or_else(|| invalid_error("promotion target has no parent"))?;
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
            fs::rename(&staged, &target).map_err(|error| io_at(&target, error))?;
            moved_targets.push((target, staged));
        }
        fs::rename(&plan.registry_path, &old_registry)
            .map_err(|error| io_at(&plan.registry_path, error))?;
        registry_backed_up = true;
        fs::rename(stage.join("characters.json"), &plan.registry_path)
            .map_err(|error| io_at(&plan.registry_path, error))?;
        registry_installed = true;
        fs::rename(&plan.manifest_path, &old_manifest)
            .map_err(|error| io_at(&plan.manifest_path, error))?;
        manifest_backed_up = true;
        fs::rename(stage.join("asset-manifest.json"), &plan.manifest_path)
            .map_err(|error| io_at(&plan.manifest_path, error))?;
        manifest_installed = true;
        Ok(())
    })();

    if let Err(error) = commit {
        if manifest_installed {
            let _ = fs::remove_file(&plan.manifest_path);
        }
        if manifest_backed_up {
            let _ = fs::rename(&old_manifest, &plan.manifest_path);
        }
        if registry_installed {
            let _ = fs::remove_file(&plan.registry_path);
        }
        if registry_backed_up {
            let _ = fs::rename(&old_registry, &plan.registry_path);
        }
        for (target, staged) in moved_targets.iter().rev() {
            let _ = fs::rename(target, staged);
        }
        for (source, backup) in moved_sources.iter().rev() {
            let _ = fs::rename(backup, source);
        }
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    fs::remove_dir_all(&stage).map_err(|source| {
        invalid_error(format!(
            "promotion committed, but recovery staging {} could not be removed: {source}",
            stage.display()
        ))
    })?;
    Ok(())
}
