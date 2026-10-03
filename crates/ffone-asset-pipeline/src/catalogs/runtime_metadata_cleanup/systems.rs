use super::*;

pub(super) fn apply_plan(plan: &CleanupPlan, source_build: &str) -> Result<()> {
    let mut no_fault = || Ok(());
    apply_plan_inner(plan, source_build, &mut no_fault)
}

pub(super) fn apply_plan_inner(
    plan: &CleanupPlan,
    source_build: &str,
    before_archive_publication: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let archive_parent = plan
        .archive_root
        .parent()
        .ok_or_else(|| invalid_error("archive root has no parent"))?;
    fs::create_dir_all(archive_parent).map_err(|error| io_at(archive_parent, error))?;
    reject_stale_transactions(&plan.asset_root, Some(archive_parent))?;
    let transaction_lock = acquire_cleanup_transaction_lock(&plan.asset_root, source_build)?;
    let manifest_before_blake3 = validate_plan_manifest_identity(plan, None)?;

    let mut manifest_after_bytes =
        serde_json::to_vec_pretty(&plan.next_manifest).map_err(|source| PipelineError::Json {
            path: "clean-runtime-metadata manifest-next".to_owned(),
            source,
        })?;
    manifest_after_bytes.push(b'\n');
    let manifest_after_blake3 = blake3::hash(&manifest_after_bytes).to_hex().to_string();

    let stamp = transaction_stamp();
    let archive_stage = if !plan.primary_archive_exists {
        Some(archive_parent.join(format!(".conversion-metadata-stage-{stamp}")))
    } else if !plan.revision_archived.is_empty() {
        Some(archive_parent.join(format!(".conversion-metadata-revision-stage-{stamp}")))
    } else {
        None
    };
    let revision_root = plan.revision_plan_blake3.as_ref().map(|plan_blake3| {
        plan.archive_root
            .join(ARCHIVE_REVISIONS_DIRECTORY)
            .join(plan_blake3)
    });
    let archive_target = if !plan.primary_archive_exists {
        Some(plan.archive_root.clone())
    } else {
        revision_root.clone()
    };
    let transaction_root = plan.asset_root.join(format!(
        "{CLEAN_RUNTIME_METADATA_TRANSACTION_DIRECTORY_PREFIX}{stamp}"
    ));
    let backup = transaction_root.join("backup");
    let manifest_next = transaction_root.join("manifest-next.json");
    let manifest_backup = transaction_root.join("manifest-backup.json");
    if transaction_root.exists() {
        return invalid(format!(
            "clean-runtime-metadata transaction target already exists: {}",
            transaction_root.display()
        ));
    }
    if let Some(path) = &archive_stage {
        if path.exists() {
            return invalid(format!(
                "clean-runtime-metadata archive stage already exists: {}",
                path.display()
            ));
        }
    }
    if let Some(path) = &archive_target {
        if path.exists() {
            return invalid(format!(
                "clean-runtime-metadata immutable archive target already exists: {}",
                path.display()
            ));
        }
    }

    let transaction_plan_identity = CleanupTransactionPlanIdentity {
        schema: CLEAN_RUNTIME_METADATA_TRANSACTION_SCHEMA,
        source_build,
        manifest_before_blake3: &manifest_before_blake3,
        manifest_after_blake3: &manifest_after_blake3,
        archived: &plan.archived,
        revision_archived: &plan.revision_archived,
        removal_only: &plan.removal_only,
        runtime_copies: &plan.runtime_copies,
    };
    let transaction_plan_bytes =
        serde_json::to_vec(&transaction_plan_identity).map_err(|source| PipelineError::Json {
            path: transaction_root.join("plan.json").display().to_string(),
            source,
        })?;
    let transaction_plan_blake3 = blake3::hash(&transaction_plan_bytes).to_hex().to_string();
    let journal = CleanupTransactionJournal {
        schema: CLEAN_RUNTIME_METADATA_TRANSACTION_SCHEMA,
        transaction_id: &stamp,
        transaction_plan_blake3: &transaction_plan_blake3,
        source_build,
        manifest_before_blake3: &manifest_before_blake3,
        manifest_after_blake3: &manifest_after_blake3,
        archive_stage: archive_stage
            .as_ref()
            .map(|path| path.display().to_string()),
        archive_target: archive_target
            .as_ref()
            .map(|path| path.display().to_string()),
        backup_path: backup.display().to_string(),
        manifest_next_path: manifest_next.display().to_string(),
        manifest_backup_path: manifest_backup.display().to_string(),
        archived: &plan.archived,
        revision_archived: &plan.revision_archived,
        removal_only: &plan.removal_only,
        runtime_copies: &plan.runtime_copies,
    };
    fs::create_dir(&transaction_root).map_err(|error| io_at(&transaction_root, error))?;

    let mut backed_up = Vec::<ArchivedRuntimeMetadata>::new();
    let mut installed_runtime = Vec::<RuntimeMetadataRegistryCopy>::new();
    let mut manifest_was_backed_up = false;
    let mut manifest_was_replaced = false;
    let commit = (|| -> Result<()> {
        write_json_new(&transaction_root.join("plan.json"), &journal)?;
        record_cleanup_transaction_phase(&transaction_root, "prepared")?;

        if let Some(stage) = &archive_stage {
            fs::create_dir(stage).map_err(|error| io_at(stage, error))?;
            let staged_files = if plan.primary_archive_exists {
                &plan.revision_archived
            } else {
                &plan.archived
            };
            stage_archive_payload(&plan.asset_root, stage, staged_files)?;
            if plan.primary_archive_exists {
                let plan_blake3 = plan.revision_plan_blake3.as_ref().ok_or_else(|| {
                    invalid_error("revision payload has no deterministic plan hash")
                })?;
                let archived_bytes = staged_files.iter().map(|file| file.bytes).sum();
                let index = ConversionMetadataRevisionIndex {
                    schema: CLEAN_RUNTIME_METADATA_REVISION_INDEX_SCHEMA.to_owned(),
                    source_build: source_build.to_owned(),
                    plan_blake3: plan_blake3.clone(),
                    files: staged_files.clone(),
                };
                let report = ConversionMetadataRevisionReport {
                    schema: CLEAN_RUNTIME_METADATA_REVISION_REPORT_SCHEMA.to_owned(),
                    source_build: source_build.to_owned(),
                    plan_blake3: plan_blake3.clone(),
                    archived_files: staged_files.len() as u64,
                    archived_bytes,
                    files: staged_files.clone(),
                };
                write_json_new(&stage.join(CLEAN_RUNTIME_METADATA_INDEX_FILE), &index)?;
                write_json_new(&stage.join(CLEAN_RUNTIME_METADATA_REPORT_FILE), &report)?;
            } else {
                let applied_report = plan.report(source_build, CleanRuntimeMetadataMode::Apply);
                let index = ConversionMetadataArchiveIndex {
                    schema: CLEAN_RUNTIME_METADATA_INDEX_SCHEMA,
                    source_build,
                    files: &plan.archived,
                };
                write_json_new(&stage.join(CLEAN_RUNTIME_METADATA_INDEX_FILE), &index)?;
                write_json_new(
                    &stage.join(CLEAN_RUNTIME_METADATA_REPORT_FILE),
                    &applied_report,
                )?;
            }
            record_cleanup_transaction_phase(&transaction_root, "archive-staged")?;
        }

        write_json_new(&manifest_next, &plan.next_manifest)?;
        record_cleanup_transaction_phase(&transaction_root, "manifest-prepared")?;
        if let Some(target) = &archive_target {
            let parent = target
                .parent()
                .ok_or_else(|| invalid_error("archive target has no parent"))?;
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
        }
        validate_plan_manifest_identity(plan, Some(&manifest_before_blake3))?;
        fs::create_dir(&backup).map_err(|error| io_at(&backup, error))?;

        for entry in &plan.archived {
            let relative = native_path(&entry.source_path)?;
            let source = plan.asset_root.join(&relative);
            let target = backup.join(&relative);
            let parent = target
                .parent()
                .ok_or_else(|| invalid_error("backup payload has no parent"))?;
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
            validate_file_identity(&source, entry.bytes, &entry.blake3, "live archive source")?;
            fs::rename(&source, &target).map_err(|error| io_at(&source, error))?;
            backed_up.push(entry.clone());
            validate_file_identity(&target, entry.bytes, &entry.blake3, "transaction backup")?;
        }
        record_cleanup_transaction_phase(&transaction_root, "sources-backed-up")?;
        for copy in &plan.runtime_copies {
            let source = backup.join(native_path(&copy.source_path)?);
            let relative = native_path(&copy.runtime_path)?;
            let target = plan.asset_root.join(&relative);
            let parent = target
                .parent()
                .ok_or_else(|| invalid_error("runtime registry has no parent"))?;
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
            copy_new_verified(&source, &target, copy.bytes, &copy.blake3)?;
            installed_runtime.push(copy.clone());
        }
        record_cleanup_transaction_phase(&transaction_root, "runtime-copies-installed")?;
        validate_plan_manifest_identity(plan, Some(&manifest_before_blake3))?;
        for entry in &plan.archived {
            let source = plan.asset_root.join(native_path(&entry.source_path)?);
            if source.exists() {
                return invalid(format!(
                    "archived source reappeared during cleanup transaction: {}",
                    source.display()
                ));
            }
        }
        for copy in &plan.runtime_copies {
            validate_file_identity(
                &plan.asset_root.join(native_path(&copy.runtime_path)?),
                copy.bytes,
                &copy.blake3,
                "installed runtime registry",
            )?;
        }
        record_cleanup_transaction_phase(&transaction_root, "commit-started")?;
        fs::rename(&plan.manifest_path, &manifest_backup)
            .map_err(|error| io_at(&plan.manifest_path, error))?;
        manifest_was_backed_up = true;
        record_cleanup_transaction_phase(&transaction_root, "manifest-backed-up")?;
        fs::rename(&manifest_next, &plan.manifest_path)
            .map_err(|error| io_at(&plan.manifest_path, error))?;
        manifest_was_replaced = true;
        if let (Some(stage), Some(target)) = (&archive_stage, &archive_target) {
            record_cleanup_transaction_phase(&transaction_root, "manifest-swapped")?;
            before_archive_publication()?;
            fs::rename(stage, target).map_err(|error| io_at(target, error))?;
        }
        Ok(())
    })();
    if let Err(error) = commit {
        let rollback_result = rollback(
            plan,
            &transaction_root,
            &backup,
            &manifest_next,
            &manifest_backup,
            archive_stage.as_deref(),
            &backed_up,
            &installed_runtime,
            manifest_was_backed_up,
            manifest_was_replaced,
        );
        return match rollback_result {
            Ok(()) => Err(error),
            Err(rollback_error) => invalid(format!(
                "commit failed ({error}); {rollback_error}; recovery artifacts were preserved"
            )),
        };
    }

    if let Err(error) =
        verify_committed_cleanup_state(plan, source_build, archive_target.as_deref())
    {
        return invalid(format!(
            "cleanup commit is published but final verification failed ({error}); transaction \
             evidence was preserved at {}",
            transaction_root.display()
        ));
    }
    if let Err(error) = fs::remove_dir_all(&transaction_root) {
        return invalid(format!(
            "cleanup commit is complete but transaction garbage collection failed at {}: {error}",
            transaction_root.display()
        ));
    }
    transaction_lock.release()
}
