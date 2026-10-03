use super::*;

pub const SEMANTIC_CHARACTER_INSTALL_REPORT_SCHEMA: &str =
    "ffone.semantic-character-install-report.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCharacterInstallOptions {
    pub candidate_root: PathBuf,
    pub gpu_audit_path: PathBuf,
    pub table_set_path: PathBuf,
    pub asset_root: PathBuf,
    pub source_build: String,
    pub report_output: PathBuf,
    /// Permit an exact candidate ID to replace its existing package and
    /// registry row in the same atomic transaction.
    pub replace_existing: bool,
}

impl SemanticCharacterInstallOptions {
    pub fn new(
        candidate_root: impl Into<PathBuf>,
        gpu_audit_path: impl Into<PathBuf>,
        table_set_path: impl Into<PathBuf>,
        asset_root: impl Into<PathBuf>,
        source_build: impl Into<String>,
        report_output: impl Into<PathBuf>,
    ) -> Self {
        Self {
            candidate_root: candidate_root.into(),
            gpu_audit_path: gpu_audit_path.into(),
            table_set_path: table_set_path.into(),
            asset_root: asset_root.into(),
            source_build: source_build.into(),
            report_output: report_output.into(),
            replace_existing: false,
        }
    }

    #[must_use]
    pub const fn with_replace_existing(mut self, replace_existing: bool) -> Self {
        self.replace_existing = replace_existing;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticCharacterInstallCounts {
    pub planned_sources: u64,
    pub candidate_models: u64,
    pub published_models: u64,
    pub replaced_models: u64,
    pub published_nanos: u64,
    pub published_npcs: u64,
    pub published_mobs: u64,
    pub published_fusions: u64,
    pub published_shared: u64,
    pub gpu_blocked_models: u64,
    pub unresolved_models: u64,
    pub source_blocked_models: u64,
    pub skipped_models: u64,
    pub installed_glbs: u64,
    pub installed_pngs: u64,
    pub installed_bytes: u64,
    pub manifest_files: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterInstallInput {
    pub role: String,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticCharacterInstallReport {
    pub schema: String,
    pub status: String,
    pub source_build: String,
    pub visual_parity_pending: bool,
    pub coordinate_contract: String,
    pub production_assets_mutated: bool,
    pub raw_recovery_models_mutated: bool,
    pub registry_path: String,
    pub registry_blake3: String,
    pub proofs: CharacterRegistryProofs,
    pub inputs: Vec<CharacterInstallInput>,
    pub counts: SemanticCharacterInstallCounts,
    pub published: Vec<PublishedCharacter>,
    pub skipped: Vec<SkippedCharacter>,
}

#[derive(Clone, Debug)]
pub(super) struct RuntimeCopy {
    pub(super) source: PathBuf,
    pub(super) destination: String,
    pub(super) kind: ProjectAssetKind,
}

pub fn install_semantic_characters(
    options: &SemanticCharacterInstallOptions,
) -> Result<SemanticCharacterInstallReport> {
    if options.source_build.trim().is_empty() {
        return invalid("source build identity may not be empty");
    }
    let candidate_root = canonical_directory(&options.candidate_root, "candidate root")?;
    let asset_root = canonical_directory(&options.asset_root, "asset root")?;
    let report_output = canonical_output_file(&options.report_output, &asset_root)?;
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let (mut manifest, mut preserved) = match fs::read(&manifest_path) {
        Ok(manifest_bytes) => {
            let manifest: ProjectAssetManifest =
                serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
                    path: manifest_path.display().to_string(),
                    source,
                })?;
            if manifest.schema != PROJECT_ASSET_SCHEMA {
                return invalid("project asset manifest has the wrong schema");
            }
            validate_existing_managed_tree(&asset_root, &manifest)?;
            let preserved = plan_preserved_character_content(&asset_root, &manifest)?;
            (Some(manifest), preserved)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            (None, plan_manifestless_character_content(&asset_root)?)
        }
        Err(error) => return Err(io_at(&manifest_path, error)),
    };

    let structural = audit_logical_model_tree(&candidate_root)?;
    if !structural.violations.is_empty() || structural.models.is_empty() {
        return invalid("candidate tree must have a clean structural audit");
    }
    let structural_paths = structural
        .models
        .iter()
        .map(|model| model.path.clone())
        .collect::<BTreeSet<_>>();

    let gpu_audit_bytes = read_file(&options.gpu_audit_path)?;
    let gpu_audit: LogicalModelGpuEvidenceAuditReport = serde_json::from_slice(&gpu_audit_bytes)
        .map_err(|source| PipelineError::Json {
            path: options.gpu_audit_path.display().to_string(),
            source,
        })?;
    if gpu_audit.schema != GPU_EVIDENCE_AUDIT_SCHEMA || !gpu_audit.structural_passed {
        return invalid("GPU audit has the wrong schema or failed structural gate");
    }
    let mut gpu_by_path = BTreeMap::new();
    for model in &gpu_audit.models {
        validate_relative(&model.relative_glb)?;
        if gpu_by_path
            .insert(model.relative_glb.clone(), model.clone())
            .is_some()
        {
            return invalid("GPU audit contains duplicate model paths");
        }
    }
    if gpu_by_path.keys().cloned().collect::<BTreeSet<_>>() != structural_paths {
        return invalid("GPU audit model set does not exactly match the candidate tree");
    }

    let batch_path = candidate_root.join(BATCH_REPORT_FILE);
    let batch_bytes = read_file(&batch_path)?;
    let batch: Value =
        serde_json::from_slice(&batch_bytes).map_err(|source| PipelineError::Json {
            path: batch_path.display().to_string(),
            source,
        })?;
    if batch.get("structuralAuditPassed").and_then(Value::as_bool) != Some(true) {
        return invalid("batch report does not carry a passing structural audit");
    }
    let mappings = value_array(batch.get("models"));
    let mapping_paths = mappings
        .iter()
        .map(|mapping| required_string(mapping, "outputGlb").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    if mapping_paths != structural_paths || mappings.len() != structural_paths.len() {
        return invalid("batch mappings do not exactly match the structural model set");
    }

    let table_bytes = read_file(&options.table_set_path)?;
    let table_set: Value =
        serde_json::from_slice(&table_bytes).map_err(|source| PipelineError::Json {
            path: options.table_set_path.display().to_string(),
            source,
        })?;
    let route_evidence = exact_character_route_evidence(&table_set)?;

    let mut selected_gpu = gpu_audit.clone();
    selected_gpu.models.clear();
    selected_gpu.violations.clear();
    let mut selections = Vec::new();
    let mut skipped = Vec::new();
    let mut gpu_blocked_count = 0_u64;
    let mut unresolved_count = 0_u64;

    for mapping in mappings {
        let logical_name = required_string(mapping, "logicalName")?.to_owned();
        let output_glb = required_string(mapping, "outputGlb")?.to_owned();
        let (legacy_route, semantic_path) = exact_route_from_mapping(mapping)?;
        let family = required_string(mapping, "family")?;
        let references = route_evidence
            .get(&legacy_route)
            .cloned()
            .unwrap_or_default();
        let classification = classify_route(&legacy_route, family, references);
        let gpu = gpu_by_path
            .get(&output_glb)
            .ok_or_else(|| invalid_error("GPU audit lost a candidate mapping"))?;
        let mut reasons = Vec::new();
        if !gpu.passed || !gpu.violations.is_empty() {
            gpu_blocked_count += 1;
            if gpu.violations.is_empty() {
                reasons.push(CharacterSkipReason {
                    code: "gpu_blocked".to_owned(),
                    detail: "independent GPU audit did not accept this model".to_owned(),
                });
            } else {
                reasons.extend(gpu.violations.iter().map(|violation| CharacterSkipReason {
                    code: format!("gpu_{}", violation.code),
                    detail: violation.message.clone(),
                }));
            }
        }
        let Some(category) = classification.category else {
            unresolved_count += 1;
            reasons.push(CharacterSkipReason {
                code: "classification_unresolved".to_owned(),
                detail: format!(
                    "exact route {legacy_route:?} has no single proven NPC/Mob/Nano/shared role"
                ),
            });
            skipped.push(SkippedCharacter {
                logical_name,
                legacy_route,
                candidate_glb: Some(output_glb),
                reasons,
            });
            continue;
        };
        if !reasons.is_empty() {
            skipped.push(SkippedCharacter {
                logical_name,
                legacy_route,
                candidate_glb: Some(output_glb),
                reasons,
            });
            continue;
        }
        selected_gpu.models.push(gpu.clone());
        selections.push((
            mapping,
            logical_name,
            output_glb,
            legacy_route,
            semantic_path,
            category,
            classification,
        ));
    }

    if selections.is_empty() {
        return invalid("no individually GPU-passed, strictly classified character models");
    }
    verify_archived_gpu_evidence(&candidate_root, &selected_gpu)?;

    let source_blockers = value_array(batch.get("blockers"));
    for blocker in source_blockers {
        let (route, _) = exact_route_from_source_blocker(blocker)?;
        skipped.push(SkippedCharacter {
            logical_name: required_string(blocker, "logicalName")?.to_owned(),
            legacy_route: route,
            candidate_glb: None,
            reasons: vec![CharacterSkipReason {
                code: format!(
                    "source_{}",
                    required_string(blocker, "code").unwrap_or("blocked")
                ),
                detail: string(blocker, "detail").unwrap_or_default().to_owned(),
            }],
        });
    }

    let mut replacement_package_roots = BTreeSet::new();
    for (_, logical_name, _, _, semantic_path, category, _) in &selections {
        let id = format!("{}/{}", category.label(), semantic_path);
        let existing = preserved
            .models
            .iter()
            .filter(|model| model.id == id)
            .collect::<Vec<_>>();
        if existing.len() > 1 {
            return invalid(format!(
                "preserved registry has duplicate replacement ID {id:?}"
            ));
        }
        let Some(existing) = existing.first() else {
            continue;
        };
        if !options.replace_existing {
            return invalid(format!(
                "candidate ID {id:?} already exists; explicit replacement mode is required"
            ));
        }
        let expected_package = format!("{}/{}", category.directory(), semantic_path);
        let actual_package = runtime_character_package_root(&existing.glb)?.ok_or_else(|| {
            invalid_error(format!(
                "existing replacement GLB is outside a runtime package: {:?}",
                existing.glb
            ))
        })?;
        if existing.logical_name != *logical_name
            || existing.category != *category
            || actual_package != expected_package
        {
            return invalid(format!(
                "candidate replacement identity contradicts existing registry row {id:?}"
            ));
        }
        replacement_package_roots.insert(actual_package);
    }
    if !replacement_package_roots.is_empty() {
        preserved.models.retain(|model| {
            runtime_character_package_root(&model.glb)
                .ok()
                .flatten()
                .is_none_or(|package| !replacement_package_roots.contains(&package))
        });
        preserved.entries.retain(|entry| {
            !replacement_package_roots
                .iter()
                .any(|package| entry.path.starts_with(&format!("{package}/")))
        });
    }

    let stage = create_transaction_directory(&asset_root, "stage")?;
    for category in [
        RuntimeCharacterCategory::Nano,
        RuntimeCharacterCategory::Npc,
        RuntimeCharacterCategory::Mob,
        RuntimeCharacterCategory::Fusion,
        RuntimeCharacterCategory::Shared,
    ] {
        let directory = join_relative(&stage, category.directory())?;
        fs::create_dir_all(&directory).map_err(|error| io_at(&directory, error))?;
    }
    if manifest.is_some() {
        stage_preserved_character_content(&asset_root, &stage, &preserved)?;
    }

    let mut registry_models = preserved.models.clone();
    let mut copies = Vec::new();
    let mut published = Vec::new();
    for (mapping, logical_name, output_glb, legacy_route, semantic_path, category, proof) in
        selections
    {
        let destination_directory = format!("{}/{}", category.directory(), semantic_path);
        let inspected = build_catalog_model_for_destination(
            &candidate_root,
            &output_glb,
            mapping,
            category.label(),
            &destination_directory,
        )?;
        if inspected.logical_name != logical_name
            || inspected.family != required_string(mapping, "family")?
        {
            return invalid("candidate publish contract disagrees with its batch mapping");
        }
        let model_copies =
            runtime_closure_files(&candidate_root, &output_glb, &destination_directory)?;
        let destination_paths = model_copies
            .iter()
            .map(|copy| copy.destination.as_str())
            .collect::<BTreeSet<_>>();
        if !destination_paths.contains(inspected.glb.as_str()) {
            return invalid("runtime copy set does not contain the inspected GLB");
        }
        for texture in &inspected.material_shader_metadata.textures {
            if !destination_paths.contains(texture.path.as_str())
                || texture
                    .mip_paths
                    .iter()
                    .any(|path| !destination_paths.contains(path.as_str()))
            {
                return invalid("runtime copy set is missing an inspected material texture");
            }
        }
        copies.extend(model_copies);
        let mut roles = proof
            .references
            .iter()
            .map(|reference| reference.role.label().to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        roles.sort();
        let id = format!("{}/{}", category.label(), semantic_path);
        let mut animation_names = inspected.animations.names.clone();
        animation_names.sort();
        animation_names.dedup();
        registry_models.push(RuntimeCharacterModel {
            id: id.clone(),
            logical_name: logical_name.clone(),
            legacy_aliases: legacy_aliases(&legacy_route, &logical_name)?,
            category,
            glb: inspected.glb.clone(),
            glb_blake3: inspected.glb_blake3.clone(),
            collision: None,
            animations: animation_names,
        });
        published.push(PublishedCharacter {
            id,
            logical_name,
            category,
            legacy_route,
            glb: inspected.glb,
            glb_blake3: inspected.glb_blake3,
            source_sha256: inspected.source_sha256,
            runtime_spawn_policy: category.spawn_policy().to_owned(),
            classification: PublishedCharacterClassification {
                method: proof.method.to_owned(),
                table_reference_count: proof.references.len() as u64,
                observed_roles: roles,
            },
            authored_root: inspected.authored_root,
            rig: inspected.rig,
            animations: inspected.animations,
            materials: PublishedCharacterMaterialSummary {
                material_count: inspected.material_shader_metadata.material_count,
                texture_count: inspected.material_shader_metadata.texture_count,
                sampler_count: inspected.material_shader_metadata.sampler_count,
                shaders: inspected.material_shader_metadata.shaders,
                textures: inspected
                    .material_shader_metadata
                    .textures
                    .into_iter()
                    .map(|texture| PublishedCharacterTexture {
                        source_name: texture.source_name,
                        path: texture.path,
                        png_sha256: texture.png_sha256,
                        mip_paths: texture.mip_paths,
                    })
                    .collect(),
            },
        });
    }

    registry_models.sort_by(|left, right| left.glb.cmp(&right.glb));
    ensure_unique_registry_models(&registry_models)?;
    published.sort_by(|left, right| left.glb.cmp(&right.glb));
    skipped.sort_by(|left, right| {
        left.legacy_route
            .cmp(&right.legacy_route)
            .then_with(|| left.logical_name.cmp(&right.logical_name))
    });
    copies.sort_by(|left, right| left.destination.cmp(&right.destination));
    ensure_unique_copy_destinations(&copies)?;

    let glbs = copies
        .iter()
        .filter(|copy| copy.kind == ProjectAssetKind::Model)
        .count() as u64;
    let pngs = copies
        .iter()
        .filter(|copy| copy.kind == ProjectAssetKind::Texture)
        .count() as u64;
    let registry = SemanticCharacterRegistry {
        schema: SEMANTIC_CHARACTER_REGISTRY_SCHEMA.to_owned(),
        models: registry_models,
    };
    let registry_bytes = pretty_json(&registry, SEMANTIC_CHARACTER_REGISTRY_PATH)?;

    let retained_paths = if let Some(manifest) = manifest.as_mut() {
        let preserved_paths = preserved
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<BTreeSet<_>>();
        manifest.files.retain(|entry| {
            !is_managed_path(&entry.path) || preserved_paths.contains(entry.path.as_str())
        });
        manifest
            .files
            .iter()
            .map(|entry| entry.path.to_ascii_lowercase())
            .collect::<BTreeSet<_>>()
    } else {
        preserved
            .entries
            .iter()
            .map(|entry| entry.path.to_ascii_lowercase())
            .collect::<BTreeSet<_>>()
    };
    let mut planned_paths = BTreeSet::new();
    let mut new_entries = Vec::with_capacity(copies.len() + 1);
    for copy in &copies {
        reserve_destination(&retained_paths, &mut planned_paths, &copy.destination)?;
        let bytes = read_file(&copy.source)?;
        write_new_file(&stage, &copy.destination, &bytes)?;
        new_entries.push(ProjectAssetFile {
            source_path: format!(
                "{SEMANTIC_CHARACTER_SOURCE_PREFIX}{}",
                relative_path(&candidate_root, &copy.source)?
            ),
            path: copy.destination.clone(),
            kind: copy.kind,
            bytes: bytes.len() as u64,
            blake3: blake3::hash(&bytes).to_hex().to_string(),
        });
    }
    reserve_destination(
        &retained_paths,
        &mut planned_paths,
        SEMANTIC_CHARACTER_REGISTRY_PATH,
    )?;
    write_new_file(&stage, SEMANTIC_CHARACTER_REGISTRY_PATH, &registry_bytes)?;
    new_entries.push(ProjectAssetFile {
        source_path: format!("{SEMANTIC_CHARACTER_SOURCE_PREFIX}registry.json"),
        path: SEMANTIC_CHARACTER_REGISTRY_PATH.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: registry_bytes.len() as u64,
        blake3: blake3::hash(&registry_bytes).to_hex().to_string(),
    });
    let next_manifest_bytes = if let Some(manifest) = manifest.as_mut() {
        manifest.files.extend(new_entries.iter().cloned());
        manifest
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        Some(pretty_json(manifest, ASSET_MANIFEST_FILE)?)
    } else {
        None
    };

    let installed_bytes = new_entries.iter().map(|entry| entry.bytes).sum::<u64>();
    let counts = SemanticCharacterInstallCounts {
        planned_sources: mappings.len() as u64 + source_blockers.len() as u64,
        candidate_models: mappings.len() as u64,
        published_models: published.len() as u64,
        replaced_models: replacement_package_roots.len() as u64,
        published_nanos: published
            .iter()
            .filter(|model| model.category == RuntimeCharacterCategory::Nano)
            .count() as u64,
        published_npcs: published
            .iter()
            .filter(|model| model.category == RuntimeCharacterCategory::Npc)
            .count() as u64,
        published_mobs: published
            .iter()
            .filter(|model| model.category == RuntimeCharacterCategory::Mob)
            .count() as u64,
        published_fusions: published
            .iter()
            .filter(|model| model.category == RuntimeCharacterCategory::Fusion)
            .count() as u64,
        published_shared: published
            .iter()
            .filter(|model| model.category == RuntimeCharacterCategory::Shared)
            .count() as u64,
        gpu_blocked_models: gpu_blocked_count,
        unresolved_models: unresolved_count,
        source_blocked_models: source_blockers.len() as u64,
        skipped_models: skipped.len() as u64,
        installed_glbs: glbs,
        installed_pngs: pngs,
        installed_bytes,
        manifest_files: manifest
            .as_ref()
            .map(|manifest| manifest.files.len() as u64)
            .unwrap_or_else(|| (preserved.entries.len() + new_entries.len()) as u64),
    };
    let report = SemanticCharacterInstallReport {
        schema: SEMANTIC_CHARACTER_INSTALL_REPORT_SCHEMA.to_owned(),
        status: "committed".to_owned(),
        source_build: options.source_build.trim().to_owned(),
        visual_parity_pending: true,
        coordinate_contract: COORDINATE_CONTRACT.to_owned(),
        production_assets_mutated: true,
        raw_recovery_models_mutated: false,
        registry_path: SEMANTIC_CHARACTER_REGISTRY_PATH.to_owned(),
        registry_blake3: blake3::hash(&registry_bytes).to_hex().to_string(),
        proofs: CharacterRegistryProofs {
            batch_report_sha256: sha256(&batch_bytes),
            gpu_audit_sha256: sha256(&gpu_audit_bytes),
            table_set_sha256: sha256(&table_bytes),
            structural_audit_schema: LOGICAL_MODEL_TREE_AUDIT_SCHEMA.to_owned(),
            gpu_audit_schema: gpu_audit.schema.clone(),
            exact_route_policy: "candidate family/semanticDirectories exact KFM route + consolidated table role + named authoring-role overrides"
                .to_owned(),
        },
        inputs: vec![
            input_proof("logical-model-batch", &batch_path, &batch_bytes),
            input_proof("gpu-audit", &options.gpu_audit_path, &gpu_audit_bytes),
            input_proof("table-set", &options.table_set_path, &table_bytes),
        ],
        counts,
        published,
        skipped,
    };
    let report_bytes = pretty_json(
        &report,
        report_output
            .to_str()
            .unwrap_or("semantic-character-install-report.json"),
    )?;
    if let Some(next_manifest_bytes) = next_manifest_bytes {
        commit_transaction(
            &asset_root,
            &stage,
            &next_manifest_bytes,
            &report_output,
            &report_bytes,
        )?;
    } else {
        let package_roots = report
            .published
            .iter()
            .map(|model| {
                runtime_character_package_root(&model.glb)?.ok_or_else(|| {
                    invalid_error(format!(
                        "published character GLB is outside a runtime package: {:?}",
                        model.glb
                    ))
                })
            })
            .collect::<Result<BTreeSet<_>>>()?;
        commit_manifestless_transaction(
            &asset_root,
            &stage,
            &package_roots,
            &replacement_package_roots,
            &report_output,
            &report_bytes,
        )?;
    }
    Ok(report)
}

pub(super) fn ensure_unique_copy_destinations(copies: &[RuntimeCopy]) -> Result<()> {
    let mut paths = BTreeSet::new();
    for copy in copies {
        validate_relative(&copy.destination)?;
        if !paths.insert(copy.destination.to_ascii_lowercase()) {
            return invalid(format!(
                "case-insensitive semantic character destination collision: {:?}",
                copy.destination
            ));
        }
    }
    Ok(())
}

pub(super) fn write_new_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<()> {
    let path = join_relative(root, relative)?;
    create_parent(&path)?;
    write_exact_new_file(&path, bytes)
}

pub(super) fn write_exact_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
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
