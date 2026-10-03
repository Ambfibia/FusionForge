use super::*;

pub(super) fn candidate_files(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| io_at(&directory, error))? {
            let entry = entry.map_err(|error| io_at(&directory, error))?;
            let file_type = entry
                .file_type()
                .map_err(|error| io_at(entry.path(), error))?;
            if file_type.is_symlink() {
                return invalid(format!(
                    "logical character candidates may not contain symlinks: {:?}",
                    entry.path()
                ));
            }
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                let relative = relative_path(root, &entry.path())?;
                if relative != "logical-model-batch-report.json" {
                    if !relative.starts_with("models/")
                        || !(relative.ends_with(".glb")
                            || relative.ends_with(".png")
                            || relative.ends_with(".publish.json"))
                    {
                        return invalid(format!(
                            "unexpected file in audited logical character tree: {relative:?}"
                        ));
                    }
                    files.push((relative, entry.path()));
                }
            } else {
                return invalid(format!(
                    "logical character candidate is not a regular file: {:?}",
                    entry.path()
                ));
            }
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

pub(super) fn installed_character_files(root: &Path) -> Result<Vec<String>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| io_at(&directory, error))? {
            let entry = entry.map_err(|error| io_at(&directory, error))?;
            let file_type = entry
                .file_type()
                .map_err(|error| io_at(entry.path(), error))?;
            if file_type.is_symlink() {
                return invalid("installed character tree contains a symlink");
            }
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                files.push(relative_path(root, &entry.path())?);
            } else {
                return invalid("installed character tree contains a non-regular file");
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn create_stage(asset_root: &Path) -> Result<PathBuf> {
    for _ in 0..128 {
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = asset_root.join(format!(
            ".characters-install-stage-{}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_at(&path, error)),
        }
    }
    Err(PipelineError::StagingCollision(
        asset_root.join("characters"),
    ))
}

pub(super) fn kind_for(path: &str) -> Result<ProjectAssetKind> {
    if path.ends_with(".glb") {
        Ok(ProjectAssetKind::Model)
    } else if path.ends_with(".png") {
        Ok(ProjectAssetKind::Texture)
    } else if path.ends_with(".publish.json") {
        Ok(ProjectAssetKind::Data)
    } else {
        invalid(format!("unsupported logical character file {path:?}"))
    }
}

pub(super) fn semantic_kind(logical_name: &str) -> Result<&'static str> {
    if logical_name.starts_with("npc_") {
        Ok("npc")
    } else if logical_name.starts_with("fusion_") {
        Ok("mob")
    } else if logical_name.starts_with("nano_") {
        Ok("nano")
    } else {
        invalid(format!(
            "logical character true name has no typed runtime taxonomy: {logical_name:?}"
        ))
    }
}

pub(super) fn parent_slash(path: &str) -> Result<String> {
    Path::new(path)
        .parent()
        .map(slash_path)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| invalid_error(format!("semantic path has no parent: {path:?}")))
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    if !canonical.is_dir() {
        return invalid(format!("{label} is not a directory: {canonical:?}"));
    }
    Ok(canonical)
}

pub(super) fn join_relative(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative(relative)?;
    Ok(relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component)))
}

pub(crate) fn verify_archived_gpu_evidence(
    candidate_root: &Path,
    audit: &LogicalModelGpuEvidenceAuditReport,
) -> Result<()> {
    let audited_candidate = fs::canonicalize(Path::new(&audit.candidate_root))
        .map_err(|error| io_at(&audit.candidate_root, error))?;
    if audited_candidate != candidate_root {
        return invalid("stored GPU evidence was produced for a different candidate root");
    }
    let evidence_root = canonical_directory(Path::new(&audit.evidence_root), "GPU evidence root")?;
    for model in &audit.models {
        validate_relative(&model.relative_glb)?;
        validate_relative(&model.evidence_json)?;
        validate_relative(&model.screenshot_png)?;
        let glb_path = join_relative(candidate_root, &model.relative_glb)?;
        let glb = fs::read(&glb_path).map_err(|error| io_at(&glb_path, error))?;
        let evidence_path = join_relative(&evidence_root, &model.evidence_json)?;
        let evidence_bytes =
            fs::read(&evidence_path).map_err(|error| io_at(&evidence_path, error))?;
        let evidence: LogicalModelGpuEvidence =
            serde_json::from_slice(&evidence_bytes).map_err(|source| PipelineError::Json {
                path: evidence_path.display().to_string(),
                source,
            })?;
        if evidence.schema != GPU_EVIDENCE_SCHEMA
            || evidence.render_profile != GPU_RENDER_PROFILE
            || evidence.visual_parity != VisualParityClaim::NotAsserted
            || evidence.model.relative_glb != model.relative_glb
            || evidence.model.true_name != model.true_name.as_deref().unwrap_or_default()
            || evidence.model.glb_byte_length != glb.len() as u64
            || evidence.model.glb_sha256 != sha256(&glb)
            || !evidence.runtime.scene_ready
            || evidence.runtime.material_errors != 0
            || evidence.runtime.shader_errors != 0
            || evidence.runtime.resolved_skin_joint_references
                != evidence.runtime.skin_joint_references
        {
            return invalid(format!(
                "archived GPU evidence identity/runtime gate failed for {:?}",
                model.relative_glb
            ));
        }
        let facts = gpu_model_facts_from_glb(&glb)
            .map_err(|error| invalid_error(format!("GPU facts failed: {error}")))?;
        if facts.true_name != evidence.model.true_name
            || facts.mesh_parts != evidence.runtime.mesh_parts
            || facts.skinned_mesh_parts != evidence.runtime.skinned_mesh_parts
            || facts.skin_joint_references != evidence.runtime.skin_joint_references
            || facts.inverse_bind_matrices != evidence.runtime.inverse_bind_matrices
            || facts.materials_applied != evidence.runtime.materials_applied
            || facts.legacy_pass_companions != evidence.runtime.legacy_pass_companions
            || facts.outline_pass_companions != evidence.runtime.outline_pass_companions
            || facts.assigned_texture_bindings != evidence.runtime.assigned_texture_bindings
            || facts.exact_mip_markers != evidence.runtime.exact_mip_markers
            || facts.exact_mip_chains != evidence.runtime.exact_mip_chains
            || facts.exact_mip_levels != evidence.runtime.exact_mip_levels
        {
            return invalid(format!(
                "archived GPU facts no longer match GLB {:?}",
                model.relative_glb
            ));
        }
        if let Some(selected) = &evidence.animation.selected_exact_name {
            if !facts.standard_animation_names.contains(selected) {
                return invalid(format!(
                    "archived sampled animation {selected:?} is absent from {:?}",
                    model.relative_glb
                ));
            }
        }
        let screenshot_path = join_relative(&evidence_root, &model.screenshot_png)?;
        let screenshot =
            fs::read(&screenshot_path).map_err(|error| io_at(&screenshot_path, error))?;
        if evidence.screenshot.relative_png != model.screenshot_png
            || evidence.screenshot.byte_length != screenshot.len() as u64
            || evidence.screenshot.sha256 != sha256(&screenshot)
            || evidence.screenshot.foreground_pixels == 0
        {
            return invalid(format!(
                "archived GPU screenshot evidence failed for {:?}",
                model.relative_glb
            ));
        }
        let image = image::load_from_memory(&screenshot).map_err(|error| {
            invalid_error(format!(
                "GPU evidence screenshot {:?} is not a valid PNG: {error}",
                model.screenshot_png
            ))
        })?;
        if image.width() != evidence.screenshot.width
            || image.height() != evidence.screenshot.height
        {
            return invalid(format!(
                "archived GPU screenshot dimensions changed for {:?}",
                model.relative_glb
            ));
        }
    }
    Ok(())
}

pub(super) fn verify_sha256_file(path: &Path, expected: &str) -> Result<()> {
    let bytes = fs::read(path).map_err(|error| io_at(path, error))?;
    let actual = sha256(&bytes);
    if actual != expected {
        return invalid(format!(
            "published PNG SHA-256 mismatch at {}: report={expected}, disk={actual}",
            path.display()
        ));
    }
    Ok(())
}

pub(super) fn string<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

pub(super) fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    string(value, field)
        .ok_or_else(|| invalid_error(format!("required string field {field:?} is missing")))
}

pub(super) fn integer(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

pub(super) fn float_array<const N: usize>(value: &Value, field: &str) -> Result<[f64; N]> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_error(format!("required numeric array {field:?} is missing")))?;
    if values.len() != N {
        return invalid(format!(
            "{field:?} has {} values, expected {N}",
            values.len()
        ));
    }
    let mut result = [0.0; N];
    for (index, item) in values.iter().enumerate() {
        result[index] = item
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| invalid_error(format!("{field:?}[{index}] is not finite")))?;
    }
    Ok(result)
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
