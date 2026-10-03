use super::*;

pub(super) const INSTALL_BACKUP: &str = ".tutorial-models-install.previous";

pub fn install_tutorial_models(
    options: &TutorialModelInstallOptions,
) -> Result<TutorialModelInstallReport> {
    validate_source_build(&options.source_build)?;
    if options.models.is_empty() {
        return invalid("at least one --model selection is required");
    }
    if options.evidence_roots.is_empty() {
        return invalid("at least one --evidence-root is required");
    }

    let candidate_root = canonical_plain_directory(&options.candidate_root, "candidate root")?;
    let asset_root = canonical_plain_directory(&options.asset_root, "asset root")?;
    reject_overlapping_roots(&candidate_root, &asset_root, "candidate root", "asset root")?;

    let selections = validate_selections(&options.models)?;
    let evidence_roots =
        prepare_evidence_roots(&options.evidence_roots, &candidate_root, &asset_root)?;

    let batch_bytes = read_regular_relative(&candidate_root, LOGICAL_MODEL_BATCH_REPORT_FILE)?;
    let batch: LogicalModelBatchPublishReport =
        serde_json::from_slice(&batch_bytes).map_err(|source| PipelineError::Json {
            path: candidate_root
                .join(LOGICAL_MODEL_BATCH_REPORT_FILE)
                .display()
                .to_string(),
            source,
        })?;
    validate_batch(&batch)?;
    let mappings = index_batch_mappings(&batch.models)?;

    let mut prepared_files = Vec::new();
    let mut catalog_models = Vec::with_capacity(selections.len());
    let mut destination_folds = BTreeSet::new();
    for selection in &selections {
        let mapping = mappings.get(&casefold(selection)).ok_or_else(|| {
            invalid_error(format!(
                "batch report has no exact mapping for {selection:?}"
            ))
        })?;
        if mapping.output_glb != *selection {
            return invalid(format!(
                "case-folded batch mapping differs from exact selection {selection:?}"
            ));
        }
        let (entry, mut files) =
            prepare_model(&candidate_root, &evidence_roots, selection, mapping)?;
        for file in &files {
            let folded = casefold(&file.installed_path);
            if !destination_folds.insert(folded) {
                return invalid(format!(
                    "selected tutorial closures collide at {:?}",
                    file.installed_path
                ));
            }
        }
        catalog_models.push(entry);
        prepared_files.append(&mut files);
    }
    catalog_models.sort_by(|left, right| left.installed_glb.cmp(&right.installed_glb));
    prepared_files.sort_by(|left, right| left.installed_path.cmp(&right.installed_path));

    let catalog = TutorialModelCatalog {
        schema: TUTORIAL_MODEL_CATALOG_SCHEMA.to_owned(),
        status: "runtime-accepted-automated-gpu-passed-visual-parity-not-asserted".to_owned(),
        installer: INSTALLER_ID.to_owned(),
        install_prefix: TUTORIAL_MODEL_ROOT.to_owned(),
        source_build: options.source_build.clone(),
        candidate_batch: TutorialModelBatchProof {
            candidate_relative_path: LOGICAL_MODEL_BATCH_REPORT_FILE.to_owned(),
            schema: batch.schema.clone(),
            sha256: sha256(&batch_bytes),
            selected_models: catalog_models.len() as u64,
        },
        models: catalog_models,
    };
    let mut catalog_bytes =
        serde_json::to_vec_pretty(&catalog).map_err(|source| PipelineError::Json {
            path: TUTORIAL_MODEL_CATALOG_PATH.to_owned(),
            source,
        })?;
    catalog_bytes.push(b'\n');
    if !destination_folds.insert(casefold(TUTORIAL_MODEL_CATALOG_PATH)) {
        return invalid("tutorial catalog collides with a selected model closure");
    }
    prepared_files.push(PreparedFile {
        candidate_path: format!("generated/{TUTORIAL_MODEL_CATALOG_PATH}"),
        installed_path: TUTORIAL_MODEL_CATALOG_PATH.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: catalog_bytes,
    });
    prepared_files.sort_by(|left, right| left.installed_path.cmp(&right.installed_path));

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    validate_manifest(&manifest)?;
    let replaced_previous_install = validate_previous_install(&asset_root, &manifest)?;

    let mut outside_entries = manifest
        .files
        .iter()
        .filter(|entry| !owned_path(&entry.path))
        .cloned()
        .collect::<Vec<_>>();
    let mut reserved = outside_entries
        .iter()
        .map(|entry| casefold(&entry.path))
        .collect::<BTreeSet<_>>();
    let mut new_entries = Vec::with_capacity(prepared_files.len());
    for file in &prepared_files {
        validate_relative(&file.installed_path)?;
        if !owned_path(&file.installed_path) {
            return invalid(format!(
                "prepared tutorial file escaped installer prefix: {:?}",
                file.installed_path
            ));
        }
        if !reserved.insert(casefold(&file.installed_path)) {
            return invalid(format!(
                "project asset path collision at {:?}",
                file.installed_path
            ));
        }
        new_entries.push(ProjectAssetFile {
            source_path: format!("native-tutorial-model-installer/{}", file.candidate_path),
            path: file.installed_path.clone(),
            kind: file.kind,
            bytes: file.bytes.len() as u64,
            blake3: blake3::hash(&file.bytes).to_hex().to_string(),
        });
    }
    outside_entries.extend(new_entries.iter().cloned());
    outside_entries.sort_by(|left, right| left.path.cmp(&right.path));
    let next_manifest = ProjectAssetManifest {
        schema: manifest.schema,
        protocol: manifest.protocol,
        locale: manifest.locale,
        source_pack: manifest.source_pack,
        files: outside_entries,
    };

    ensure_no_stale_transaction(&asset_root)?;
    let stage = create_stage(&asset_root)?;
    let stage_result = write_stage(&stage, &prepared_files);
    if let Err(error) = stage_result {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    if let Err(error) = commit_install(
        &asset_root,
        &stage,
        &next_manifest,
        replaced_previous_install,
    ) {
        if stage.exists() {
            let _ = fs::remove_dir_all(&stage);
        }
        return Err(error);
    }

    let installed_bytes = new_entries.iter().map(|entry| entry.bytes).sum();
    Ok(TutorialModelInstallReport {
        schema: TUTORIAL_MODEL_INSTALL_REPORT_SCHEMA.to_owned(),
        status: "installed".to_owned(),
        replaced_previous_install,
        installed_models: catalog.models.len() as u64,
        installed_files: new_entries.len() as u64,
        installed_bytes,
        manifest_files: next_manifest.files.len() as u64,
        catalog,
    })
}

pub(super) fn write_stage(stage: &Path, files: &[PreparedFile]) -> Result<()> {
    for file in files {
        let relative = file
            .installed_path
            .strip_prefix(&format!("{TUTORIAL_MODEL_ROOT}/"))
            .ok_or_else(|| invalid_error("staged tutorial model lost its owned prefix"))?;
        write_new_file(stage, relative, &file.bytes)?;
    }
    Ok(())
}

pub(super) fn commit_install(
    asset_root: &Path,
    stage: &Path,
    manifest: &ProjectAssetManifest,
    had_previous: bool,
) -> Result<()> {
    let destination = asset_root.join(TUTORIAL_MODEL_ROOT);
    let tutorial_parent = asset_root.join("tutorial");
    let install_backup = asset_root.join(INSTALL_BACKUP);
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_next = asset_root.join(MANIFEST_NEXT);
    let manifest_backup = asset_root.join(MANIFEST_BACKUP);

    let mut manifest_bytes =
        serde_json::to_vec_pretty(manifest).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    manifest_bytes.push(b'\n');
    write_new_absolute(&manifest_next, &manifest_bytes)?;

    let parent_existed = match fs::symlink_metadata(&tutorial_parent) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                let _ = fs::remove_file(&manifest_next);
                return invalid("tutorial parent is not a regular directory");
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Err(error) = fs::create_dir(&tutorial_parent) {
                let _ = fs::remove_file(&manifest_next);
                return Err(io_at(&tutorial_parent, error));
            }
            false
        }
        Err(error) => {
            let _ = fs::remove_file(&manifest_next);
            return Err(io_at(&tutorial_parent, error));
        }
    };

    if had_previous {
        if let Err(error) = fs::rename(&destination, &install_backup) {
            rollback_empty_parent(&tutorial_parent, parent_existed);
            let _ = fs::remove_file(&manifest_next);
            return Err(io_at(&destination, error));
        }
    }
    if let Err(error) = fs::rename(stage, &destination) {
        if had_previous {
            let _ = fs::rename(&install_backup, &destination);
        }
        rollback_empty_parent(&tutorial_parent, parent_existed);
        let _ = fs::remove_file(&manifest_next);
        return Err(io_at(&destination, error));
    }
    if let Err(error) = fs::rename(&manifest_path, &manifest_backup) {
        let _ = fs::remove_dir_all(&destination);
        if had_previous {
            let _ = fs::rename(&install_backup, &destination);
        }
        rollback_empty_parent(&tutorial_parent, parent_existed);
        let _ = fs::remove_file(&manifest_next);
        return Err(io_at(&manifest_path, error));
    }
    if let Err(error) = fs::rename(&manifest_next, &manifest_path) {
        let _ = fs::rename(&manifest_backup, &manifest_path);
        let _ = fs::remove_dir_all(&destination);
        if had_previous {
            let _ = fs::rename(&install_backup, &destination);
        }
        rollback_empty_parent(&tutorial_parent, parent_existed);
        return Err(io_at(&manifest_path, error));
    }
    fs::remove_file(&manifest_backup).map_err(|error| io_at(&manifest_backup, error))?;
    if had_previous {
        fs::remove_dir_all(&install_backup).map_err(|error| io_at(&install_backup, error))?;
    }
    Ok(())
}

pub(super) fn write_new_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<()> {
    validate_relative(relative)?;
    let path = relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component));
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("staged tutorial file has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    write_new_absolute(&path, bytes)
}

pub(super) fn write_new_absolute(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_at(path, error))?;
    output
        .write_all(bytes)
        .map_err(|error| io_at(path, error))?;
    output.sync_all().map_err(|error| io_at(path, error))
}
