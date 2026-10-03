use super::*;

pub const TUTORIAL_EFFECT_INSTALL_REPORT_SCHEMA: &str = "ffone.tutorial-effect-install-report.v1";

#[derive(Clone, Debug)]
pub struct TutorialEffectInstallOptions {
    pub asset_root: PathBuf,
    pub source_bundle: PathBuf,
    pub object_dump: PathBuf,
    pub dependencies: Vec<TutorialEffectDependencyInput>,
    pub source_build: String,
}

impl TutorialEffectInstallOptions {
    pub fn new(
        asset_root: impl Into<PathBuf>,
        source_bundle: impl Into<PathBuf>,
        object_dump: impl Into<PathBuf>,
    ) -> Self {
        Self {
            asset_root: asset_root.into(),
            source_bundle: source_bundle.into(),
            object_dump: object_dump.into(),
            dependencies: Vec::new(),
            source_build: RETROBUTION_TUTORIAL_BUILD_ID.to_owned(),
        }
    }

    pub fn with_dependency(
        mut self,
        source_bundle: impl Into<PathBuf>,
        object_dump: impl Into<PathBuf>,
    ) -> Self {
        self.dependencies.push(TutorialEffectDependencyInput::new(
            source_bundle,
            object_dump,
        ));
        self
    }

    pub fn add_dependency(
        &mut self,
        source_bundle: impl Into<PathBuf>,
        object_dump: impl Into<PathBuf>,
    ) {
        self.dependencies.push(TutorialEffectDependencyInput::new(
            source_bundle,
            object_dump,
        ));
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialEffectInstallReport {
    pub schema: String,
    pub source_build: String,
    pub source_bundle_blake3: String,
    pub source_bundle_bytes: u64,
    pub source_dump_blake3: String,
    pub source_dump_bytes: u64,
    pub dependency_count: u64,
    pub effect_count: u64,
    pub projectile_effect_count: u64,
    pub bullet_row_count: u64,
    pub closure_object_count: u64,
    pub installed_files: u64,
    pub installed_bytes: u64,
    pub manifest_files: u64,
    pub effect_root: String,
    pub projectile_root: String,
    pub renderer_status: String,
}

pub fn install_tutorial_effects(
    options: &TutorialEffectInstallOptions,
) -> Result<TutorialEffectInstallReport> {
    if options.source_build != RETROBUTION_TUTORIAL_BUILD_ID {
        return invalid(format!(
            "tutorial effects are audited only for source build {:?}, got {:?}",
            RETROBUTION_TUTORIAL_BUILD_ID, options.source_build
        ));
    }

    let asset_root =
        fs::canonicalize(&options.asset_root).map_err(|error| io_at(&options.asset_root, error))?;
    reject_symlink(&asset_root, "asset root")?;
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let mut manifest = if manifest_path.is_file() {
        let manifest_bytes =
            fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
        let manifest: ProjectAssetManifest =
            serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
                path: manifest_path.display().to_string(),
                source,
            })?;
        if manifest.schema != PROJECT_ASSET_SCHEMA {
            return invalid(format!(
                "asset manifest schema is {:?}, expected {:?}",
                manifest.schema, PROJECT_ASSET_SCHEMA
            ));
        }
        Some(manifest)
    } else {
        None
    };

    let source_bundle = canonical_regular_file(&options.source_bundle, "source Effects asset")?;
    let object_dump = canonical_regular_file(&options.object_dump, "source Effects object dump")?;
    let dependencies = options
        .dependencies
        .iter()
        .map(|dependency| {
            Ok((
                canonical_regular_file(
                    &dependency.source_bundle,
                    "tutorial effect dependency asset",
                )?,
                canonical_regular_file(
                    &dependency.object_dump,
                    "tutorial effect dependency object dump",
                )?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let audit = audit_source(
        &source_bundle,
        &object_dump,
        &dependencies,
        &options.source_build,
    )?;
    let prepared = prepare_publication(&audit, &options.source_build)?;
    validate_prepared_paths(&prepared)?;

    let effects_destination = asset_root.join(TUTORIAL_EFFECT_ROOT);
    let projectiles_destination = asset_root.join(TUTORIAL_PROJECTILE_ROOT);
    let previous = prepare_previous_install(
        &asset_root,
        &effects_destination,
        &projectiles_destination,
        manifest.as_mut(),
    )?;

    let token = unique_token();
    let stage = asset_root.join(format!(".tutorial-effects-stage-{token}"));
    if stage.exists() {
        restore_previous(&effects_destination, &projectiles_destination, &previous);
        return invalid(format!(
            "stale tutorial effect staging directory exists at {}",
            stage.display()
        ));
    }
    fs::create_dir_all(stage.join("effects")).map_err(|error| io_at(&stage, error))?;
    fs::create_dir_all(stage.join("projectiles")).map_err(|error| io_at(&stage, error))?;

    let staged_result = stage_preserved_effect_payloads(&stage, &previous)
        .and_then(|()| stage_prepared(&stage, &prepared, manifest.as_ref()));
    let (entries, installed_bytes) = match staged_result {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            restore_previous(&effects_destination, &projectiles_destination, &previous);
            return Err(error);
        }
    };

    let publish_result = publish_staged_trees(
        &stage,
        &effects_destination,
        &projectiles_destination,
        &previous,
    );
    if let Err(error) = publish_result {
        let _ = fs::remove_dir_all(&stage);
        restore_previous(&effects_destination, &projectiles_destination, &previous);
        return Err(error);
    }

    if let Some(manifest) = manifest.as_mut() {
        manifest.files.extend(entries);
        manifest
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        if let Err(error) = replace_manifest(&manifest_path, manifest, &token) {
            let _ = fs::remove_dir_all(&effects_destination);
            let _ = fs::remove_dir_all(&projectiles_destination);
            restore_previous(&effects_destination, &projectiles_destination, &previous);
            return Err(error);
        }
    }

    cleanup_previous(previous)?;
    let _ = fs::remove_dir_all(&stage);

    Ok(TutorialEffectInstallReport {
        schema: TUTORIAL_EFFECT_INSTALL_REPORT_SCHEMA.to_owned(),
        source_build: options.source_build.clone(),
        source_bundle_blake3: audit.bundle.blake3.clone(),
        source_bundle_bytes: audit.bundle.bytes,
        source_dump_blake3: audit.dump.blake3.clone(),
        source_dump_bytes: audit.dump.bytes,
        dependency_count: audit.source_assets.len().saturating_sub(1) as u64,
        effect_count: audit.effect_closures.len() as u64,
        projectile_effect_count: audit.projectile_effect_closures.len() as u64,
        bullet_row_count: audit.bullets.len() as u64,
        closure_object_count: audit
            .effect_closures
            .iter()
            .map(|closure| closure.objects.len() as u64)
            .sum::<u64>()
            .saturating_add(
                audit
                    .projectile_effect_closures
                    .iter()
                    .map(|closure| closure.objects.len() as u64)
                    .sum::<u64>(),
            )
            .saturating_add(audit.bullet_closure.objects.len() as u64),
        installed_files: prepared.len() as u64,
        installed_bytes,
        manifest_files: manifest
            .as_ref()
            .map_or(0, |manifest| manifest.files.len() as u64),
        effect_root: TUTORIAL_EFFECT_ROOT.to_owned(),
        projectile_root: TUTORIAL_PROJECTILE_ROOT.to_owned(),
        renderer_status: format!("{EFFECT_RENDERER_STATUS}; {PROJECTILE_RENDERER_STATUS}"),
    })
}

pub(super) fn prepare_previous_install(
    asset_root: &Path,
    effects: &Path,
    projectiles: &Path,
    manifest: Option<&mut ProjectAssetManifest>,
) -> Result<Vec<(OwnedTree, PathBuf)>> {
    let effect_exists = effects.exists();
    let projectile_exists = projectiles.exists();
    if effect_exists != projectile_exists {
        return invalid(
            "partial tutorial effects install exists; effects and projectiles must be owned together",
        );
    }
    if !effect_exists {
        return Ok(Vec::new());
    }
    reject_symlink(effects, "existing tutorial effects directory")?;
    reject_symlink(projectiles, "existing tutorial projectiles directory")?;
    verify_owned_catalog(
        &effects.join("catalog.json"),
        TUTORIAL_EFFECT_CATALOG_SCHEMA,
    )?;
    verify_owned_catalog(
        &projectiles.join("catalog.json"),
        TUTORIAL_PROJECTILE_CATALOG_SCHEMA,
    )?;

    let token = unique_token();
    let effect_backup = asset_root.join(format!(".tutorial-effects-backup-{token}"));
    let projectile_backup = asset_root.join(format!(".tutorial-projectiles-backup-{token}"));
    if effect_backup.exists() || projectile_backup.exists() {
        return invalid("tutorial effects backup collision");
    }
    fs::rename(effects, &effect_backup).map_err(|error| io_at(effects, error))?;
    if let Err(error) = fs::rename(projectiles, &projectile_backup) {
        let _ = fs::rename(&effect_backup, effects);
        return Err(io_at(projectiles, error));
    }
    if let Some(manifest) = manifest {
        manifest.files.retain(|entry| {
            is_preserved_effect_payload_path(&entry.path)
                || (!entry.path.starts_with(&format!("{TUTORIAL_EFFECT_ROOT}/"))
                    && !entry
                        .path
                        .starts_with(&format!("{TUTORIAL_PROJECTILE_ROOT}/")))
        });
    }
    Ok(vec![
        (OwnedTree::Effects, effect_backup),
        (OwnedTree::Projectiles, projectile_backup),
    ])
}

pub(super) fn copy_directory_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination).map_err(|error| io_at(destination, error))?;
    for entry in fs::read_dir(source).map_err(|error| io_at(source, error))? {
        let entry = entry.map_err(|error| io_at(source, error))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| io_at(&source_path, error))?;
        if file_type.is_symlink() {
            return invalid(format!(
                "tutorial effect model tree contains a symlink at {}",
                source_path.display()
            ));
        }
        if file_type.is_dir() {
            copy_directory_tree(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path)
                .map_err(|error| io_at(&destination_path, error))?;
        } else {
            return invalid(format!(
                "tutorial effect model tree contains an unsupported entry at {}",
                source_path.display()
            ));
        }
    }
    Ok(())
}

pub(super) fn publish_staged_trees(
    stage: &Path,
    effects_destination: &Path,
    projectiles_destination: &Path,
    previous: &[(OwnedTree, PathBuf)],
) -> Result<()> {
    if effects_destination.exists() || projectiles_destination.exists() {
        return invalid("tutorial effects destinations appeared during transaction");
    }
    if let Some(parent) = effects_destination.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let staged_effects = stage.join("effects");
    let staged_projectiles = stage.join("projectiles");
    if let Err(error) = fs::rename(&staged_effects, effects_destination) {
        restore_previous(effects_destination, projectiles_destination, previous);
        return Err(io_at(&staged_effects, error));
    }
    if let Err(error) = fs::rename(&staged_projectiles, projectiles_destination) {
        let _ = fs::remove_dir_all(effects_destination);
        restore_previous(effects_destination, projectiles_destination, previous);
        return Err(io_at(&staged_projectiles, error));
    }
    Ok(())
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options.open(path).map_err(|error| io_at(path, error))?;
    std::io::Write::write_all(&mut file, bytes).map_err(|error| io_at(path, error))?;
    file.sync_all().map_err(|error| io_at(path, error))
}
