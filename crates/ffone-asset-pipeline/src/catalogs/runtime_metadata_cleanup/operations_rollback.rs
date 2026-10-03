use super::*;

pub(super) fn revision_plan_blake3(source_build: &str, files: &[ArchivedRuntimeMetadata]) -> Result<String> {
    let identity = ConversionMetadataRevisionPlanIdentity {
        schema: CLEAN_RUNTIME_METADATA_REVISION_PLAN_SCHEMA,
        source_build,
        files,
    };
    let bytes = serde_json::to_vec(&identity).map_err(|source| PipelineError::Json {
        path: "conversion-metadata revision plan identity".to_owned(),
        source,
    })?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

pub(super) fn acquire_cleanup_transaction_lock(
    asset_root: &Path,
    source_build: &str,
) -> Result<CleanupTransactionLock> {
    let path = asset_root.join(CLEAN_RUNTIME_METADATA_LOCK_FILE);
    let mut created = false;
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|error| io_at(&path, error))?;
        created = true;
        writeln!(
            file,
            "schema={CLEAN_RUNTIME_METADATA_TRANSACTION_SCHEMA}\nsourceBuild={source_build}\npid={}",
            std::process::id()
        )
        .map_err(|error| io_at(&path, error))?;
        file.sync_all().map_err(|error| io_at(&path, error))
    })();
    if let Err(error) = result {
        if created {
            let _ = fs::remove_file(&path);
        }
        return Err(error);
    }
    Ok(CleanupTransactionLock {
        path,
        released: false,
    })
}

pub(super) fn record_cleanup_transaction_phase(transaction_root: &Path, phase: &str) -> Result<()> {
    let path = transaction_root.join("phase.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| io_at(&path, error))?;
    writeln!(file, "{phase}").map_err(|error| io_at(&path, error))?;
    file.sync_all().map_err(|error| io_at(&path, error))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rollback(
    plan: &CleanupPlan,
    transaction_root: &Path,
    backup: &Path,
    manifest_next: &Path,
    manifest_backup: &Path,
    archive_stage: Option<&Path>,
    backed_up: &[ArchivedRuntimeMetadata],
    installed_runtime: &[RuntimeMetadataRegistryCopy],
    manifest_was_backed_up: bool,
    manifest_was_replaced: bool,
) -> Result<()> {
    let mut failures = Vec::new();

    for entry in backed_up.iter().rev() {
        let relative = match native_path(&entry.source_path) {
            Ok(relative) => relative,
            Err(error) => {
                failures.push(error.to_string());
                continue;
            }
        };
        let source = backup.join(&relative);
        let target = plan.asset_root.join(&relative);
        if target.exists() {
            if let Err(error) =
                validate_file_identity(&target, entry.bytes, &entry.blake3, "restored live source")
            {
                failures.push(error.to_string());
            }
            continue;
        }
        if let Err(error) = validate_file_identity(
            &source,
            entry.bytes,
            &entry.blake3,
            "rollback backup source",
        ) {
            failures.push(error.to_string());
            continue;
        }
        if let Some(parent) = target.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            failures.push(io_at(parent, error).to_string());
            continue;
        }
        if let Err(error) = fs::rename(&source, &target) {
            failures.push(io_at(&target, error).to_string());
            continue;
        }
        if let Err(error) =
            validate_file_identity(&target, entry.bytes, &entry.blake3, "restored live source")
        {
            failures.push(error.to_string());
        }
    }

    let mut active_next_was_moved = false;
    if manifest_was_replaced {
        if manifest_next.exists() {
            failures.push(format!(
                "rollback manifest-next target already exists: {}",
                manifest_next.display()
            ));
        } else {
            match fs::rename(&plan.manifest_path, manifest_next) {
                Ok(()) => active_next_was_moved = true,
                Err(error) => failures.push(io_at(&plan.manifest_path, error).to_string()),
            }
        }
    }
    if manifest_was_backed_up && (!manifest_was_replaced || active_next_was_moved) {
        match fs::rename(manifest_backup, &plan.manifest_path) {
            Ok(()) => {}
            Err(error) => {
                failures.push(io_at(&plan.manifest_path, error).to_string());
                if active_next_was_moved && !plan.manifest_path.exists() {
                    let _ = fs::rename(manifest_next, &plan.manifest_path);
                }
            }
        }
    }

    for copy in installed_runtime.iter().rev() {
        let target = match native_path(&copy.runtime_path) {
            Ok(relative) => plan.asset_root.join(relative),
            Err(error) => {
                failures.push(error.to_string());
                continue;
            }
        };
        if !target.exists() {
            continue;
        }
        if let Err(error) =
            validate_file_identity(&target, copy.bytes, &copy.blake3, "rollback runtime copy")
        {
            failures.push(error.to_string());
            continue;
        }
        if let Err(error) = fs::remove_file(&target) {
            failures.push(io_at(&target, error).to_string());
        }
    }

    if plan.manifest_path.exists() {
        match fs::read(&plan.manifest_path) {
            Ok(bytes) => match serde_json::from_slice::<ProjectAssetManifest>(&bytes) {
                Ok(manifest) if manifest == plan.manifest => {}
                Ok(_) => failures.push("rollback restored the wrong manifest identity".to_owned()),
                Err(error) => failures.push(format!(
                    "rollback manifest is not valid project JSON: {error}"
                )),
            },
            Err(error) => failures.push(io_at(&plan.manifest_path, error).to_string()),
        }
    } else {
        failures.push("rollback left the active asset manifest absent".to_owned());
    }
    for entry in backed_up {
        match native_path(&entry.source_path) {
            Ok(relative) => {
                let target = plan.asset_root.join(relative);
                if let Err(error) = validate_file_identity(
                    &target,
                    entry.bytes,
                    &entry.blake3,
                    "rollback final live source",
                ) {
                    failures.push(error.to_string());
                }
            }
            Err(error) => failures.push(error.to_string()),
        }
    }

    if !failures.is_empty() {
        return invalid(format!(
            "rollback is incomplete: {}; backup/stage/journal remain at {}",
            failures.join("; "),
            transaction_root.display()
        ));
    }
    if let Some(stage) = archive_stage
        && stage.exists()
    {
        fs::remove_dir_all(stage).map_err(|error| io_at(stage, error))?;
    }
    if transaction_root.exists() {
        fs::remove_dir_all(transaction_root).map_err(|error| io_at(transaction_root, error))?;
    }
    Ok(())
}

pub(super) fn manifested_file_is_valid(
    asset_root: &Path,
    manifest_by_path: &BTreeMap<&str, Vec<&ProjectAssetFile>>,
    relative: &str,
) -> Result<bool> {
    let path = asset_root.join(native_path(relative)?);
    if !path.is_file() {
        return Ok(false);
    }
    let Some(entries) = manifest_by_path.get(relative) else {
        return Ok(false);
    };
    let [entry] = entries.as_slice() else {
        return Ok(false);
    };
    let metadata = fs::metadata(&path).map_err(|error| io_at(&path, error))?;
    let actual_hash = hash_file(&path)?;
    if entry.bytes != metadata.len() || entry.blake3 != actual_hash {
        return invalid(format!(
            "runtime registry identity mismatch for {relative:?}: manifest bytes={} blake3={}, \
             actual bytes={} blake3={actual_hash}",
            entry.bytes,
            entry.blake3,
            metadata.len()
        ));
    }
    Ok(true)
}

pub(super) fn hash_file(path: &Path) -> Result<String> {
    let file = fs::File::open(path).map_err(|error| io_at(path, error))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| io_at(path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub(super) fn is_zero(value: &u64) -> bool {
    *value == 0
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
