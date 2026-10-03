use super::*;

pub fn run_equipment_gpu_batch(
    options: &EquipmentGpuBatchOptions,
) -> Result<EquipmentGpuBatchReport> {
    let total_started = Instant::now();
    validate_options(options)?;
    let preflight_started = Instant::now();
    let candidate_root = canonical_directory(&options.candidate_root, "candidate root")?;
    let preview_executable = canonical_file(&options.preview_executable, "preview executable")?;
    let evidence_root = create_or_open_evidence_root(&options.evidence_root)?;
    let report_path = absolute_new_report_path(&options.report_path)?;
    reject_overlapping_paths(&candidate_root, &evidence_root, &report_path)?;

    let candidate_batch_path = candidate_root.join(EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE);
    let candidate_batch_bytes =
        fs::read(&candidate_batch_path).map_err(|error| io_at(&candidate_batch_path, error))?;
    let candidate_batch: EquipmentLogicalModelBatchReport =
        serde_json::from_slice(&candidate_batch_bytes).map_err(|source| PipelineError::Json {
            path: candidate_batch_path.display().to_string(),
            source,
        })?;
    if candidate_batch.schema != EQUIPMENT_LOGICAL_MODEL_BATCH_SCHEMA
        || !candidate_batch.structural_audit_passed
        || candidate_batch.production_assets_mutated
    {
        return gpu_batch_error("candidate is not a clean native equipment publication");
    }
    let structural = audit_logical_model_tree(&candidate_root)?;
    if !structural.passed
        || structural.counts.glbs != candidate_batch.counts.published_models
        || structural.models.len() != candidate_batch.models.len()
    {
        return gpu_batch_error("candidate structural audit or model cardinality failed");
    }

    let mut candidates = Vec::with_capacity(candidate_batch.models.len());
    for mapping in &candidate_batch.models {
        if !REQUIRED_SLOTS.contains(&mapping.category.as_str()) {
            return gpu_batch_error(format!(
                "candidate has unsupported equipment category {:?}",
                mapping.category
            ));
        }
        if !is_exact_equipment_output_path(
            &mapping.category,
            &mapping.exact_route,
            &mapping.true_name,
            &mapping.output_glb,
        ) {
            return gpu_batch_error(format!(
                "candidate path is not exact equipment taxonomy: {:?}",
                mapping.output_glb
            ));
        }
        let glb_path = join_relative(&candidate_root, &mapping.output_glb)?;
        let glb = fs::read(&glb_path).map_err(|error| io_at(&glb_path, error))?;
        let facts = gpu_model_facts_from_glb(&glb)
            .map_err(|error| gpu_batch_error_value(error.to_string()))?;
        if facts.true_name != mapping.true_name {
            return gpu_batch_error(format!(
                "GPU facts true name differs from equipment mapping for {:?}",
                mapping.output_glb
            ));
        }
        candidates.push(Candidate {
            category: mapping.category.clone(),
            true_name: mapping.true_name.clone(),
            relative_glb: mapping.output_glb.clone(),
            variants: variant_labels(&facts),
            facts,
        });
    }
    candidates.sort_by(|left, right| left.relative_glb.cmp(&right.relative_glb));
    let (selections, required_variants) =
        select_candidates(options.mode, options.shard, &candidates);
    let mut blockers = scope_blockers(&candidates);
    let preflight_milliseconds = elapsed_milliseconds(preflight_started)?;

    let initial_audit = audit_logical_model_gpu_evidence(&candidate_root, &evidence_root)?;
    let initial_models = initial_audit
        .models
        .iter()
        .map(|model| (model.relative_glb.as_str(), model))
        .collect::<BTreeMap<_, _>>();
    let mut models = Vec::with_capacity(selections.len());
    let execution_started = Instant::now();
    for selection in selections {
        let candidate = &candidates[selection.index];
        let (relative_json_path, relative_png_path) =
            gpu_evidence_relative_paths(&candidate.relative_glb)
                .map_err(|error| gpu_batch_error_value(error.to_string()))?;
        let relative_json = slash_path(&relative_json_path);
        let relative_png = slash_path(&relative_png_path);
        let json_exists = evidence_root.join(&relative_json_path).is_file();
        let png_exists = evidence_root.join(&relative_png_path).is_file();
        let started = Instant::now();
        let initial_passed = initial_models
            .get(candidate.relative_glb.as_str())
            .is_some_and(|model| model.passed);

        let (disposition, process_exit_code) = if json_exists && png_exists && initial_passed {
            ("resumed-valid-evidence".to_owned(), None)
        } else if json_exists || png_exists {
            blockers.push(model_blocker(
                "resume-preflight",
                "incompleteOrInvalidExistingEvidence",
                candidate,
                "immutable evidence paths already exist but do not form a valid exact pair",
                json!({
                    "evidenceJsonExists": json_exists,
                    "screenshotPngExists": png_exists,
                    "initialIndependentAuditPassed": initial_passed,
                    "fallbackApplied": false,
                }),
                "blocked-no-overwrite",
            ));
            ("blocked-existing-evidence".to_owned(), None)
        } else {
            let mut command = Command::new(&preview_executable);
            command
                .arg("--asset-root")
                .arg(&candidate_root)
                .arg("--model")
                .arg(&candidate.relative_glb)
                .arg("--evidence-root")
                .arg(&evidence_root)
                .arg("--outline")
                .arg("source")
                .arg("--frames")
                .arg(options.max_frames.to_string())
                .arg("--timeout")
                .arg(options.timeout_seconds.to_string());
            let selected_animation = candidate.facts.standard_animation_names.first();
            if let Some(animation) = selected_animation {
                command.arg("--animation-name").arg(animation);
            }
            let output = command.output().map_err(|error| {
                gpu_batch_error_value(format!(
                    "could not execute GPU preview {}: {error}",
                    preview_executable.display()
                ))
            })?;
            let exit_code = output.status.code();
            if output.status.success()
                && evidence_root.join(&relative_json_path).is_file()
                && evidence_root.join(&relative_png_path).is_file()
            {
                ("passed-new-evidence".to_owned(), exit_code)
            } else {
                let classified = classify_preview_failure(&output.stdout);
                blockers.push(model_blocker(
                    "preview-execution",
                    &classified.code,
                    candidate,
                    &classified.detail,
                    json!({
                        "exitCode": exit_code,
                        "runtimeError": classified.runtime_error,
                        "typed": classified.typed_evidence,
                        "stdoutTail": output_tail(&output.stdout),
                        "stderrTail": output_tail(&output.stderr),
                        "evidenceJsonExists": evidence_root.join(&relative_json_path).is_file(),
                        "screenshotPngExists": evidence_root.join(&relative_png_path).is_file(),
                        "fallbackApplied": false,
                    }),
                    "blocked-no-fake-or-placeholder-evidence",
                ));
                ("blocked-preview-failed".to_owned(), exit_code)
            }
        };

        models.push(EquipmentGpuModelRun {
            category: candidate.category.clone(),
            true_name: candidate.true_name.clone(),
            relative_glb: candidate.relative_glb.clone(),
            selected_exact_animation_name: candidate
                .facts
                .standard_animation_names
                .first()
                .cloned(),
            selection_reasons: selection.reasons,
            facts: facts_summary(&candidate.facts),
            disposition,
            process_exit_code,
            elapsed_milliseconds: elapsed_milliseconds(started)?,
            evidence_json: relative_json,
            screenshot_png: relative_png,
        });
    }
    let preview_execution_milliseconds = elapsed_milliseconds(execution_started)?;

    let audit_started = Instant::now();
    let final_audit = audit_logical_model_gpu_evidence(&candidate_root, &evidence_root)?;
    let final_models = final_audit
        .models
        .iter()
        .map(|model| (model.relative_glb.as_str(), model))
        .collect::<BTreeMap<_, _>>();
    for model in &mut models {
        let passed = final_models
            .get(model.relative_glb.as_str())
            .is_some_and(|audit| audit.passed);
        if passed {
            if model.disposition == "blocked-preview-failed"
                || model.disposition == "blocked-existing-evidence"
            {
                return gpu_batch_error("blocked model unexpectedly passed final evidence audit");
            }
        } else if !model.disposition.starts_with("blocked-") {
            let candidate = candidates
                .iter()
                .find(|candidate| candidate.relative_glb == model.relative_glb)
                .ok_or_else(|| gpu_batch_error_value("selected candidate disappeared"))?;
            blockers.push(model_blocker(
                "independent-evidence-audit",
                "standaloneGpuEvidenceAuditFailed",
                candidate,
                "published evidence pair failed independent at-rest validation",
                json!({
                    "violations": final_models
                        .get(model.relative_glb.as_str())
                        .map(|audit| &audit.violations),
                    "fallbackApplied": false,
                }),
                "blocked-requires-new-evidence-root-after-fix",
            ));
            model.disposition = "blocked-independent-audit".to_owned();
        }
    }
    let independent_audit_milliseconds = elapsed_milliseconds(audit_started)?;

    let live_candidate_batch =
        fs::read(&candidate_batch_path).map_err(|error| io_at(&candidate_batch_path, error))?;
    if live_candidate_batch != candidate_batch_bytes {
        return gpu_batch_error("candidate batch report changed while GPU evidence was running");
    }

    blockers.sort_by(|left, right| {
        (&left.stage, &left.category, &left.relative_glb, &left.code).cmp(&(
            &right.stage,
            &right.category,
            &right.relative_glb,
            &right.code,
        ))
    });
    models.sort_by(|left, right| left.relative_glb.cmp(&right.relative_glb));
    let selected_passed = models
        .iter()
        .filter(|model| {
            final_models
                .get(model.relative_glb.as_str())
                .is_some_and(|audit| audit.passed)
        })
        .count();
    let selected_evidence_passed = selected_passed == models.len()
        && final_audit.counts.orphan_jsons == 0
        && final_audit.counts.orphan_pngs == 0;
    let full_evidence_passed =
        options.mode == EquipmentGpuBatchMode::Full && final_audit.automated_gpu_passed;
    let execution_blockers = blockers
        .iter()
        .filter(|blocker| blocker.stage != "remaining-runtime-scope")
        .count();
    let scope_blocker_count = blockers
        .iter()
        .filter(|blocker| blocker.stage == "remaining-runtime-scope")
        .count();
    let covered_variants = models
        .iter()
        .filter_map(|model| {
            candidates
                .iter()
                .find(|candidate| candidate.relative_glb == model.relative_glb)
        })
        .flat_map(|candidate| candidate.variants.iter().cloned())
        .collect::<BTreeSet<_>>();
    let counts = EquipmentGpuBatchCounts {
        candidate_models: u64_count(candidates.len(), "candidate model count")?,
        selected_models: u64_count(models.len(), "selected model count")?,
        executed_models: u64_count(
            models
                .iter()
                .filter(|model| model.process_exit_code.is_some())
                .count(),
            "executed model count",
        )?,
        resumed_valid_models: u64_count(
            models
                .iter()
                .filter(|model| model.disposition == "resumed-valid-evidence")
                .count(),
            "resumed model count",
        )?,
        standalone_gpu_passed_models: u64_count(selected_passed, "passed model count")?,
        execution_blocked_models: u64_count(
            models
                .iter()
                .filter(|model| model.disposition.starts_with("blocked-"))
                .count(),
            "blocked model count",
        )?,
        rigid_selected: u64_count(
            models
                .iter()
                .filter(|model| model.facts.skinned_mesh_parts == 0)
                .count(),
            "rigid selection count",
        )?,
        skinned_selected: u64_count(
            models
                .iter()
                .filter(|model| model.facts.skinned_mesh_parts > 0)
                .count(),
            "skinned selection count",
        )?,
        animated_selected: u64_count(
            models
                .iter()
                .filter(|model| !model.facts.standard_animation_names.is_empty())
                .count(),
            "animated selection count",
        )?,
        static_selected: u64_count(
            models
                .iter()
                .filter(|model| model.facts.standard_animation_names.is_empty())
                .count(),
            "static selection count",
        )?,
        slots_required: u64_count(REQUIRED_SLOTS.len(), "required slot count")?,
        slots_with_candidates: u64_count(
            candidates
                .iter()
                .map(|candidate| candidate.category.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            "slots with candidates",
        )?,
        slots_selected: u64_count(
            models
                .iter()
                .map(|model| model.category.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            "selected slot count",
        )?,
        scope_blockers: u64_count(scope_blocker_count, "scope blocker count")?,
        execution_blockers: u64_count(execution_blockers, "execution blocker count")?,
        total_blockers: u64_count(blockers.len(), "total blocker count")?,
    };
    let slots = REQUIRED_SLOTS
        .iter()
        .map(|slot| EquipmentGpuSlotCoverage {
            slot: (*slot).to_owned(),
            candidate_models: candidates
                .iter()
                .filter(|candidate| candidate.category == *slot)
                .count() as u64,
            selected_models: models
                .iter()
                .filter(|model| model.category == *slot)
                .count() as u64,
            passed_models: models
                .iter()
                .filter(|model| {
                    model.category == *slot
                        && final_models
                            .get(model.relative_glb.as_str())
                            .is_some_and(|audit| audit.passed)
                })
                .count() as u64,
        })
        .collect();
    let report = EquipmentGpuBatchReport {
        schema: EQUIPMENT_GPU_BATCH_SCHEMA.to_owned(),
        status: status(
            options.mode,
            options.shard,
            selected_evidence_passed,
            full_evidence_passed,
            execution_blockers,
        )
        .to_owned(),
        mode: options.mode,
        shard: options.shard,
        scope: EQUIPMENT_GPU_SCOPE.to_owned(),
        standalone_gate_applicable_to_rigid: true,
        standalone_gate_applicable_to_self_contained_skinned: true,
        player_attachment_parity_asserted: false,
        visual_parity_pending: true,
        publishable: false,
        production_assets_mutated: false,
        candidate_root: slash_path(&candidate_root),
        evidence_root: slash_path(&evidence_root),
        candidate_batch_report: bytes_evidence(&candidate_batch_path, &candidate_batch_bytes)?,
        preview_executable: file_evidence(&preview_executable)?,
        deterministic_selection_policy: match (options.mode, options.shard) {
            (EquipmentGpuBatchMode::Smoke, None) => {
                "deterministic greedy cover of every populated exact slot and every observed rigid/skinned/animation/material/mip variant; ties by relative GLB".to_owned()
            }
            (EquipmentGpuBatchMode::Full, Some(shard)) => format!(
                "candidate GLBs ordered by exact relative path; select zero-based ordinal modulo {} equal to {}",
                shard.count, shard.index
            ),
            (EquipmentGpuBatchMode::Full, None) => {
                "all candidate GLBs ordered by exact relative path".to_owned()
            }
            (EquipmentGpuBatchMode::Smoke, Some(_)) => {
                unreachable!("smoke sharding is rejected during option validation")
            }
        },
        animation_selection_policy:
            "first exact standard animation m_Name in deterministic published GLB order; omitted for static GLBs"
                .to_owned(),
        timing: EquipmentGpuBatchTiming {
            preflight_milliseconds,
            preview_execution_milliseconds,
            independent_audit_milliseconds,
            total_milliseconds: elapsed_milliseconds(total_started)?,
        },
        counts,
        coverage: EquipmentGpuCoverage {
            required_slots: REQUIRED_SLOTS.iter().map(|slot| (*slot).to_owned()).collect(),
            slots,
            required_variant_labels: required_variants.into_iter().collect(),
            covered_variant_labels: covered_variants.into_iter().collect(),
            independent_structural_passed: final_audit.structural_passed,
            independent_selected_evidence_passed: selected_evidence_passed,
            independent_full_evidence_passed: full_evidence_passed,
            independent_audit_matched_models: final_audit.counts.matched_models,
            independent_audit_candidate_models: final_audit.counts.candidate_glbs,
            independent_audit_violations: u64_count(
                final_audit.violations.len(),
                "independent audit violation count",
            )?,
        },
        models,
        blockers,
    };
    write_report_new(&report_path, &report)?;
    Ok(report)
}

pub(super) fn select_candidates(
    mode: EquipmentGpuBatchMode,
    shard: Option<EquipmentGpuShard>,
    candidates: &[Candidate],
) -> (Vec<Selection>, BTreeSet<String>) {
    let required_variants = candidates
        .iter()
        .flat_map(|candidate| candidate.variants.iter().cloned())
        .collect::<BTreeSet<_>>();
    if mode == EquipmentGpuBatchMode::Full {
        return (
            (0..candidates.len())
                .filter(|index| {
                    shard.is_none_or(|shard| {
                        u32::try_from(*index).is_ok_and(|index| index % shard.count == shard.index)
                    })
                })
                .map(|index| Selection {
                    index,
                    reasons: vec![shard.map_or_else(
                        || "full-candidate-set".to_owned(),
                        |shard| format!("full-candidate-set-shard-{}/{}", shard.index, shard.count),
                    )],
                })
                .collect(),
            required_variants,
        );
    }

    let mut required = required_variants.clone();
    for category in candidates
        .iter()
        .map(|candidate| candidate.category.as_str())
        .collect::<BTreeSet<_>>()
    {
        required.insert(format!("slot:{category}"));
    }
    let mut uncovered = required;
    let mut remaining = (0..candidates.len()).collect::<BTreeSet<_>>();
    let mut selected = Vec::new();
    while !uncovered.is_empty() {
        let best = remaining
            .iter()
            .map(|index| {
                let labels = candidate_labels(&candidates[*index]);
                let score = labels.intersection(&uncovered).count();
                (*index, score)
            })
            .filter(|(_, score)| *score > 0)
            .max_by(|(left_index, left_score), (right_index, right_score)| {
                left_score.cmp(right_score).then_with(|| {
                    candidates[*right_index]
                        .relative_glb
                        .cmp(&candidates[*left_index].relative_glb)
                })
            });
        let Some((index, _)) = best else {
            break;
        };
        remaining.remove(&index);
        let labels = candidate_labels(&candidates[index]);
        let reasons = labels.intersection(&uncovered).cloned().collect::<Vec<_>>();
        for reason in &reasons {
            uncovered.remove(reason);
        }
        selected.push(Selection { index, reasons });
    }
    selected.sort_by_key(|selection| candidates[selection.index].relative_glb.clone());
    (selected, required_variants)
}

pub(super) fn candidate_labels(candidate: &Candidate) -> BTreeSet<String> {
    let mut labels = candidate.variants.clone();
    labels.insert(format!("slot:{}", candidate.category));
    labels
}

pub(super) fn variant_labels(facts: &LogicalModelGpuFacts) -> BTreeSet<String> {
    let mut labels = BTreeSet::new();
    labels.insert(
        if facts.skinned_mesh_parts == 0 {
            "rig:rigid"
        } else {
            "rig:skinned"
        }
        .to_owned(),
    );
    labels.insert(
        if facts.standard_animation_names.is_empty() {
            "animation:static"
        } else {
            "animation:animated"
        }
        .to_owned(),
    );
    labels.insert(
        if facts.assigned_texture_bindings == 0 {
            "material:untextured"
        } else {
            "material:textured"
        }
        .to_owned(),
    );
    labels.insert(
        if facts.legacy_pass_companions == 0 {
            "material:single-pass"
        } else {
            "material:multi-pass"
        }
        .to_owned(),
    );
    labels.insert(
        if facts.outline_pass_companions == 0 {
            "material:no-outline"
        } else {
            "material:outline"
        }
        .to_owned(),
    );
    labels.insert(
        if facts.exact_mip_chains == 0 {
            "mip:base-only"
        } else {
            "mip:exact-chain"
        }
        .to_owned(),
    );
    labels
}

pub(super) fn facts_summary(facts: &LogicalModelGpuFacts) -> EquipmentGpuFactsSummary {
    EquipmentGpuFactsSummary {
        mesh_parts: facts.mesh_parts,
        skinned_mesh_parts: facts.skinned_mesh_parts,
        skin_joint_references: facts.skin_joint_references,
        inverse_bind_matrices: facts.inverse_bind_matrices,
        standard_animation_names: facts.standard_animation_names.clone(),
        materials_applied: facts.materials_applied,
        legacy_pass_companions: facts.legacy_pass_companions,
        outline_pass_companions: facts.outline_pass_companions,
        assigned_texture_bindings: facts.assigned_texture_bindings,
        exact_mip_chains: facts.exact_mip_chains,
        exact_mip_levels: facts.exact_mip_levels,
    }
}

pub(super) fn scope_blockers(candidates: &[Candidate]) -> Vec<EquipmentGpuBlocker> {
    let mut blockers = Vec::new();
    for slot in REQUIRED_SLOTS {
        let count = candidates
            .iter()
            .filter(|candidate| candidate.category == *slot)
            .count();
        if count == 0 {
            blockers.push(EquipmentGpuBlocker {
                stage: "remaining-runtime-scope".to_owned(),
                code: "noPublishedCandidateForEquipmentSlot".to_owned(),
                category: Some((*slot).to_owned()),
                true_name: None,
                relative_glb: None,
                detail: format!(
                    "the exact published candidate contains no standalone GLB for slot {slot:?}"
                ),
                evidence: json!({
                    "candidateModels": 0,
                    "fallbackApplied": false,
                }),
                disposition: "blocked-requires-authoritative-published-slot-candidate".to_owned(),
            });
            continue;
        }
        let (code, detail, required) = match *slot {
            "hat" | "glasses" | "back" | "weapon" => (
                "playerSocketAttachmentParityPending",
                "standalone evidence does not locate the exact player socket, reparent the item, apply ActorSkinCombiner local rotation, or verify socket scale",
                "player assembly GPU evidence using the exact socket path and standard attachment placement",
            ),
            "head" | "shirt" | "pants" | "shoes" => (
                "playerSkinnedBodyAssemblyParityPending",
                "standalone self-contained skin evidence does not prove assembly with the selected player skeleton, body-part combination, or hat hide policy",
                "selected-player body/equipment assembly GPU evidence with exact shared skeleton and hide policies",
            ),
            "mask" => (
                "playerSkinnedFaceAssemblyParityPending",
                "standalone face skin evidence does not prove ActorWearIndexTable bone remapping into the selected player skeleton or coexistence with the selected head and hat hide policy",
                "selected-player face/head assembly GPU evidence with exact shared skeleton remap and hat hide policy",
            ),
            "vehicle" => (
                "playerVehicleMountParityPending",
                "standalone vehicle rendering does not prove rider mounting, vehicle transform policy, or player visibility while mounted",
                "mounted-player runtime GPU evidence driven by authoritative vehicle policy",
            ),
            _ => (
                "playerEquipmentAssemblyParityPending",
                "standalone rendering does not prove final selected-player equipment assembly",
                "selected-player runtime assembly GPU evidence",
            ),
        };
        blockers.push(EquipmentGpuBlocker {
            stage: "remaining-runtime-scope".to_owned(),
            code: code.to_owned(),
            category: Some((*slot).to_owned()),
            true_name: None,
            relative_glb: None,
            detail: detail.to_owned(),
            evidence: json!({
                "candidateModels": count,
                "standaloneGateApplicable": true,
                "playerAttachmentParityAsserted": false,
                "requiredEvidence": required,
                "fallbackApplied": false,
            }),
            disposition: "pending-player-attachment-visual-parity-harness".to_owned(),
        });
    }
    blockers
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let path = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| io_at(&path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return gpu_batch_error(format!("{label} must be a real directory"));
    }
    Ok(path)
}

pub(super) fn canonical_file(path: &Path, label: &str) -> Result<PathBuf> {
    let path = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| io_at(&path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return gpu_batch_error(format!("{label} must be a real regular file"));
    }
    Ok(path)
}

pub(super) fn create_or_open_evidence_root(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|error| io_at(path, error))?;
    }
    canonical_directory(path, "evidence root")
}

pub(super) fn join_relative(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return gpu_batch_error("unsafe candidate-relative path");
    }
    Ok(root.join(path))
}

pub(super) fn file_evidence(path: &Path) -> Result<GpuBatchFileEvidence> {
    let bytes = fs::read(path).map_err(|error| io_at(path, error))?;
    bytes_evidence(path, &bytes)
}

pub(super) fn bytes_evidence(path: &Path, bytes: &[u8]) -> Result<GpuBatchFileEvidence> {
    Ok(GpuBatchFileEvidence {
        path: slash_path(path),
        byte_length: u64_count(bytes.len(), "file byte length")?,
        sha256: sha256_hex(bytes),
    })
}

pub(super) fn output_tail(bytes: &[u8]) -> String {
    const LIMIT: usize = 16 * 1024;
    let start = bytes.len().saturating_sub(LIMIT);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}
