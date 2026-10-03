use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalCharacterInstallOptions {
    pub candidate_root: PathBuf,
    pub gpu_audit_path: PathBuf,
    pub asset_root: PathBuf,
    pub source_build: String,
}

impl LogicalCharacterInstallOptions {
    pub fn new(
        candidate_root: impl Into<PathBuf>,
        gpu_audit_path: impl Into<PathBuf>,
        asset_root: impl Into<PathBuf>,
        source_build: impl Into<String>,
    ) -> Self {
        Self {
            candidate_root: candidate_root.into(),
            gpu_audit_path: gpu_audit_path.into(),
            asset_root: asset_root.into(),
            source_build: source_build.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalCharacterInstallReport {
    pub catalog: SemanticCharacterCatalog,
    pub installed_files: u64,
    pub installed_bytes: u64,
    pub manifest_files: u64,
}

pub fn install_logical_characters(
    options: &LogicalCharacterInstallOptions,
) -> Result<LogicalCharacterInstallReport> {
    if options.source_build.trim().is_empty() {
        return invalid("source build identity may not be empty");
    }
    let candidate_root = canonical_directory(&options.candidate_root, "candidate root")?;
    let asset_root = canonical_directory(&options.asset_root, "asset root")?;

    let mut structural = audit_logical_model_tree(&candidate_root)?;
    if structural.models.is_empty() {
        return invalid(format!(
            "logical character candidate contains no auditable models ({} violations)",
            structural.violations.len()
        ));
    }
    structural.output_root = "offline-native-logical-character-candidate".to_owned();

    let gpu_audit_bytes =
        fs::read(&options.gpu_audit_path).map_err(|error| io_at(&options.gpu_audit_path, error))?;
    let stored_gpu: LogicalModelGpuEvidenceAuditReport = serde_json::from_slice(&gpu_audit_bytes)
        .map_err(|source| PipelineError::Json {
        path: options.gpu_audit_path.display().to_string(),
        source,
    })?;
    if stored_gpu.schema != GPU_EVIDENCE_AUDIT_SCHEMA
        || !stored_gpu.structural_passed
        || !stored_gpu.automated_gpu_passed
        || stored_gpu.models.iter().any(|model| !model.passed)
        || !stored_gpu.violations.is_empty()
    {
        return invalid("stored GPU evidence audit is not a clean automated acceptance report");
    }
    verify_archived_gpu_evidence(&candidate_root, &stored_gpu)?;

    let structural_paths = structural
        .models
        .iter()
        .map(|model| model.path.as_str())
        .collect::<BTreeSet<_>>();
    let gpu_paths = stored_gpu
        .models
        .iter()
        .map(|model| model.relative_glb.as_str())
        .collect::<BTreeSet<_>>();
    if structural_paths != gpu_paths {
        return invalid("GPU evidence model set does not exactly match the structural model set");
    }

    let batch_path = candidate_root.join("logical-model-batch-report.json");
    let batch_bytes = fs::read(&batch_path).map_err(|error| io_at(&batch_path, error))?;
    let batch: Value =
        serde_json::from_slice(&batch_bytes).map_err(|source| PipelineError::Json {
            path: batch_path.display().to_string(),
            source,
        })?;
    if batch.get("structuralAuditPassed").and_then(Value::as_bool) != Some(true) {
        return invalid("batch report does not carry a passing structural audit");
    }
    let batch_models = batch
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_error("batch report has no model mappings"))?;
    if batch_models.len() != structural.models.len() {
        return invalid("batch, structural and GPU model counts differ");
    }

    let mut catalog_models = Vec::with_capacity(structural.models.len());
    let mut routes = Vec::with_capacity(structural.models.len());
    for model in &structural.models {
        let mapping = batch_models
            .iter()
            .find(|mapping| string(mapping, "outputGlb") == Some(model.path.as_str()))
            .ok_or_else(|| {
                invalid_error(format!("batch report has no mapping for {:?}", model.path))
            })?;
        let catalog_model = build_catalog_model(&candidate_root, model.path.as_str(), mapping)?;
        routes.push(CharacterRoute {
            source_directory: parent_slash(model.path.as_str())?,
            destination_directory: parent_slash(&catalog_model.glb)?,
        });
        catalog_models.push(catalog_model);
    }
    catalog_models.sort_by(|left, right| left.glb.cmp(&right.glb));

    let catalog = SemanticCharacterCatalog {
        schema: SEMANTIC_CHARACTER_CATALOG_SCHEMA.to_owned(),
        status:
            "runtime-accepted-archived-attestation-schema-upgrade-required-visual-parity-pending"
                .to_owned(),
        runtime_accepted: true,
        production_approved: false,
        visual_parity_pending: stored_gpu.visual_parity_pending,
        schema_upgrade_required: !structural.passed,
        readable_sidecar_layout_pending: true,
        source_build: options.source_build.clone(),
        coordinate_contract: "ffone.native-coordinate-contract.v1 / H=diag(-1,1,1)".to_owned(),
        runtime_spawn_policy:
            "typed character kind; identity loader wrappers; exact root m_Name; one visual Ry180"
                .to_owned(),
        proofs: SemanticCharacterProofs {
            batch_report_source: "offline-native-logical-model-batch-report.json".to_owned(),
            batch_report_sha256: sha256(&batch_bytes),
            structural_audit_schema: structural.schema.clone(),
            structural_audit_passed: structural.passed,
            structural_violation_count: structural.violations.len() as u64,
            structural_upgrade_report: CHARACTER_SCHEMA_UPGRADE_REPORT_PATH.to_owned(),
            gpu_audit_source: "offline-native-logical-model-gpu-evidence.audit.json".to_owned(),
            gpu_audit_sha256: sha256(&gpu_audit_bytes),
            gpu_audit_schema: stored_gpu.schema.clone(),
            automated_gpu_passed: true,
            archived_gpu_evidence_verified_at_rest: true,
        },
        models: catalog_models,
    };
    if !catalog.visual_parity_pending {
        return invalid(
            "installer refuses to infer production approval from an automated GPU report",
        );
    }

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    let destination = asset_root.join("characters");
    let previous = prepare_previous_install(&asset_root, &destination, &mut manifest)?;

    let source_files = candidate_files(&candidate_root)?;
    let stage = create_stage(&asset_root)?;
    let mut structural_bytes =
        serde_json::to_vec_pretty(&structural).map_err(|source| PipelineError::Json {
            path: CHARACTER_SCHEMA_UPGRADE_REPORT_PATH.to_owned(),
            source,
        })?;
    structural_bytes.push(b'\n');
    let install_result = write_stage(
        &candidate_root,
        &stage,
        &source_files,
        &routes,
        &catalog,
        &structural_bytes,
        &manifest,
    );
    let new_entries = match install_result {
        Ok(entries) => entries,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            restore_previous_install(&destination, previous.as_deref());
            return Err(error);
        }
    };
    if let Err(error) = fs::rename(&stage, &destination) {
        restore_previous_install(&destination, previous.as_deref());
        return Err(io_at(&destination, error));
    }

    manifest.files.extend(new_entries.iter().cloned());
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    if let Err(error) = replace_manifest(&asset_root, &manifest) {
        let failed = asset_root.join(".characters-install-failed");
        let _ = fs::rename(&destination, &failed);
        restore_previous_install(&destination, previous.as_deref());
        return Err(error);
    }
    if let Some(previous) = previous {
        fs::remove_dir_all(&previous).map_err(|error| io_at(&previous, error))?;
    }

    let installed_bytes = new_entries.iter().map(|entry| entry.bytes).sum();
    Ok(LogicalCharacterInstallReport {
        catalog,
        installed_files: new_entries.len() as u64,
        installed_bytes,
        manifest_files: manifest.files.len() as u64,
    })
}

pub(super) fn write_stage(
    candidate_root: &Path,
    stage: &Path,
    source_files: &[(String, PathBuf)],
    routes: &[CharacterRoute],
    catalog: &SemanticCharacterCatalog,
    structural_upgrade_report: &[u8],
    manifest: &ProjectAssetManifest,
) -> Result<Vec<ProjectAssetFile>> {
    let mut entries = Vec::with_capacity(source_files.len() + 1);
    let existing = manifest
        .files
        .iter()
        .map(|entry| entry.path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut planned = BTreeSet::new();
    for (source_relative, source_path) in source_files {
        let destination = destination_file_path(source_relative, routes)?;
        reserve_path(&existing, &mut planned, &destination)?;
        let bytes = fs::read(source_path).map_err(|error| io_at(source_path, error))?;
        let stage_relative = destination
            .strip_prefix("characters/")
            .ok_or_else(|| invalid_error("character destination lost its prefix"))?;
        write_new_file(stage, stage_relative, &bytes)?;
        entries.push(ProjectAssetFile {
            source_path: format!("native-logical-characters/{source_relative}"),
            path: destination,
            kind: kind_for(source_relative)?,
            bytes: bytes.len() as u64,
            blake3: blake3::hash(&bytes).to_hex().to_string(),
        });
    }
    reserve_path(&existing, &mut planned, SEMANTIC_CHARACTER_CATALOG_PATH)?;
    let mut catalog_bytes =
        serde_json::to_vec_pretty(catalog).map_err(|source| PipelineError::Json {
            path: SEMANTIC_CHARACTER_CATALOG_PATH.to_owned(),
            source,
        })?;
    catalog_bytes.push(b'\n');
    write_new_file(stage, "catalog.json", &catalog_bytes)?;
    entries.push(ProjectAssetFile {
        source_path: format!(
            "native-logical-characters/{}/catalog.json",
            candidate_root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("candidate")
        ),
        path: SEMANTIC_CHARACTER_CATALOG_PATH.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: catalog_bytes.len() as u64,
        blake3: blake3::hash(&catalog_bytes).to_hex().to_string(),
    });
    reserve_path(
        &existing,
        &mut planned,
        CHARACTER_SCHEMA_UPGRADE_REPORT_PATH,
    )?;
    write_new_file(
        stage,
        "schema-upgrade-violations.json",
        structural_upgrade_report,
    )?;
    entries.push(ProjectAssetFile {
        source_path: "native-logical-characters/current-structural-audit.json".to_owned(),
        path: CHARACTER_SCHEMA_UPGRADE_REPORT_PATH.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: structural_upgrade_report.len() as u64,
        blake3: blake3::hash(structural_upgrade_report).to_hex().to_string(),
    });
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

pub(super) fn prepare_previous_install(
    asset_root: &Path,
    destination: &Path,
    manifest: &mut ProjectAssetManifest,
) -> Result<Option<PathBuf>> {
    let metadata = match fs::symlink_metadata(destination) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_at(destination, error)),
    };
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return invalid("existing character install is not a regular directory");
    }
    let catalog_path = destination.join("catalog.json");
    let catalog_bytes = fs::read(&catalog_path).map_err(|error| io_at(&catalog_path, error))?;
    let catalog: Value =
        serde_json::from_slice(&catalog_bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    if string(&catalog, "schema") != Some(SEMANTIC_CHARACTER_CATALOG_SCHEMA) {
        return invalid("existing character tree was not created by this native installer");
    }

    let character_entries = manifest
        .files
        .iter()
        .filter(|entry| entry.path.starts_with("characters/"))
        .collect::<Vec<_>>();
    if character_entries.is_empty() {
        return invalid("existing character tree has no project-manifest ownership");
    }
    let mut manifest_paths = BTreeSet::new();
    for entry in &character_entries {
        let path = join_relative(asset_root, &entry.path)?;
        let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
        if bytes.len() as u64 != entry.bytes
            || blake3::hash(&bytes).to_hex().as_str() != entry.blake3
        {
            return invalid(format!(
                "existing native character asset no longer matches its manifest: {:?}",
                entry.path
            ));
        }
        manifest_paths.insert(entry.path.clone());
    }
    let disk_paths = installed_character_files(destination)?
        .into_iter()
        .map(|relative| format!("characters/{relative}"))
        .collect::<BTreeSet<_>>();
    if disk_paths != manifest_paths {
        return invalid("existing character directory and manifest file sets differ");
    }

    let backup = asset_root.join(format!(
        ".characters-install-previous-{}-{}",
        std::process::id(),
        STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    if fs::symlink_metadata(&backup).is_ok() {
        return invalid("previous-install backup path already exists");
    }
    fs::rename(destination, &backup).map_err(|error| io_at(destination, error))?;
    manifest
        .files
        .retain(|entry| !entry.path.starts_with("characters/"));
    Ok(Some(backup))
}

pub(super) fn restore_previous_install(destination: &Path, previous: Option<&Path>) {
    if let Some(previous) = previous {
        let _ = fs::rename(previous, destination);
    }
}

pub(super) fn write_new_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<()> {
    let path = join_relative(root, relative)?;
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("staged file has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| io_at(&path, error))?;
    output
        .write_all(bytes)
        .map_err(|error| io_at(&path, error))?;
    output.sync_all().map_err(|error| io_at(&path, error))
}
