use super::*;

pub fn promote_tutorial_props(
    options: &TutorialPropPromotionOptions,
) -> Result<TutorialPropPromotionReport> {
    let plan = build_plan(options)?;
    if options.apply {
        if plan.report.counts.blocked != 0 {
            return invalid(format!(
                "refusing --apply because {} exact prop candidates are blocked",
                plan.report.counts.blocked
            ));
        }
        if !plan.promotions.is_empty() {
            apply_plan(&plan)?;
        }
    }
    let mut report = plan.report;
    report.mode = if options.apply {
        TutorialPropPromotionMode::Apply
    } else {
        TutorialPropPromotionMode::DryRun
    };
    report.applied = options.apply && report.counts.ready != 0;
    Ok(report)
}

pub(super) fn build_plan(options: &TutorialPropPromotionOptions) -> Result<PromotionPlan> {
    let project_root = canonical_plain_directory(&options.project_root, "project root")?;
    let asset_root = canonical_plain_directory(
        &project_root.join(native_path(ASSET_ROOT_RELATIVE)),
        "asset root",
    )?;
    let candidate_root = canonical_plain_directory(&options.candidate_root, "candidate root")?;
    let source_root = canonical_plain_directory(&options.source_root, "source root")?;
    let evidence_root = canonical_plain_directory(&options.gpu_evidence_root, "GPU evidence root")?;
    reject_overlaps(&asset_root, &candidate_root, &source_root, &evidence_root)?;

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_before = read_regular(&manifest_path)?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_before).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid("project asset manifest has the wrong schema");
    }
    let manifest_index = index_manifest(&manifest)?;

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
    let mut counts = TutorialPropPromotionCounts {
        candidates: CANDIDATES.len() as u64,
        manifest_files_before: manifest.files.len() as u64,
        ..Default::default()
    };
    for candidate in CANDIDATES {
        match prepare_promotion(
            candidate,
            &project_root,
            &asset_root,
            &candidate_root,
            &source_root,
            &evidence_root,
            &manifest,
            &manifest_index,
            &mappings,
        ) {
            Ok(PreparedState::Promote(prepared, proof)) => {
                counts.ready += 1;
                counts.runtime_references_updated += 1;
                counts.promoted_files += prepared.target_entries.len() as u64;
                counts.promoted_bytes += prepared
                    .target_entries
                    .iter()
                    .map(|entry| entry.bytes)
                    .sum::<u64>();
                models.push(report_model(
                    candidate,
                    TutorialPropPromotionStatus::Promote,
                    Some(proof),
                    Vec::new(),
                ));
                promotions.push(prepared);
            }
            Ok(PreparedState::AlreadyPromoted(proof)) => {
                counts.already_promoted += 1;
                models.push(report_model(
                    candidate,
                    TutorialPropPromotionStatus::AlreadyPromoted,
                    Some(proof),
                    Vec::new(),
                ));
            }
            Err(error) => {
                counts.blocked += 1;
                models.push(report_model(
                    candidate,
                    TutorialPropPromotionStatus::Blocked,
                    None,
                    vec![TutorialPropPromotionBlocker {
                        code: "proof_failed".to_owned(),
                        detail: error.to_string(),
                    }],
                ));
            }
        }
    }

    let (next_manifest, preserved) = build_next_manifest(&manifest, &promotions)?;
    counts.external_manifest_entries_preserved = preserved;
    counts.manifest_files_after = next_manifest.files.len() as u64;
    let manifest_after = if promotions.is_empty() {
        manifest_before.clone()
    } else {
        pretty_json(&next_manifest, ASSET_MANIFEST_FILE)?
    };
    let report = TutorialPropPromotionReport {
        schema: TUTORIAL_PROP_PROMOTION_SCHEMA.to_owned(),
        mode: TutorialPropPromotionMode::DryRun,
        applied: false,
        requires_asset_index_regeneration: counts.ready != 0,
        exact_allowlist: CANDIDATES
            .iter()
            .map(|candidate| candidate.id.to_owned())
            .collect(),
        external_texture_duplicates_untouched: true,
        counts,
        models,
    };
    Ok(PromotionPlan {
        project_root,
        asset_root,
        manifest_path,
        manifest_before,
        manifest_after,
        promotions,
        report,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_promotion(
    candidate: &PromotionCandidate,
    project_root: &Path,
    asset_root: &Path,
    candidate_root: &Path,
    source_documents_root: &Path,
    evidence_root: &Path,
    manifest: &ProjectAssetManifest,
    manifest_index: &BTreeMap<String, ProjectAssetFile>,
    mappings: &BTreeMap<String, LogicalModelBatchMapping>,
) -> Result<PreparedState> {
    let source_root = candidate.source_root();
    let target_root = candidate.target_root();
    let source_path = asset_root.join(native_path(&source_root));
    let target_path = asset_root.join(native_path(&target_root));
    let source_exists = regular_directory_exists(&source_path)?;
    let target_exists = regular_directory_exists(&target_path)?;

    match (source_exists, target_exists) {
        (true, false) => {
            if !manifest_paths_below(manifest, &target_root).is_empty() {
                return invalid(format!(
                    "target {target_root:?} is absent on disk but present in the manifest"
                ));
            }
            let (runtime_source_path, runtime_before, runtime_after, runtime_proof) =
                prepare_runtime_reference(project_root, candidate, false)?;
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
                runtime_proof,
            )?;
            let source_tree = collect_tree(&source_path, false)?;
            let mut target_entries = Vec::with_capacity(source_tree.len());
            for (relative, identity) in &source_tree {
                let source_manifest_path = format!("{source_root}/{relative}");
                let source_entry = manifest_index.get(&source_manifest_path).ok_or_else(|| {
                    invalid_error(format!("missing manifest entry {source_manifest_path:?}"))
                })?;
                target_entries.push(ProjectAssetFile {
                    source_path: format!(
                        "native-tutorial-prop-promotion/models/mob/{}/{}",
                        candidate.id, relative
                    ),
                    path: format!("{target_root}/{relative}"),
                    kind: source_entry.kind,
                    bytes: identity.bytes,
                    blake3: identity.blake3.clone(),
                });
            }
            target_entries.sort_by(|left, right| left.path.cmp(&right.path));
            Ok(PreparedState::Promote(
                PreparedPromotion {
                    candidate: *candidate,
                    source_root,
                    target_root,
                    source_tree,
                    target_entries,
                    runtime_source_path,
                    runtime_before,
                    runtime_after,
                },
                proof,
            ))
        }
        (false, true) => {
            if !manifest_paths_below(manifest, &source_root).is_empty() {
                return invalid(format!(
                    "tutorial source {source_root:?} is absent on disk but remains in the manifest"
                ));
            }
            let (_, _, _, runtime_proof) =
                prepare_runtime_reference(project_root, candidate, true)?;
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
                runtime_proof,
            )?;
            Ok(PreparedState::AlreadyPromoted(proof))
        }
        (true, true) => invalid(format!(
            "both tutorial and prop packages exist for {:?}; refusing to guess ownership",
            candidate.id
        )),
        (false, false) => invalid(format!(
            "neither tutorial nor prop package exists for {:?}",
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
    runtime_reference: TutorialPropRuntimeReferenceProof,
) -> Result<TutorialPropPromotionProof> {
    let candidate_glb = candidate.candidate_glb();
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
            "installed package is not byte-identical to candidate closure for {:?}",
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
    if facts.true_name != mapping.logical_name
        || facts.mesh_parts == 0
        || facts.skinned_mesh_parts != 0
        || facts.skin_joint_references != 0
        || facts.inverse_bind_matrices != 0
        || !facts.standard_animation_names.is_empty()
    {
        return invalid(format!(
            "exact candidate {:?} is not a rigid, non-animated static prop",
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
    let contract: ModelPublishContract = serde_json::from_value(
        publish
            .get("contract")
            .cloned()
            .ok_or_else(|| invalid_error("publish report has no contract"))?,
    )
    .map_err(|source| PipelineError::Json {
        path: format!("{}#contract", publish_path.display()),
        source,
    })?;
    if contract.family != mapping.family
        || contract.legacy_name != mapping.logical_name
        || contract.root_node != mapping.logical_name
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
    let evidence_json_bytes = read_regular(&evidence_root.join(&evidence_json_relative))?;
    let evidence_png_bytes = read_regular(&evidence_root.join(&evidence_png_relative))?;
    let evidence: LogicalModelGpuEvidence =
        serde_json::from_slice(&evidence_json_bytes).map_err(|source| PipelineError::Json {
            path: evidence_root
                .join(&evidence_json_relative)
                .display()
                .to_string(),
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

    Ok(TutorialPropPromotionProof {
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
        runtime_reference,
        mesh_parts: facts.mesh_parts,
        materials_applied: facts.materials_applied,
        external_texture_bindings: facts.assigned_texture_bindings,
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
        || runtime.skinned_mesh_parts != 0
        || runtime.skin_joint_references != 0
        || runtime.resolved_skin_joint_references != 0
        || runtime.inverse_bind_matrices != 0
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
    if evidence.animation.standard_clips_loaded != 0
        || evidence.animation.selected_exact_name.is_some()
        || evidence.animation.animation_players != 0
        || evidence.animation.sampled_players != 0
    {
        return invalid(format!(
            "GPU evidence invented animation state for static prop {candidate_glb:?}"
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

pub(super) fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn unique_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

pub(super) fn pretty_json<T: Serialize>(value: &T, path: &str) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| PipelineError::Json {
        path: path.to_owned(),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
