use super::*;

pub(super) fn apply_plan(plan: &RuntimeWorldMigrationPlan, source_build: &str) -> Result<()> {
    let archive_parent = plan
        .archive_root
        .parent()
        .ok_or_else(|| invalid_error("world archive root has no parent"))?;
    fs::create_dir_all(archive_parent).map_err(|error| io_at(archive_parent, error))?;
    reject_stale_transactions(&plan.asset_root, Some(archive_parent))?;
    let stamp = transaction_stamp();
    let archive_stage = archive_parent.join(format!(".runtime-world-stage-{stamp}"));
    let backup = plan
        .asset_root
        .join(format!(".runtime-world-backup-{stamp}"));
    let manifest_next = plan
        .asset_root
        .join(format!(".runtime-world-manifest-next-{stamp}"));
    let manifest_backup = plan
        .asset_root
        .join(format!(".runtime-world-manifest-backup-{stamp}"));
    for path in [
        &archive_stage,
        &backup,
        &manifest_next,
        &manifest_backup,
        &plan.archive_root,
    ] {
        if path.exists() {
            return invalid(format!(
                "runtime-world transaction target already exists: {}",
                path.display()
            ));
        }
    }

    fs::create_dir(&archive_stage).map_err(|error| io_at(&archive_stage, error))?;
    for entry in &plan.archived {
        let source = plan.asset_root.join(native_path(&entry.source_path)?);
        let target = archive_stage.join(native_path(&entry.archive_path)?);
        copy_file_new_verified(&source, &target, entry.bytes, &entry.blake3)?;
    }
    for rewrite in &plan.rewrites {
        let source = plan.asset_root.join(native_path(&rewrite.report.path)?);
        let original_target =
            archive_stage.join(native_path(&rewrite.report.original_archive_path)?);
        copy_file_new_verified(
            &source,
            &original_target,
            rewrite.report.original_bytes,
            &rewrite.report.original_blake3,
        )?;
        let next_path = format!("{ARCHIVE_NEXT_DIRECTORY}/{}", rewrite.report.path);
        let runtime_target = archive_stage.join(native_path(&next_path)?);
        write_bytes_new(&runtime_target, &rewrite.bytes)?;
        verify_file_identity(
            &runtime_target,
            rewrite.report.runtime_bytes,
            &rewrite.report.runtime_blake3,
        )?;
    }
    let registry_stage_path = archive_stage.join(native_path(&format!(
        "{ARCHIVE_NEXT_DIRECTORY}/{RUNTIME_WORLD_REGISTRY_PATH}"
    ))?);
    write_bytes_new(&registry_stage_path, &plan.registry_bytes)?;
    verify_file_identity(
        &registry_stage_path,
        plan.registry.bytes,
        &plan.registry.blake3,
    )?;

    let report = plan.report(source_build, RuntimeWorldMigrationMode::Apply);
    let index = RuntimeWorldArchiveIndex {
        schema: RUNTIME_WORLD_ARCHIVE_INDEX_SCHEMA,
        source_build,
        technical_files: &plan.archived,
        rewritten_originals: &report.rewritten,
        legacy_catalog_hash_drifts: &report.legacy_catalog_hash_drifts,
        runtime_registry: &plan.registry,
    };
    write_json_new(&archive_stage.join("index.json"), &index)?;
    write_json_new(&archive_stage.join("report.json"), &report)?;
    write_json_new(&manifest_next, &plan.next_manifest)?;

    fs::rename(&archive_stage, &plan.archive_root)
        .map_err(|error| io_at(&plan.archive_root, error))?;
    fs::create_dir(&backup).map_err(|error| {
        let _ = fs::remove_dir_all(&plan.archive_root);
        io_at(&backup, error)
    })?;

    let mut backed_up = Vec::new();
    let mut installed = Vec::new();
    let mut manifest_was_backed_up = false;
    let mut manifest_was_replaced = false;
    let commit = (|| -> Result<()> {
        for relative in plan
            .archived
            .iter()
            .map(|entry| entry.source_path.as_str())
            .chain(plan.rewrites.iter().map(|entry| entry.report.path.as_str()))
        {
            let relative = native_path(relative)?;
            let source = plan.asset_root.join(&relative);
            let target = backup.join(&relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
            }
            fs::rename(&source, &target).map_err(|error| io_at(&source, error))?;
            backed_up.push(relative);
        }
        for rewrite in &plan.rewrites {
            let relative = native_path(&rewrite.report.path)?;
            let source = plan.archive_root.join(native_path(&format!(
                "{ARCHIVE_NEXT_DIRECTORY}/{}",
                rewrite.report.path
            ))?);
            let target = plan.asset_root.join(&relative);
            copy_file_new_verified(
                &source,
                &target,
                rewrite.report.runtime_bytes,
                &rewrite.report.runtime_blake3,
            )?;
            installed.push(relative);
        }
        let registry_relative = native_path(RUNTIME_WORLD_REGISTRY_PATH)?;
        let registry_source = plan.archive_root.join(native_path(&format!(
            "{ARCHIVE_NEXT_DIRECTORY}/{RUNTIME_WORLD_REGISTRY_PATH}"
        ))?);
        let registry_target = plan.asset_root.join(&registry_relative);
        copy_file_new_verified(
            &registry_source,
            &registry_target,
            plan.registry.bytes,
            &plan.registry.blake3,
        )?;
        installed.push(registry_relative);

        fs::rename(&plan.manifest_path, &manifest_backup)
            .map_err(|error| io_at(&plan.manifest_path, error))?;
        manifest_was_backed_up = true;
        fs::rename(&manifest_next, &plan.manifest_path)
            .map_err(|error| io_at(&plan.manifest_path, error))?;
        manifest_was_replaced = true;
        Ok(())
    })();
    if let Err(error) = commit {
        rollback(
            plan,
            &backup,
            &manifest_next,
            &manifest_backup,
            &backed_up,
            &installed,
            manifest_was_backed_up,
            manifest_was_replaced,
        );
        return Err(error);
    }
    fs::remove_file(&manifest_backup).map_err(|error| io_at(&manifest_backup, error))?;
    fs::remove_dir_all(&backup).map_err(|error| io_at(&backup, error))?;
    Ok(())
}
