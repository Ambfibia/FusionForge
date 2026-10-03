use super::*;

pub(super) const fn candidate(
    id: &'static str,
    glb_name: &'static str,
    category: RuntimeCharacterCategory,
) -> PromotionCandidate {
    PromotionCandidate {
        id,
        glb_name,
        category,
    }
}

pub fn promote_tutorial_characters(
    options: &TutorialCharacterPromotionOptions,
) -> Result<TutorialCharacterPromotionReport> {
    let plan = build_plan(options)?;
    if options.apply {
        if plan.report.counts.blocked != 0 {
            return invalid(format!(
                "refusing --apply because {} required promotion candidates are blocked",
                plan.report.counts.blocked
            ));
        }
        if !plan.promotions.is_empty() {
            apply_plan(&plan)?;
        }
    }
    let mut report = plan.report;
    report.mode = if options.apply {
        TutorialCharacterPromotionMode::Apply
    } else {
        TutorialCharacterPromotionMode::DryRun
    };
    report.applied = options.apply && report.counts.ready != 0;
    Ok(report)
}

pub(super) fn build_plan(options: &TutorialCharacterPromotionOptions) -> Result<PromotionPlan> {
    let asset_root = canonical_plain_directory(&options.asset_root, "asset root")?;
    let candidate_root = canonical_plain_directory(&options.candidate_root, "candidate root")?;
    let source_root = canonical_plain_directory(&options.source_root, "source root")?;
    let evidence_root = canonical_plain_directory(&options.gpu_evidence_root, "GPU evidence root")?;
    reject_overlaps(&asset_root, &candidate_root, &source_root, &evidence_root)?;

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let registry_path = asset_root.join(native_path(SEMANTIC_CHARACTER_REGISTRY_PATH));
    let manifest_before = read_regular(&manifest_path)?;
    let registry_before = read_regular(&registry_path)?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_before).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    let registry: SemanticCharacterRegistry =
        serde_json::from_slice(&registry_before).map_err(|source| PipelineError::Json {
            path: registry_path.display().to_string(),
            source,
        })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid("project asset manifest has the wrong schema");
    }
    if registry.schema != SEMANTIC_CHARACTER_REGISTRY_SCHEMA {
        return invalid("semantic character registry has the wrong schema");
    }
    let manifest_index = index_manifest(&manifest)?;
    validate_registry(&registry)?;
    validate_registry_manifest_identity(&manifest_index, &registry_before)?;

    let batch_path = candidate_root.join(LOGICAL_MODEL_BATCH_REPORT_FILE);
    let batch_bytes = read_regular(&batch_path)?;
    let batch: LogicalModelBatchPublishReport =
        serde_json::from_slice(&batch_bytes).map_err(|source| PipelineError::Json {
            path: batch_path.display().to_string(),
            source,
        })?;
    if batch.schema != LOGICAL_MODEL_BATCH_REPORT_SCHEMA || !batch.structural_audit_passed {
        return invalid("candidate batch is not a structurally passing logical-model batch");
    }
    let mappings = index_batch(&batch.models)?;

    let mut models = Vec::with_capacity(CANDIDATES.len());
    let mut promotions = Vec::new();
    let mut counts = TutorialCharacterPromotionCounts {
        candidates: CANDIDATES.len() as u64,
        registry_models_before: registry.models.len() as u64,
        manifest_files_before: manifest.files.len() as u64,
        ..Default::default()
    };
    for candidate in CANDIDATES {
        let source_package_root = format!("{TUTORIAL_CHARACTER_ROOT}/{}", candidate.id);
        let target_package_root = format!(
            "{}/{}",
            category_directory(candidate.category),
            candidate.id
        );
        let target_glb = format!("{target_package_root}/{}", candidate.glb_name);
        match prepare_promotion(
            candidate,
            &asset_root,
            &candidate_root,
            &source_root,
            &evidence_root,
            &manifest,
            &manifest_index,
            &registry,
            &mappings,
        ) {
            Ok(PreparedState::Promote(prepared)) => {
                counts.ready += 1;
                counts.promoted_files += prepared.target_entries.len() as u64;
                counts.promoted_bytes += prepared
                    .target_entries
                    .iter()
                    .map(|entry| entry.bytes)
                    .sum::<u64>();
                models.push(TutorialCharacterPromotionModel {
                    id: candidate.id.to_owned(),
                    category: candidate.category,
                    tutorial_root: source_package_root,
                    character_root: target_package_root,
                    character_glb: target_glb,
                    status: TutorialCharacterPromotionStatus::Promote,
                    proof: Some(prepared.proof.clone()),
                    blockers: Vec::new(),
                });
                promotions.push(prepared);
            }
            Ok(PreparedState::AlreadyPromoted(proof)) => {
                counts.already_promoted += 1;
                models.push(TutorialCharacterPromotionModel {
                    id: candidate.id.to_owned(),
                    category: candidate.category,
                    tutorial_root: source_package_root,
                    character_root: target_package_root,
                    character_glb: target_glb,
                    status: TutorialCharacterPromotionStatus::AlreadyPromoted,
                    proof: Some(proof),
                    blockers: Vec::new(),
                });
            }
            Err(error) => {
                counts.blocked += 1;
                models.push(TutorialCharacterPromotionModel {
                    id: candidate.id.to_owned(),
                    category: candidate.category,
                    tutorial_root: source_package_root,
                    character_root: target_package_root,
                    character_glb: target_glb,
                    status: TutorialCharacterPromotionStatus::Blocked,
                    proof: None,
                    blockers: vec![TutorialCharacterPromotionBlocker {
                        code: "proof_failed".to_owned(),
                        detail: error.to_string(),
                    }],
                });
            }
        }
    }

    let (next_manifest, next_registry) = build_next_metadata(&manifest, &registry, &promotions)?;
    counts.registry_models_after = next_registry.models.len() as u64;
    let registry_after = pretty_json(&next_registry, SEMANTIC_CHARACTER_REGISTRY_PATH)?;
    let mut next_manifest = next_manifest;
    update_registry_manifest_entry(&mut next_manifest, &registry_after)?;
    counts.manifest_files_after = next_manifest.files.len() as u64;
    let manifest_after = pretty_json(&next_manifest, ASSET_MANIFEST_FILE)?;

    Ok(PromotionPlan {
        asset_root,
        manifest_path,
        registry_path,
        manifest_before,
        registry_before,
        manifest_after,
        registry_after,
        promotions,
        report: TutorialCharacterPromotionReport {
            schema: TUTORIAL_CHARACTER_PROMOTION_SCHEMA.to_owned(),
            mode: TutorialCharacterPromotionMode::DryRun,
            applied: false,
            requires_asset_index_regeneration: counts.ready != 0,
            excluded_tutorial_props: vec!["etc_domeglass_04".to_owned(), "npc_building".to_owned()],
            missing_models_not_synthesized: vec!["nano_buttercup".to_owned()],
            counts,
            models,
        },
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_promotion(
    candidate: &PromotionCandidate,
    asset_root: &Path,
    candidate_root: &Path,
    source_documents_root: &Path,
    evidence_root: &Path,
    manifest: &ProjectAssetManifest,
    manifest_index: &BTreeMap<String, ProjectAssetFile>,
    registry: &SemanticCharacterRegistry,
    mappings: &BTreeMap<String, LogicalModelBatchMapping>,
) -> Result<PreparedState> {
    let source_root = format!("{TUTORIAL_CHARACTER_ROOT}/{}", candidate.id);
    let target_root = format!(
        "{}/{}",
        category_directory(candidate.category),
        candidate.id
    );
    let source_path = asset_root.join(native_path(&source_root));
    let target_path = asset_root.join(native_path(&target_root));
    let source_exists = regular_directory_exists(&source_path)?;
    let target_exists = regular_directory_exists(&target_path)?;
    let registry_id = format!("{}/{}", category_label(candidate.category), candidate.id);
    let registry_matches = registry
        .models
        .iter()
        .filter(|model| model.id.eq_ignore_ascii_case(&registry_id))
        .collect::<Vec<_>>();
    if registry_matches.len() > 1 {
        return invalid(format!("registry has duplicate id {registry_id:?}"));
    }

    match (source_exists, target_exists) {
        (true, false) => {
            if !registry_matches.is_empty() {
                return invalid(format!(
                    "tutorial source still exists but registry id {registry_id:?} is already occupied"
                ));
            }
            if !manifest_paths_below(manifest, &target_root).is_empty() {
                return invalid(format!(
                    "target {target_root:?} is absent on disk but present in the manifest"
                ));
            }
            let proof = prove_package(
                candidate,
                &source_path,
                &source_root,
                candidate_root,
                source_documents_root,
                evidence_root,
                manifest,
                manifest_index,
                mappings,
            )?;
            let source_tree = collect_tree(&source_path, false)?;
            let mut source_files = Vec::with_capacity(source_tree.len());
            let mut target_entries = Vec::with_capacity(source_tree.len());
            for (relative, identity) in &source_tree {
                source_files.push(source_path.join(native_path(relative)));
                let source_manifest_path = format!("{source_root}/{relative}");
                let source_entry = manifest_index.get(&source_manifest_path).ok_or_else(|| {
                    invalid_error(format!("missing manifest entry {source_manifest_path:?}"))
                })?;
                target_entries.push(ProjectAssetFile {
                    source_path: format!(
                        "native-tutorial-character-promotion/models/mob/{}/{}",
                        candidate.id, relative
                    ),
                    path: format!("{target_root}/{relative}"),
                    kind: source_entry.kind,
                    bytes: identity.bytes,
                    blake3: identity.blake3.clone(),
                });
            }
            target_entries.sort_by(|left, right| left.path.cmp(&right.path));
            source_files.sort();
            let registry_model = RuntimeCharacterModel {
                id: registry_id,
                logical_name: proof_glb_true_name(candidate_root, candidate)?,
                legacy_aliases: Vec::new(),
                category: candidate.category,
                glb: format!("{target_root}/{}", candidate.glb_name),
                glb_blake3: proof.glb_blake3.clone(),
                collision: None,
                animations: proof.standard_animations.clone(),
            };
            Ok(PreparedState::Promote(PreparedPromotion {
                candidate: *candidate,
                source_root,
                target_root,
                source_files,
                target_entries,
                registry_model,
                proof,
            }))
        }
        (false, true) => {
            if !manifest_paths_below(manifest, &source_root).is_empty() {
                return invalid(format!(
                    "tutorial source {source_root:?} is absent on disk but remains in the manifest"
                ));
            }
            let proof = prove_package(
                candidate,
                &target_path,
                &target_root,
                candidate_root,
                source_documents_root,
                evidence_root,
                manifest,
                manifest_index,
                mappings,
            )?;
            let model = registry_matches.first().ok_or_else(|| {
                invalid_error(format!(
                    "promoted package has no registry id {registry_id:?}"
                ))
            })?;
            let expected_glb = format!("{target_root}/{}", candidate.glb_name);
            if model.category != candidate.category
                || model.glb != expected_glb
                || model.glb_blake3 != proof.glb_blake3
                || model.animations != proof.standard_animations
            {
                return invalid(format!(
                    "promoted registry entry {registry_id:?} does not match its exact package proof"
                ));
            }
            Ok(PreparedState::AlreadyPromoted(proof))
        }
        (true, true) => invalid(format!(
            "both tutorial and general packages exist for {:?}; refusing to guess ownership",
            candidate.id
        )),
        (false, false) => invalid(format!(
            "neither tutorial nor general package exists for {:?}",
            candidate.id
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prove_package(
    candidate: &PromotionCandidate,
    installed_package: &Path,
    installed_manifest_root: &str,
    candidate_root: &Path,
    source_documents_root: &Path,
    evidence_root: &Path,
    manifest: &ProjectAssetManifest,
    manifest_index: &BTreeMap<String, ProjectAssetFile>,
    mappings: &BTreeMap<String, LogicalModelBatchMapping>,
) -> Result<TutorialCharacterPromotionProof> {
    let candidate_glb = format!("models/mob/{}/{}", candidate.id, candidate.glb_name);
    let mapping = mappings
        .get(&candidate_glb)
        .ok_or_else(|| invalid_error(format!("batch has no exact mapping {candidate_glb:?}")))?;
    if mapping.family != "mob"
        || mapping.semantic_directories != [candidate.id]
        || mapping.output_glb != candidate_glb
    {
        return invalid(format!(
            "batch mapping identity is not exact for {:?}",
            candidate.id
        ));
    }
    require_sha256(&mapping.source_sha256, "source document SHA-256")?;
    let source_document_path = source_documents_root.join(native_path(&mapping.source));
    let source_document_bytes = read_regular(&source_document_path)?;
    if sha256(&source_document_bytes) != mapping.source_sha256 {
        return invalid(format!(
            "source document identity changed for {:?}",
            candidate.id
        ));
    }

    let candidate_parent =
        candidate_root.join(native_path(&format!("models/mob/{}", candidate.id)));
    let candidate_tree = collect_tree(&candidate_parent, true)?;
    let installed_tree = collect_tree(installed_package, false)?;
    if candidate_tree != installed_tree {
        return invalid(format!(
            "installed runtime package is not byte-identical to candidate closure for {:?}",
            candidate.id
        ));
    }
    validate_tree_manifest(
        installed_manifest_root,
        &installed_tree,
        manifest,
        manifest_index,
    )?;

    let glb_path = candidate_parent.join(candidate.glb_name);
    let glb = read_regular(&glb_path)?;
    let facts = gpu_model_facts_from_glb(&glb).map_err(|error| {
        invalid_error(format!("GLB facts failed for {:?}: {error}", candidate.id))
    })?;
    if facts.true_name != mapping.logical_name {
        return invalid(format!(
            "GLB true name differs from batch logical name for {:?}",
            candidate.id
        ));
    }

    let publish_relative = candidate_glb
        .strip_suffix(".glb")
        .expect("fixed candidate GLB")
        .to_owned()
        + ".publish.json";
    let publish_path = candidate_root.join(native_path(&publish_relative));
    let publish_bytes = read_regular(&publish_path)?;
    let publish: Value =
        serde_json::from_slice(&publish_bytes).map_err(|source| PipelineError::Json {
            path: publish_path.display().to_string(),
            source,
        })?;
    if publish.get("schema").and_then(Value::as_str) != Some(LOGICAL_MODEL_PUBLISH_REPORT_SCHEMA)
        || publish
            .pointer("/semanticProof/matched")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return invalid(format!(
            "publish report lacks a passing semantic proof for {:?}",
            candidate.id
        ));
    }
    let contract_value = publish
        .get("contract")
        .cloned()
        .ok_or_else(|| invalid_error("publish report has no contract"))?;
    let contract: ModelPublishContract =
        serde_json::from_value(contract_value).map_err(|source| PipelineError::Json {
            path: format!("{}#contract", publish_path.display()),
            source,
        })?;
    if contract.family != mapping.family
        || contract.legacy_name != mapping.logical_name
        || contract.semantic_directories != mapping.semantic_directories
        || contract.output_glb != mapping.output_glb
    {
        return invalid(format!(
            "publish contract differs from batch mapping for {:?}",
            candidate.id
        ));
    }
    contract.validate(&glb).map_err(|error| {
        invalid_error(format!(
            "publish contract failed for {:?}: {error}",
            candidate.id
        ))
    })?;

    let (evidence_json_relative, evidence_png_relative) =
        gpu_evidence_relative_paths(&candidate_glb)
            .map_err(|error| invalid_error(format!("cannot derive GPU evidence paths: {error}")))?;
    let evidence_json_path = evidence_root.join(&evidence_json_relative);
    let evidence_png_path = evidence_root.join(&evidence_png_relative);
    let evidence_json_bytes = read_regular(&evidence_json_path)?;
    let evidence_png_bytes = read_regular(&evidence_png_path)?;
    let evidence: LogicalModelGpuEvidence =
        serde_json::from_slice(&evidence_json_bytes).map_err(|source| PipelineError::Json {
            path: evidence_json_path.display().to_string(),
            source,
        })?;
    verify_gpu_evidence(
        &evidence,
        &candidate_glb,
        &glb,
        &facts,
        &slash_path(&evidence_png_relative),
        &evidence_png_bytes,
    )?;

    let mut animations = facts.standard_animation_names.clone();
    animations.sort();
    animations.dedup();
    Ok(TutorialCharacterPromotionProof {
        candidate_glb,
        glb_blake3: contract.glb_blake3,
        source_document: mapping.source.clone(),
        source_sha256: mapping.source_sha256.clone(),
        publish_report: publish_relative,
        publish_report_sha256: sha256(&publish_bytes),
        gpu_evidence_json: slash_path(&evidence_json_relative),
        gpu_evidence_json_sha256: sha256(&evidence_json_bytes),
        gpu_screenshot_png: slash_path(&evidence_png_relative),
        gpu_screenshot_sha256: sha256(&evidence_png_bytes),
        standard_animations: animations,
        files: installed_tree.len() as u64,
        bytes: installed_tree.values().map(|file| file.bytes).sum(),
    })
}

pub(super) fn verify_gpu_evidence(
    evidence: &LogicalModelGpuEvidence,
    candidate_glb: &str,
    glb: &[u8],
    facts: &LogicalModelGpuFacts,
    screenshot_relative: &str,
    screenshot: &[u8],
) -> Result<()> {
    if evidence.schema != GPU_EVIDENCE_SCHEMA
        || evidence.status != AutomatedGpuStatus::Passed
        || evidence.render_profile != GPU_RENDER_PROFILE
        || evidence.visual_parity != VisualParityClaim::NotAsserted
        || evidence.model.relative_glb != candidate_glb
        || evidence.model.true_name != facts.true_name
        || evidence.model.glb_byte_length != glb.len() as u64
        || evidence.model.glb_sha256 != sha256(glb)
    {
        return invalid(format!("GPU model identity failed for {candidate_glb:?}"));
    }
    let runtime = &evidence.runtime;
    if !runtime.scene_ready
        || runtime.outline_mode != SourceOutlineMode::Source
        || runtime.material_errors != 0
        || runtime.shader_errors != 0
        || runtime.mesh_parts != facts.mesh_parts
        || runtime.skinned_mesh_parts != facts.skinned_mesh_parts
        || runtime.skin_joint_references != facts.skin_joint_references
        || runtime.resolved_skin_joint_references != facts.skin_joint_references
        || runtime.inverse_bind_matrices != facts.inverse_bind_matrices
        || runtime.materials_applied != facts.materials_applied
        || runtime.legacy_pass_companions != facts.legacy_pass_companions
        || runtime.outline_pass_companions != facts.outline_pass_companions
        || runtime.assigned_texture_bindings != facts.assigned_texture_bindings
        || runtime.exact_mip_markers != facts.exact_mip_markers
        || runtime.exact_mip_chains != facts.exact_mip_chains
        || runtime.exact_mip_levels != facts.exact_mip_levels
    {
        return invalid(format!("GPU runtime facts failed for {candidate_glb:?}"));
    }
    if evidence.animation.standard_clips_loaded != facts.standard_animation_names.len() as u64 {
        return invalid(format!("GPU animation count failed for {candidate_glb:?}"));
    }
    if facts.standard_animation_names.is_empty() {
        if evidence.animation.selected_exact_name.is_some()
            || evidence.animation.animation_players != 0
            || evidence.animation.sampled_players != 0
        {
            return invalid(format!(
                "GPU invented animation state for {candidate_glb:?}"
            ));
        }
    } else if evidence
        .animation
        .selected_exact_name
        .as_ref()
        .is_none_or(|name| !facts.standard_animation_names.contains(name))
        || evidence.animation.sample_normalized_ppm != Some(500_000)
        || evidence.animation.animation_players == 0
        || evidence.animation.sampled_players == 0
    {
        return invalid(format!(
            "GPU animation sampling failed for {candidate_glb:?}"
        ));
    }
    if evidence.screenshot.relative_png != screenshot_relative
        || evidence.screenshot.byte_length != screenshot.len() as u64
        || evidence.screenshot.sha256 != sha256(screenshot)
        || evidence.screenshot.width == 0
        || evidence.screenshot.height == 0
        || evidence.screenshot.foreground_pixels == 0
    {
        return invalid(format!(
            "GPU screenshot identity failed for {candidate_glb:?}"
        ));
    }
    let image = image::load_from_memory_with_format(screenshot, image::ImageFormat::Png)
        .map_err(|error| invalid_error(format!("GPU screenshot is invalid PNG: {error}")))?;
    if image.width() != evidence.screenshot.width || image.height() != evidence.screenshot.height {
        return invalid(format!(
            "GPU screenshot dimensions failed for {candidate_glb:?}"
        ));
    }
    Ok(())
}

pub(super) fn build_next_metadata(
    manifest: &ProjectAssetManifest,
    registry: &SemanticCharacterRegistry,
    promotions: &[PreparedPromotion],
) -> Result<(ProjectAssetManifest, SemanticCharacterRegistry)> {
    let removed_paths = promotions
        .iter()
        .flat_map(|promotion| manifest_paths_below(manifest, &promotion.source_root))
        .collect::<BTreeSet<_>>();
    let mut next_manifest = manifest.clone();
    next_manifest
        .files
        .retain(|entry| !removed_paths.contains(&entry.path));
    let mut folds = next_manifest
        .files
        .iter()
        .map(|entry| entry.path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    for promotion in promotions {
        for entry in &promotion.target_entries {
            if !folds.insert(entry.path.to_ascii_lowercase()) {
                return invalid(format!("promotion manifest collision at {:?}", entry.path));
            }
            next_manifest.files.push(entry.clone());
        }
    }
    next_manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));

    let mut next_registry = registry.clone();
    next_registry.models.extend(
        promotions
            .iter()
            .map(|promotion| promotion.registry_model.clone()),
    );
    next_registry
        .models
        .sort_by(|left, right| left.glb.cmp(&right.glb));
    validate_registry(&next_registry)?;
    Ok((next_manifest, next_registry))
}

pub(super) fn canonical_plain_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|source| io_at(path, source))?;
    let metadata = fs::symlink_metadata(path).map_err(|source| io_at(path, source))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} must be a regular directory"));
    }
    Ok(canonical)
}

pub(super) fn regular_directory_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(true),
        Ok(_) => invalid(format!("expected regular directory: {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_at(path, error)),
    }
}

pub(super) fn category_directory(category: RuntimeCharacterCategory) -> &'static str {
    match category {
        RuntimeCharacterCategory::Nano => "characters/nanos",
        RuntimeCharacterCategory::Npc => "characters/npcs",
        RuntimeCharacterCategory::Mob => "characters/mobs",
        RuntimeCharacterCategory::Fusion => "characters/fusions",
        RuntimeCharacterCategory::Shared => "characters/shared",
    }
}

pub(super) fn category_label(category: RuntimeCharacterCategory) -> &'static str {
    match category {
        RuntimeCharacterCategory::Nano => "nano",
        RuntimeCharacterCategory::Npc => "npc",
        RuntimeCharacterCategory::Mob => "mob",
        RuntimeCharacterCategory::Fusion => "fusion",
        RuntimeCharacterCategory::Shared => "shared",
    }
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn pretty_json<T: Serialize>(value: &T, path: &str) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| PipelineError::Json {
        path: path.to_owned(),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn unique_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
