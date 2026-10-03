use super::*;

pub(super) fn apply_plan(plan: &PromotionPlan) -> Result<()> {
    validate_plan_unchanged(plan)?;
    reject_stale_staging(&plan.project_root)?;
    let stage = plan.project_root.join(format!(
        ".tutorial-prop-promotion-{}-{}",
        std::process::id(),
        unique_stamp()
    ));
    fs::create_dir(&stage).map_err(|source| io_at(&stage, source))?;
    let stage_packages = stage.join("packages");
    let stage_runtime = stage.join("runtime");
    let stage_backup = stage.join("backup");
    fs::create_dir(&stage_packages).map_err(|source| io_at(&stage_packages, source))?;
    fs::create_dir(&stage_runtime).map_err(|source| io_at(&stage_runtime, source))?;
    fs::create_dir(&stage_backup).map_err(|source| io_at(&stage_backup, source))?;

    let prepare = (|| -> Result<()> {
        for promotion in &plan.promotions {
            let source = plan.asset_root.join(native_path(&promotion.source_root));
            let staged = stage_packages.join(promotion.candidate.id);
            for relative in promotion.source_tree.keys() {
                let destination = staged.join(native_path(relative));
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
                }
                fs::copy(source.join(native_path(relative)), &destination)
                    .map_err(|error| io_at(&destination, error))?;
            }
            if collect_tree(&staged, false)? != promotion.source_tree {
                return invalid(format!(
                    "staged package identity changed for {:?}",
                    promotion.candidate.id
                ));
            }
        }
        for (index, promotion) in plan.promotions.iter().enumerate() {
            let staged = stage_runtime.join(format!("{index}.rs"));
            fs::write(&staged, &promotion.runtime_after)
                .map_err(|source| io_at(&staged, source))?;
        }
        let staged_manifest = stage.join("asset-manifest.json");
        fs::write(&staged_manifest, &plan.manifest_after)
            .map_err(|source| io_at(&staged_manifest, source))?;
        Ok(())
    })();
    if let Err(error) = prepare {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    if let Err(error) = validate_plan_unchanged(plan) {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    let mut moved_sources = Vec::<(PathBuf, PathBuf)>::new();
    let mut moved_targets = Vec::<(PathBuf, PathBuf)>::new();
    let mut runtime_states = plan
        .promotions
        .iter()
        .enumerate()
        .map(|(index, promotion)| RuntimeCommitState {
            original: promotion.runtime_source_path.clone(),
            backup: stage_backup.join(format!("runtime-{index}.rs")),
            staged: stage_runtime.join(format!("{index}.rs")),
            backed_up: false,
            installed: false,
        })
        .collect::<Vec<_>>();
    let old_manifest = stage_backup.join("asset-manifest.json");
    let mut manifest_backed_up = false;
    let mut manifest_installed = false;

    let commit = (|| -> Result<()> {
        for promotion in &plan.promotions {
            let source = plan.asset_root.join(native_path(&promotion.source_root));
            let backup = stage_backup.join(format!("package-{}", promotion.candidate.id));
            fs::rename(&source, &backup).map_err(|error| io_at(&source, error))?;
            moved_sources.push((source, backup));
        }
        for promotion in &plan.promotions {
            let staged = stage_packages.join(promotion.candidate.id);
            let target = plan.asset_root.join(native_path(&promotion.target_root));
            let parent = target
                .parent()
                .ok_or_else(|| invalid_error("prop target has no parent"))?;
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
            fs::rename(&staged, &target).map_err(|error| io_at(&target, error))?;
            moved_targets.push((target, staged));
        }
        for state in &mut runtime_states {
            fs::rename(&state.original, &state.backup)
                .map_err(|error| io_at(&state.original, error))?;
            state.backed_up = true;
            fs::rename(&state.staged, &state.original)
                .map_err(|error| io_at(&state.original, error))?;
            state.installed = true;
        }
        fs::rename(&plan.manifest_path, &old_manifest)
            .map_err(|error| io_at(&plan.manifest_path, error))?;
        manifest_backed_up = true;
        fs::rename(stage.join("asset-manifest.json"), &plan.manifest_path)
            .map_err(|error| io_at(&plan.manifest_path, error))?;
        manifest_installed = true;
        validate_committed_plan(plan)
    })();

    if let Err(error) = commit {
        if manifest_installed {
            let _ = fs::remove_file(&plan.manifest_path);
        }
        if manifest_backed_up {
            let _ = fs::rename(&old_manifest, &plan.manifest_path);
        }
        for state in runtime_states.iter().rev() {
            if state.installed {
                let _ = fs::remove_file(&state.original);
            }
            if state.backed_up {
                let _ = fs::rename(&state.backup, &state.original);
            }
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
