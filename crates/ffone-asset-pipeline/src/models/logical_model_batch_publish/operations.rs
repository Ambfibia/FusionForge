use super::*;

pub(super) fn preflight_batch(source_root: &Path) -> Result<Vec<BatchPlan>> {
    let metadata = fs::symlink_metadata(source_root).map_err(|error| io_at(source_root, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return batch_error("batch source root must be a real directory, not a symlink");
    }
    let mut source_files = Vec::new();
    walk_source_tree(source_root, source_root, &mut source_files)?;
    source_files.sort_by(|left, right| left.0.cmp(&right.0));
    if source_files.is_empty() {
        return batch_error("batch source root contains no .source.json files");
    }

    let mut source_keys = BTreeMap::<String, String>::new();
    let mut outputs = PortableOutputRegistry::default();
    let mut plans = Vec::with_capacity(source_files.len());
    for (source_relative, source) in source_files {
        let source_key = portable_string_key(&source_relative);
        if let Some(previous) = source_keys.insert(source_key, source_relative.clone())
            && previous != source_relative
        {
            return batch_error(format!(
                "source paths collide after Unicode/case folding: {previous:?} and {source_relative:?}"
            ));
        }
        let bytes = fs::read(&source).map_err(|error| io_at(&source, error))?;
        let document: SourcePreflightDocument =
            serde_json::from_slice(&bytes).map_err(|source_error| PipelineError::Json {
                path: source.display().to_string(),
                source: source_error,
            })?;
        let (family, semantic_directories, source_filename) = source_layout(&source_relative)?;
        let plan = preflight_source(
            source,
            source_relative,
            sha256_hex(&bytes),
            family,
            semantic_directories,
            source_filename,
            document,
        )?;
        for output in &plan.output_files {
            outputs.register(output, &plan.source_relative)?;
        }
        plans.push(plan);
    }
    outputs.register(Path::new(LOGICAL_MODEL_BATCH_REPORT_FILE), "<batch-report>")?;
    Ok(plans)
}

pub(super) fn walk_source_tree(
    root: &Path,
    directory: &Path,
    output: &mut Vec<(String, PathBuf)>,
) -> Result<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| io_at(directory, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(directory, error))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| io_at(&path, error))?;
        if file_type.is_symlink() {
            return batch_error(format!(
                "symlinks are forbidden in batch source root: {path:?}"
            ));
        }
        if file_type.is_dir() {
            walk_source_tree(root, &path, output)?;
            continue;
        }
        if !file_type.is_file() {
            return batch_error(format!(
                "non-regular entry is forbidden in batch source root: {path:?}"
            ));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| batch_error_value("source escaped batch root"))?;
        let relative = relative_path_string(relative)?;
        if !relative.ends_with(".source.json") {
            return batch_error(format!(
                "unexpected file in batch source root; only .source.json is allowed: {relative:?}"
            ));
        }
        output.push((relative, path));
    }
    Ok(())
}

pub(super) fn source_layout(relative: &str) -> Result<(String, Vec<String>, String)> {
    let components = relative.split('/').collect::<Vec<_>>();
    if components.len() < 3 {
        return batch_error(format!(
            "source must be under <family>/<exact-route-stem>/...: {relative:?}"
        ));
    }
    for component in &components {
        validate_windows_component(component, "source path")?;
        if has_generated_identity(component) {
            return batch_error(format!(
                "generated/hash/PathID identity is forbidden in source path: {relative:?}"
            ));
        }
    }
    let family = components[0].to_owned();
    let semantic_directories = components[1..components.len() - 1]
        .iter()
        .map(|value| (*value).to_owned())
        .collect();
    Ok((
        family,
        semantic_directories,
        components.last().unwrap().to_string(),
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn preflight_source(
    source: PathBuf,
    source_relative: String,
    source_sha256: String,
    family: String,
    semantic_directories: Vec<String>,
    source_filename: String,
    document: SourcePreflightDocument,
) -> Result<BatchPlan> {
    if document.schema != LOGICAL_MODEL_SOURCE_SCHEMA {
        return batch_error(format!(
            "source {source_relative:?} schema must be {LOGICAL_MODEL_SOURCE_SCHEMA:?}"
        ));
    }
    let safe_glb = minimal_windows_glb_filename(&document.logical_name)
        .map_err(|error| batch_error_value(error.to_string()))?;
    let safe_stem = safe_glb
        .strip_suffix(".glb")
        .ok_or_else(|| batch_error_value("minimal logical-model filename has no .glb suffix"))?;
    let expected_source_filename = format!("{safe_stem}.source.json");
    if source_filename != expected_source_filename {
        return batch_error(format!(
            "source filename must be only the minimally Windows-safe true root m_Name: expected {expected_source_filename:?}, got {source_filename:?}"
        ));
    }

    if document.model_hierarchy.roots.len() != 1 {
        return batch_error(format!(
            "source {source_relative:?} must declare exactly one hierarchy root"
        ));
    }
    let parentless = document
        .model_hierarchy
        .nodes
        .iter()
        .filter(|node| node.parent.is_none())
        .collect::<Vec<_>>();
    if parentless.len() != 1 {
        return batch_error(format!(
            "source {source_relative:?} must contain exactly one parentless node"
        ));
    }
    let root = &document.model_hierarchy.roots[0];
    let root_node = parentless[0];
    if root.name != document.logical_name
        || root.path != document.logical_name
        || root_node.name != document.logical_name
        || root_node.path != document.logical_name
    {
        return batch_error(format!(
            "source {source_relative:?} logicalName is not the exact sole root m_Name"
        ));
    }

    for component in
        std::iter::once(family.as_str()).chain(semantic_directories.iter().map(String::as_str))
    {
        validate_windows_component(component, "family/semantic directory")?;
        if has_generated_identity(component) {
            return batch_error(format!(
                "generated/hash/PathID family or semantic directory is forbidden: {component:?}"
            ));
        }
    }
    let semantic_refs = semantic_directories
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let output_glb = model_relative_path(&family, &semantic_refs, &document.logical_name)
        .map_err(|error| batch_error_value(error.to_string()))?;
    let output_parent = output_glb
        .parent()
        .ok_or_else(|| batch_error_value("logical-model output has no parent"))?;
    let output_report = output_glb.with_file_name(format!("{safe_stem}.publish.json"));
    let texture_directory = format!("{safe_stem}.textures");
    let mut output_files = vec![output_glb.clone(), output_report];
    let invalid_texture_slot_evidence =
        invalid_saved_texture_slot_evidence(&document.materials, &source_relative)?;
    let mut preferred_name_counts = BTreeMap::<String, usize>::new();
    for texture in document.textures.values() {
        let preferred = minimal_windows_png_filename(&texture.name)
            .map_err(|error| batch_error_value(error.to_string()))?;
        *preferred_name_counts
            .entry(preferred.to_ascii_lowercase())
            .or_default() += 1;
    }
    for (texture_id, texture) in document.textures {
        if texture.mip_levels.0 == 0 {
            return batch_error(format!(
                "texture {texture_id:?} in {source_relative:?} has no mipLevels"
            ));
        }
        let preferred_png_name = minimal_windows_png_filename(&texture.name)
            .map_err(|error| batch_error_value(error.to_string()))?;
        let png_name = if preferred_name_counts
            .get(&preferred_png_name.to_ascii_lowercase())
            .copied()
            .unwrap_or_default()
            > 1
        {
            windows_png_filename_preserving_legacy_extension(&texture.name)
                .map_err(|error| batch_error_value(error.to_string()))?
        } else {
            preferred_png_name
        };
        let mip_stem = png_name
            .strip_suffix(".png")
            .ok_or_else(|| batch_error_value("minimal texture filename has no .png suffix"))?;
        for level in 0..texture.mip_levels.0 {
            let relative = if level == 0 {
                PathBuf::from(&texture_directory).join(&png_name)
            } else {
                PathBuf::from(&texture_directory)
                    .join(format!("{mip_stem}.mips"))
                    .join(format!("mip-{level:02}.png"))
            };
            output_files.push(output_parent.join(relative));
        }
    }
    output_files.sort();
    let logical_name = document.logical_name;
    let blocker =
        invalid_texture_slot_evidence.map(|evidence| LogicalModelBatchBlocker {
            code: "invalidSerializedTextureSlotName".to_owned(),
            source: source_relative.clone(),
            source_sha256: source_sha256.clone(),
            family: family.clone(),
            semantic_directories: semantic_directories.clone(),
            logical_name: logical_name.clone(),
            detail: "an exact saved material texture property has an empty/invalid serialized name; mapping it to a declared ShaderLab slot would be an unproved fallback".to_owned(),
            evidence,
            required_evidence: vec![
                "recover the authoritative non-empty property name from the exact legacy Material bytes and matching type tree".to_owned(),
                "prove the recovered property maps to one declared ShaderLab texture slot without changing saved array order or values".to_owned(),
                "regenerate the exact logical-model source with matching byte length/SHA evidence and rerun publication".to_owned(),
            ],
            disposition: "blocked-no-fallback-glb".to_owned(),
        });

    Ok(BatchPlan {
        source,
        source_relative,
        source_sha256,
        family,
        semantic_directories,
        logical_name,
        output_glb,
        output_files,
        blocker,
    })
}

pub(super) fn invalid_native_name(value: &str) -> bool {
    value.trim().is_empty() || value.len() > 1_024 || value.chars().any(char::is_control)
}

pub(super) fn build_report(
    plans: &[BatchPlan],
    coordinate_evidence: &[BatchCoordinateEvidence],
) -> Result<LogicalModelBatchPublishReport> {
    let published_plans = plans
        .iter()
        .filter(|plan| plan.blocker.is_none())
        .collect::<Vec<_>>();
    let blockers = plans
        .iter()
        .filter_map(|plan| plan.blocker.clone())
        .collect::<Vec<_>>();
    if published_plans.len() != coordinate_evidence.len() {
        return batch_error("coordinate evidence count differs from batch plan count");
    }
    let families = published_plans
        .iter()
        .map(|plan| plan.family.as_str())
        .collect::<BTreeSet<_>>();
    let semantic_directories = published_plans
        .iter()
        .flat_map(|plan| {
            let mut prefix = plan.family.clone();
            plan.semantic_directories.iter().map(move |directory| {
                prefix.push('/');
                prefix.push_str(directory);
                prefix.clone()
            })
        })
        .collect::<BTreeSet<_>>();
    let glbs = published_plans.len();
    let publish_reports = published_plans.len();
    let pngs = published_plans
        .iter()
        .flat_map(|plan| &plan.output_files)
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("png"))
        .count();
    let model_files = published_plans.iter().try_fold(0usize, |total, plan| {
        total
            .checked_add(plan.output_files.len())
            .ok_or_else(|| batch_error_value("batch file count overflow"))
    })?;
    let files = model_files
        .checked_add(1)
        .ok_or_else(|| batch_error_value("batch report file count overflow"))?;

    let coordinate_artifact_space_proven = coordinate_evidence
        .iter()
        .filter(|evidence| {
            evidence.coordinate_status == "artifact-space-proven-runtime-spawn-policy-pending"
        })
        .count();
    let coordinate_runtime_spawn_policy_pending = coordinate_evidence
        .iter()
        .filter(|evidence| evidence.runtime_spawn_policy == "runtime-archetype-dependent-pending")
        .count();
    let coordinate_mismatches = coordinate_evidence
        .iter()
        .filter(|evidence| {
            evidence.coordinate_status != "artifact-space-proven-runtime-spawn-policy-pending"
                || evidence.runtime_spawn_policy != "runtime-archetype-dependent-pending"
        })
        .count();
    let coordinate_status = if coordinate_mismatches == 0 {
        "artifact-space-proven-runtime-spawn-policy-pending"
    } else {
        "coordinate-evidence-mismatch"
    };
    let skinning_basis_parity_max_error = coordinate_evidence
        .iter()
        .filter_map(|evidence| evidence.skinning_basis_parity_max_error)
        .reduce(f64::max);
    let current_pose_bind_identity_deviation_max = coordinate_evidence
        .iter()
        .filter_map(|evidence| evidence.current_pose_bind_identity_deviation_max)
        .reduce(f64::max);

    Ok(LogicalModelBatchPublishReport {
        schema: LOGICAL_MODEL_BATCH_REPORT_SCHEMA.to_owned(),
        status: if blockers.is_empty() {
            "structural-audit-passed-coordinate-runtime-pending-gpu-pending".to_owned()
        } else {
            "complete-with-blocked-structural-audit-passed-coordinate-runtime-pending-gpu-pending"
                .to_owned()
        },
        structural_audit_passed: true,
        coordinate_status: coordinate_status.to_owned(),
        coordinate_artifact_space_proven: to_u64(
            coordinate_artifact_space_proven,
            "artifact coordinate proof count",
        )?,
        coordinate_runtime_spawn_policy_pending: to_u64(
            coordinate_runtime_spawn_policy_pending,
            "runtime spawn coordinate policy pending count",
        )?,
        coordinate_mismatches: to_u64(coordinate_mismatches, "coordinate mismatch count")?,
        skinning_basis_parity_max_error,
        current_pose_bind_identity_deviation_max,
        gpu_gate_pending: true,
        candidate_publishable: false,
        counts: LogicalModelBatchCounts {
            planned_sources: to_u64(plans.len(), "planned source count")?,
            sources: to_u64(published_plans.len(), "published source count")?,
            blocked_sources: to_u64(blockers.len(), "blocked source count")?,
            families: to_u64(families.len(), "family count")?,
            semantic_directories: to_u64(semantic_directories.len(), "semantic directory count")?,
            files: to_u64(files, "file count")?,
            glbs: to_u64(glbs, "GLB count")?,
            pngs: to_u64(pngs, "PNG count")?,
            publish_reports: to_u64(publish_reports, "publish report count")?,
        },
        models: published_plans
            .iter()
            .zip(coordinate_evidence)
            .map(|(plan, coordinate)| LogicalModelBatchMapping {
                source: plan.source_relative.clone(),
                source_sha256: plan.source_sha256.clone(),
                family: plan.family.clone(),
                semantic_directories: plan.semantic_directories.clone(),
                logical_name: plan.logical_name.clone(),
                output_glb: slash_path(&plan.output_glb),
                coordinate_status: coordinate.coordinate_status.clone(),
                runtime_spawn_policy: coordinate.runtime_spawn_policy.clone(),
                skinning_basis_parity_status: coordinate.skinning_basis_parity_status.clone(),
                skinning_basis_parity_max_error: coordinate.skinning_basis_parity_max_error,
                current_pose_bind_identity_deviation_max: coordinate
                    .current_pose_bind_identity_deviation_max,
            })
            .collect(),
        blockers,
    })
}

pub(super) fn portable_string_key(value: &str) -> String {
    // NFKC catches canonically equivalent and compatibility spellings. The
    // second normalization makes lowercase expansions deterministic too.
    value.nfkc().flat_map(char::to_lowercase).nfkc().collect()
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn to_u64(value: usize, label: &str) -> Result<u64> {
    value
        .try_into()
        .map_err(|_| batch_error_value(format!("{label} exceeds u64")))
}
