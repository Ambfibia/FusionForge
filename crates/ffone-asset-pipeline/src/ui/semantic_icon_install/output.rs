use super::*;

#[derive(Clone, Debug)]
pub struct SemanticIconInstallOptions {
    pub asset_root: PathBuf,
    pub table_set: PathBuf,
    pub source_build: String,
}

impl SemanticIconInstallOptions {
    pub fn new(
        asset_root: impl Into<PathBuf>,
        table_set: impl Into<PathBuf>,
        source_build: impl Into<String>,
    ) -> Self {
        Self {
            asset_root: asset_root.into(),
            table_set: table_set.into(),
            source_build: source_build.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticIconInstallReport {
    pub schema: &'static str,
    pub source_build: String,
    pub published: u64,
    pub unmatched: u64,
    pub copied_bytes: u64,
    pub manifest_files: u64,
    pub catalog: SemanticIconCatalog,
    pub unmatched_entries: Vec<SemanticIconUnmatched>,
}

pub fn install_semantic_icons(
    options: &SemanticIconInstallOptions,
) -> Result<SemanticIconInstallReport> {
    validate_options(options)?;
    let manifest_path = options.asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid(format!(
            "project manifest schema must be {PROJECT_ASSET_SCHEMA:?}, got {:?}",
            manifest.schema
        ));
    }
    validate_previous_install(&options.asset_root, &manifest)?;

    let table_set_bytes =
        fs::read(&options.table_set).map_err(|error| io_at(&options.table_set, error))?;
    let table_set: TableSetDocument =
        serde_json::from_slice(&table_set_bytes).map_err(|source| PipelineError::Json {
            path: options.table_set.display().to_string(),
            source,
        })?;
    if table_set.schema != TABLE_SET_SCHEMA {
        return invalid(format!(
            "TableData schema must be {TABLE_SET_SCHEMA:?}, got {:?}",
            table_set.schema
        ));
    }
    let table_manifest = verify_table_set(&options.asset_root, &manifest, &options.table_set)?;

    let references = collect_table_references(&table_set)?;
    let texture_index = build_texture_index(&manifest)?;
    let (mut prepared, mut unmatched, consumed) = classify_references(&references, &texture_index)?;
    append_unreferenced_textures(&manifest, &consumed, &mut unmatched)?;
    prepared.sort_by(|left, right| left.destination.cmp(&right.destination));
    unmatched.sort_by(|left, right| {
        left.legacy_true_name
            .cmp(&right.legacy_true_name)
            .then_with(|| format!("{:?}", left.reason).cmp(&format!("{:?}", right.reason)))
    });

    let mut category_counts = SemanticIconCategory::all()
        .into_iter()
        .map(|category| (category, 0_u64))
        .collect::<BTreeMap<_, _>>();
    for icon in &prepared {
        *category_counts
            .get_mut(&icon.catalog.classification.category)
            .expect("all semantic categories are initialized") += 1;
    }
    let reason_counts = unmatched_reason_counts(&unmatched);
    let counts = SemanticIconCatalogCounts {
        raw_table_references: references
            .values()
            .map(|group| group.references.len() as u64)
            .sum(),
        unique_table_icons: references.len() as u64,
        published: prepared.len() as u64,
        unmatched: unmatched.len() as u64,
        missing: reason_count(&reason_counts, "missing_texture"),
        ambiguous: reason_count(&reason_counts, "ambiguous_texture"),
        unsupported_types: reason_count(&reason_counts, "unsupported_icon_type"),
        unreferenced_legacy_textures: reason_count(&reason_counts, "unreferenced_legacy_texture"),
        name_only_unclassified: reason_count(&reason_counts, "name_only_no_table_data_proof"),
        by_category: category_counts,
    };
    let source_manifest_without_owned_icons_blake3 =
        canonical_source_manifest_hash(&manifest, &manifest_path)?;
    let proofs = SemanticIconCatalogProofs {
        source_build: options.source_build.clone(),
        source_manifest_path: ASSET_MANIFEST_FILE.to_owned(),
        source_manifest_schema: manifest.schema.clone(),
        source_manifest_without_owned_icons_blake3,
        table_set_path: manifest_relative_path(&options.asset_root, &options.table_set)?,
        table_set_manifest_source_path: table_manifest.source_path.clone(),
        table_set_schema: table_set.schema,
        table_set_bytes: table_manifest.bytes,
        table_set_blake3: table_manifest.blake3.clone(),
        classification_policy: "exact TableData m_iIconType/m_iIconNumber route plus one exact manifest texture stem; names alone never classify",
    };
    let catalog = SemanticIconCatalog {
        schema: SEMANTIC_ICON_CATALOG_SCHEMA,
        proofs,
        counts,
        assets: prepared.iter().map(|icon| icon.catalog.clone()).collect(),
    };

    let stage = options.asset_root.join(".semantic-icons.stage");
    let backup = options.asset_root.join(".semantic-icons.backup");
    if stage.exists() || backup.exists() {
        return invalid(format!(
            "stale semantic-icon transaction directory at {} or {}",
            stage.display(),
            backup.display()
        ));
    }
    fs::create_dir(&stage).map_err(|error| io_at(&stage, error))?;
    let result = stage_install(&options.asset_root, &stage, &prepared);
    let (mut new_entries, copied_bytes) = match result {
        Ok(result) => result,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
    };

    let mut next_manifest = manifest;
    next_manifest
        .files
        .retain(|entry| !entry.path.starts_with("icons/"));
    next_manifest.files.append(&mut new_entries);
    next_manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    validate_manifest_paths(&next_manifest)?;
    if let Err(error) = commit_install(
        &options.asset_root,
        &stage,
        &backup,
        &manifest_path,
        &next_manifest,
    ) {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    Ok(SemanticIconInstallReport {
        schema: SEMANTIC_ICON_CATALOG_SCHEMA,
        source_build: options.source_build.clone(),
        published: catalog.counts.published,
        unmatched: catalog.counts.unmatched,
        copied_bytes,
        manifest_files: next_manifest.files.len() as u64,
        catalog,
        unmatched_entries: unmatched,
    })
}

pub(super) fn stage_install(
    asset_root: &Path,
    stage: &Path,
    icons: &[PreparedIcon],
) -> Result<(Vec<ProjectAssetFile>, u64)> {
    for directory in CATEGORY_DIRECTORIES {
        let path = stage.join(directory.replace('/', std::path::MAIN_SEPARATOR_STR));
        fs::create_dir_all(&path).map_err(|error| io_at(&path, error))?;
    }
    let mut manifest_entries = Vec::with_capacity(icons.len());
    let mut copied_bytes = 0_u64;
    for icon in icons {
        let source = join_manifest_path(asset_root, &icon.source.path)?;
        let source_meta = fs::symlink_metadata(&source).map_err(|error| io_at(&source, error))?;
        if !source_meta.is_file() || source_meta.file_type().is_symlink() {
            return invalid(format!(
                "icon source {} must be a real file",
                source.display()
            ));
        }
        let bytes = fs::read(&source).map_err(|error| io_at(&source, error))?;
        let actual_hash = blake3::hash(&bytes).to_hex().to_string();
        if icon.source.bytes != bytes.len() as u64 || icon.source.blake3 != actual_hash {
            return invalid(format!(
                "source byte identity mismatch for {:?}",
                icon.source.path
            ));
        }
        let relative = icon
            .destination
            .strip_prefix("icons/")
            .ok_or_else(|| invalid_error("semantic icon destination is outside icons/"))?;
        let destination = join_manifest_path(stage, relative)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
        }
        fs::write(&destination, &bytes).map_err(|error| io_at(&destination, error))?;
        copied_bytes += bytes.len() as u64;
        manifest_entries.push(ProjectAssetFile {
            source_path: format!("{OWNED_SOURCE_PREFIX}{}", icon.source.path),
            path: icon.destination.clone(),
            kind: ProjectAssetKind::Texture,
            bytes: bytes.len() as u64,
            blake3: actual_hash,
        });
    }
    Ok((manifest_entries, copied_bytes))
}

pub(super) fn commit_install(
    asset_root: &Path,
    stage: &Path,
    backup: &Path,
    manifest_path: &Path,
    manifest: &ProjectAssetManifest,
) -> Result<()> {
    let icons_root = asset_root.join(ICON_ROOT);
    let had_previous = icons_root.exists();
    if had_previous {
        fs::rename(&icons_root, backup).map_err(|error| io_at(&icons_root, error))?;
    }
    if let Err(error) = fs::rename(stage, &icons_root) {
        if had_previous {
            let _ = fs::rename(backup, &icons_root);
        }
        return Err(io_at(&icons_root, error));
    }
    if let Err(error) = replace_manifest(manifest_path, manifest) {
        let _ = fs::rename(&icons_root, stage);
        if had_previous {
            let _ = fs::rename(backup, &icons_root);
        }
        return Err(error);
    }
    if had_previous {
        fs::remove_dir_all(backup).map_err(|error| io_at(backup, error))?;
    }
    Ok(())
}
